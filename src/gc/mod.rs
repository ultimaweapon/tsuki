#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

pub use self::lock::*;
pub use self::r#ref::*;

pub(crate) use self::mark::*;
pub(crate) use self::object::*;

use crate::ldo::luaD_shrinkstack;
use crate::lfunc::luaF_unlinkupval;
use crate::lobject::{CClosure, Proto, StkId, Udata, UpVal};
use crate::ltm::{TM_MODE, luaT_gettm};
use crate::table::luaH_realasize;
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, Node, Str, Table, Thread, UserId};
use alloc::alloc::handle_alloc_error;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::offset_of;
use core::ptr::{null, null_mut};

mod lock;
mod mark;
mod object;
mod r#ref;

type c_int = i32;
type c_uint = u32;
type c_long = i64;

/// Garbage Collector for Lua objects.
pub(crate) struct Gc {
    state: Cell<u8>,
    currentwhite: Cell<u8>,
    all: Cell<*const Object>,
    gray: Cell<*const Object>,
    grayagain: Cell<*const Object>,
    weak: Cell<*const Object>,
    ephemeron: Cell<*const Object>,
    allweak: Cell<*const Object>,
    twups: Cell<*const Thread>,
    sweep: Cell<*mut *const Object>,
    sweep_mark: Cell<*const Object>,
    refs: Cell<*const Object>,
    root: Cell<*const Object>,
    locks: Cell<usize>,
    paused: Cell<bool>,
}

impl Gc {
    /// # Safety
    /// - The returned [`Gc`] must be the first value on [`Lua`].
    /// - [`Lua`] must not moved for its entire lifetime.
    pub unsafe fn new() -> Self {
        Self {
            state: Cell::new(8),
            currentwhite: Cell::new(1 << 3),
            all: Cell::new(null()),
            gray: Cell::new(null()),
            grayagain: Cell::new(null()),
            weak: Cell::new(null()),
            ephemeron: Cell::new(null()),
            allweak: Cell::new(null()),
            twups: Cell::new(null()),
            sweep: Cell::new(null_mut()),
            sweep_mark: Cell::new(null()),
            refs: Cell::new(null()),
            root: Cell::new(null()),
            locks: Cell::new(0),
            paused: Cell::new(false),
        }
    }

    #[inline(always)]
    pub unsafe fn set_root(&self, o: *const Object) {
        self.root.set(o);
    }

    #[inline(always)]
    pub unsafe fn set_twups(&self, th: *const Thread) {
        (*th).twups.set(self.twups.get());
        self.twups.set(th);
    }

    #[inline(always)]
    pub unsafe fn is_dead(&self, o: *const Object) -> bool {
        (*o).marked.get() & (self.currentwhite.get() ^ (1 << 3 | 1 << 4)) != 0
    }

    /// Resurrects `o` if it dead.
    #[inline(always)]
    pub unsafe fn resurrect(&self, o: *const Object) {
        if self.is_dead(o) {
            (*o).marked.set((*o).marked.get() ^ (1 << 3 | 1 << 4));
        }
    }

    pub unsafe fn barrier(&self, o: *const Object, v: *const Object) {
        if self.state.get() <= 2 {
            self.mark(v);

            if (*o).marked.get() as c_int & 7 as c_int > 1 as c_int {
                (*v).marked
                    .set(((*v).marked.get() as c_int & !(7 as c_int) | 2) as u8);
            }
        } else {
            (*o).marked.set(
                (*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4))
                    | (self.currentwhite.get() & (1 << 3 | 1 << 4)),
            );
        }
    }

    pub unsafe fn barrier_back(&self, o: *const Object) {
        if (*o).marked.get() as c_int & 7 as c_int == 6 as c_int {
            (*o).marked
                .set((*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
        } else {
            Self::linkgclist_(o, Self::getgclist(o), self.grayagain.as_ptr());
        }

        if (*o).marked.get() as c_int & 7 as c_int > 1 as c_int {
            (*o).marked
                .set(((*o).marked.get() as c_int & !(7 as c_int) | 5 as c_int) as u8);
        }
    }

    /// # Safety
    /// `layout` must have the layout of [`Object`] at the beginning.
    pub unsafe fn alloc(&self, tt: u8, layout: Layout) -> *mut Object {
        let o = unsafe { alloc::alloc::alloc(layout).cast::<Object>() };

        if o.is_null() {
            handle_alloc_error(layout);
        }

        o.write(Object {
            global: self as *const Self as *const Lua,
            next: Cell::new(self.all.get()),
            tt,
            marked: Mark::new(self.currentwhite.get() & (1 << 3 | 1 << 4)),
            refs: Cell::new(0),
            refn: Cell::new(null()),
            refp: Cell::new(null()),
            gclist: Cell::new(null()),
        });

        self.all.set(o);

        o
    }

    #[inline(always)]
    pub fn lock(&self) -> GcLock {
        GcLock::new(self)
    }

    #[inline(never)]
    fn step(&self) {
        if self.paused.get() {
            return;
        }

        match self.state.get() {
            8 => unsafe {
                self.gray.set(null());
                self.grayagain.set(null());
                self.weak.set(null());
                self.ephemeron.set(null());
                self.allweak.set(null());
                self.mark_roots();
                self.state.set(0);
            },
            0 => unsafe {
                if self.gray.get().is_null() {
                    self.state.set(1);
                } else {
                    self.mark_one_gray();
                }
            },
            1 => unsafe {
                self.finish_marking();

                // Insert sweep mark to the head.
                let mut m = self.sweep_mark.replace(null());

                match m.is_null() {
                    true => m = self.alloc(15 | 0 << 4, Layout::new::<Object>()),
                    false => {
                        (*m).marked.set(self.currentwhite.get() & (1 << 3 | 1 << 4));
                        (*m).next.set(self.all.get());

                        self.all.set(m);
                    }
                }

                self.sweep.set((*m).next.as_ptr());
                self.state.set(3);
            },
            3 => unsafe {
                let p = self.sweep.get();

                if p.is_null() {
                    self.state.set(8);
                } else {
                    self.sweep.set(self.sweep(p));
                }
            },
            _ => unreachable!(),
        }
    }

    unsafe fn mark_roots(&self) {
        // Mark object with strong references.
        let mut o = self.refs.get();

        while !o.is_null() {
            if unsafe { ((*o).marked.get() & (1 << 3 | 1 << 4)) != 0 } {
                self.mark(o);
            }

            o = unsafe { (*o).refp.get() };
        }

        // Mark root.
        let o = self.root.get();

        if unsafe { !o.is_null() && (*o).marked.get() & (1 << 3 | 1 << 4) != 0 } {
            self.mark(o);
        }
    }

    unsafe fn mark(&self, o: *const Object) {
        match (*o).tt {
            4 | 20 | 11 => {
                (*o).marked
                    .set((*o).marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
                return;
            }
            9 => {
                let uv = o as *const UpVal;

                if (*uv).v.get() != &raw mut (*(*uv).u.get()).value as *mut UnsafeValue {
                    (*uv)
                        .hdr
                        .marked
                        .set((*uv).hdr.marked.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
                } else {
                    (*uv)
                        .hdr
                        .marked
                        .set((*uv).hdr.marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
                }

                if (*(*uv).v.get()).tt_ & 1 << 6 != 0
                    && (*(*(*uv).v.get()).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
                {
                    self.mark((*(*uv).v.get()).value_.gc);
                }

                return;
            }
            7 => {
                let u = o as *const Udata;

                if (*u).nuvalue == 0 {
                    if !((*u).metatable).is_null() {
                        if (*(*u).metatable).hdr.marked.get() as c_int
                            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                            != 0
                        {
                            self.mark((*u).metatable.cast());
                        }
                    }
                    (*u).hdr
                        .marked
                        .set((*u).hdr.marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
                    return;
                }
            }
            6 | 38 | 5 | 8 | 10 => {}
            _ => unreachable!(),
        }

        Self::linkgclist_(o, Self::getgclist(o), self.gray.as_ptr());
    }

    unsafe fn mark_one_gray(&self) -> usize {
        let o = self.gray.get();

        (*o).marked.set((*o).marked.get() | 1 << 5);

        self.gray.set(*Self::getgclist(o));

        match (*o).tt {
            5 => self.mark_table(o.cast()),
            7 => self.mark_ud(o.cast()) as usize,
            6 => self.mark_lf(o.cast()),
            38 => self.mark_rf(o.cast()) as usize,
            10 => self.mark_proto(o.cast()) as usize,
            8 => self.mark_thread(o.cast()) as usize,
            _ => unreachable!(),
        }
    }

    unsafe fn mark_table(&self, h: *const Table) -> usize {
        // Get table mode.
        let mode = if (*h).metatable.get().is_null() {
            null()
        } else if (*(*h).metatable.get()).flags.get() & 1 << TM_MODE != 0 {
            null()
        } else {
            let s = luaT_gettm((*h).metatable.get(), TM_MODE);

            if !s.is_null() && (*s).tt_ == 4 | 0 << 4 | 1 << 6 {
                (*s).value_.gc.cast::<Str>()
            } else {
                null()
            }
        };

        // Mark metatable.
        if !((*h).metatable.get()).is_null() {
            if (*(*h).metatable.get()).hdr.marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                self.mark((*h).metatable.get().cast());
            }
        }

        // Traverse table.
        let (wk, wv) = match mode.as_ref().map(|v| v.as_bytes()) {
            Some(v) => (v.contains(&b'k'), v.contains(&b'v')),
            None => (false, false),
        };

        match (wk, wv) {
            (true, true) => Self::linkgclist_(
                h as *mut Object,
                (*h).hdr.gclist.as_ptr(),
                self.allweak.as_ptr(),
            ),
            (true, false) => {
                self.mark_ephemeron(h, 0);
            }
            (false, true) => self.mark_weak_value(h),
            (false, false) => self.mark_strong_table(h),
        }

        (1 as c_int as c_uint)
            .wrapping_add((*h).alimit.get())
            .wrapping_add(
                (2 as c_int
                    * (if ((*h).lastfree.get()).is_null() {
                        0 as c_int
                    } else {
                        (1 as c_int) << (*h).lsizenode.get() as c_int
                    })) as c_uint,
            ) as usize
    }

    unsafe fn mark_strong_table(&self, h: *const Table) {
        let mut n: *mut Node = 0 as *mut Node;
        let limit: *mut Node = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node;
        let mut i: c_uint = 0;
        let asize: c_uint = luaH_realasize(h);
        i = 0 as c_int as c_uint;
        while i < asize {
            if (*(*h).array.get().offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*(*h).array.get().offset(i as isize)).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*(*h).array.get().offset(i as isize)).value_.gc);
            }
            i = i.wrapping_add(1);
        }
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
        while n < limit {
            if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                Self::clear_key(n);
            } else {
                if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0
                    && (*(*n).u.key_val.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    self.mark((*n).u.key_val.gc);
                }
                if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                    && (*(*n).i_val.value_.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    self.mark((*n).i_val.value_.gc);
                }
            }
            n = n.offset(1);
        }
    }

    unsafe fn mark_weak_value(&self, h: *const Table) {
        let mut n: *mut Node = 0 as *mut Node;
        let limit: *mut Node = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node;
        let mut hasclears: c_int = ((*h).alimit.get() > 0 as c_int as c_uint) as c_int;
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
        while n < limit {
            if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                Self::clear_key(n);
            } else {
                if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0
                    && (*(*n).u.key_val.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    self.mark((*n).u.key_val.gc);
                }
                if hasclears == 0
                    && self.is_cleared(
                        if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                            (*n).i_val.value_.gc
                        } else {
                            0 as *mut Object
                        },
                    )
                {
                    hasclears = 1 as c_int;
                }
            }
            n = n.offset(1);
        }

        if self.state.get() == 2 && hasclears != 0 {
            Self::linkgclist_(
                h as *const Object,
                (*h).hdr.gclist.as_ptr(),
                self.weak.as_ptr(),
            );
        } else {
            Self::linkgclist_(
                h as *const Object,
                (*h).hdr.gclist.as_ptr(),
                self.grayagain.as_ptr(),
            );
        };
    }

    unsafe fn mark_ephemeron(&self, h: *const Table, inv: i32) -> i32 {
        let mut marked: c_int = 0 as c_int;
        let mut hasclears: c_int = 0 as c_int;
        let mut hasww: c_int = 0 as c_int;
        let mut i: c_uint = 0;
        let asize: c_uint = luaH_realasize(h);
        let nsize: c_uint = ((1 as c_int) << (*h).lsizenode.get() as c_int) as c_uint;
        i = 0 as c_int as c_uint;
        while i < asize {
            if (*(*h).array.get().offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*(*h).array.get().offset(i as isize)).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                marked = 1 as c_int;
                self.mark((*(*h).array.get().offset(i as isize)).value_.gc);
            }
            i = i.wrapping_add(1);
        }
        i = 0 as c_int as c_uint;
        while i < nsize {
            let n: *mut Node = if inv != 0 {
                ((*h).node.get())
                    .offset(nsize.wrapping_sub(1 as c_int as c_uint).wrapping_sub(i) as isize)
                    as *mut Node
            } else {
                ((*h).node.get()).offset(i as isize) as *mut Node
            };
            if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                Self::clear_key(n);
            } else if self.is_cleared(
                if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*n).u.key_val.gc
                } else {
                    0 as *mut Object
                },
            ) {
                hasclears = 1 as c_int;
                if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                    && (*(*n).i_val.value_.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    hasww = 1 as c_int;
                }
            } else if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*n).i_val.value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                marked = 1 as c_int;
                self.mark((*n).i_val.value_.gc);
            }
            i = i.wrapping_add(1);
        }

        if self.state.get() == 0 {
            Self::linkgclist_(
                h as *const Object,
                (*h).hdr.gclist.as_ptr(),
                self.grayagain.as_ptr(),
            );
        } else if hasww != 0 {
            Self::linkgclist_(
                h as *const Object,
                (*h).hdr.gclist.as_ptr(),
                self.ephemeron.as_ptr(),
            );
        } else if hasclears != 0 {
            Self::linkgclist_(
                h as *const Object,
                (*h).hdr.gclist.as_ptr(),
                self.allweak.as_ptr(),
            );
        }

        marked
    }

    unsafe fn mark_ud(&self, u: *const Udata) -> i32 {
        let mut i: c_int = 0;

        if !((*u).metatable).is_null() {
            if (*(*u).metatable).hdr.marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                self.mark((*u).metatable.cast());
            }
        }

        i = 0 as c_int;

        while i < (*u).nuvalue as c_int {
            if (*((*u).uv).as_ptr().offset(i as isize)).tt_ & 1 << 6 != 0
                && (*(*((*u).uv).as_ptr().offset(i as isize)).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*((*u).uv).as_ptr().offset(i as isize)).value_.gc);
            }

            i += 1;
        }

        return 1 as c_int + (*u).nuvalue as c_int;
    }

    unsafe fn mark_lf(&self, cl: *const LuaFn) -> usize {
        let p = (*cl).p.get();

        if !p.is_null() && (*p).hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            self.mark(p.cast());
        }

        for uv in (*cl)
            .upvals
            .iter()
            .map(|v| v.get())
            .filter(|v| !v.is_null())
        {
            if (*uv).hdr.marked.get() as c_int & ((1 as c_int) << 3 | 1 << 4) != 0 {
                self.mark(uv.cast());
            }
        }

        1 + (&(*cl).upvals).len()
    }

    unsafe fn mark_rf(&self, cl: *const CClosure) -> c_int {
        let mut i: c_int = 0;

        while i < (*cl).nupvalues as c_int {
            if (*((*cl).upvalue).as_ptr().offset(i as isize)).tt_ as c_int
                & (1 as c_int) << 6 as c_int
                != 0
                && (*(*((*cl).upvalue).as_ptr().offset(i as isize)).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*((*cl).upvalue).as_ptr().offset(i as isize)).value_.gc);
            }
            i += 1;
        }

        return 1 as c_int + (*cl).nupvalues as c_int;
    }

    unsafe fn mark_proto(&self, f: *const Proto) -> c_int {
        let mut i = 0 as c_int;

        while i < (*f).sizek {
            if (*((*f).k).offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*((*f).k).offset(i as isize)).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*((*f).k).offset(i as isize)).value_.gc);
            }
            i += 1;
        }

        i = 0 as c_int;
        while i < (*f).sizeupvalues {
            if !((*((*f).upvalues).offset(i as isize)).name).is_null() {
                if (*(*((*f).upvalues).offset(i as isize)).name)
                    .hdr
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
                {
                    self.mark((*((*f).upvalues).offset(i as isize)).name as *const Object);
                }
            }
            i += 1;
        }
        i = 0 as c_int;
        while i < (*f).sizep {
            if !(*((*f).p).offset(i as isize)).is_null() {
                if (**((*f).p).offset(i as isize)).hdr.marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
                {
                    self.mark(*((*f).p).offset(i as isize) as *const Object);
                }
            }
            i += 1;
        }
        i = 0 as c_int;
        while i < (*f).sizelocvars {
            if !((*((*f).locvars).offset(i as isize)).varname).is_null() {
                if (*(*((*f).locvars).offset(i as isize)).varname)
                    .hdr
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
                {
                    self.mark((*((*f).locvars).offset(i as isize)).varname as *const Object);
                }
            }
            i += 1;
        }
        return 1 as c_int + (*f).sizek + (*f).sizeupvalues + (*f).sizep + (*f).sizelocvars;
    }

    unsafe fn mark_thread(&self, th: *const Thread) -> i32 {
        let mut uv: *mut UpVal = 0 as *mut UpVal;
        let mut o: StkId = (*th).stack.get();

        if self.state.get() == 0 {
            Self::linkgclist_(
                th as *const Object,
                (*th).hdr.gclist.as_ptr(),
                self.grayagain.as_ptr(),
            );
        }

        if o.is_null() {
            return 1 as c_int;
        }

        while o < (*th).top.get() {
            if (*o).val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*o).val.value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*o).val.value_.gc);
            }
            o = o.offset(1);
        }

        uv = (*th).openupval.get();

        while !uv.is_null() {
            if (*uv).hdr.marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                self.mark(uv.cast());
            }

            uv = (*(*uv).u.get()).open.next;
        }

        if self.state.get() == 2 {
            luaD_shrinkstack(th);

            o = (*th).top.get();

            while o < ((*th).stack_last.get()).offset(5 as c_int as isize) {
                (*o).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                o = o.offset(1);
            }

            if !((*th).twups.get() != th) && !((*th).openupval.get()).is_null() {
                (*th).twups.set(self.twups.get());
                self.twups.set(th);
            }
        }

        1 + ((*th).stack_last.get()).offset_from((*th).stack.get()) as c_long as c_int
    }

    unsafe fn finish_marking(&self) -> usize {
        let mut work = 0;
        let grayagain = self.grayagain.get();

        self.grayagain.set(null_mut());
        self.state.set(2);

        self.mark_roots();

        work += self.mark_all_gray();
        work += self.remark_upvalues() as usize;
        work += self.mark_all_gray();

        self.gray.set(grayagain);

        work += self.mark_all_gray();

        self.converge_ephemerons();
        self.clear_by_values(self.weak.get(), 0 as *mut Object);
        self.clear_by_values(self.allweak.get(), 0 as *mut Object);

        let ow = self.weak.get();
        let oa = self.allweak.get();

        work += self.mark_all_gray();

        self.converge_ephemerons();
        self.clear_by_keys(self.ephemeron.get());
        self.clear_by_keys(self.allweak.get());
        self.clear_by_values(self.weak.get(), ow);
        self.clear_by_values(self.allweak.get(), oa);

        self.currentwhite
            .set(self.currentwhite.get() ^ (1 << 3 | 1 << 4));

        work
    }

    unsafe fn mark_all_gray(&self) -> usize {
        let mut tot: usize = 0 as c_int as usize;

        while !self.gray.get().is_null() {
            tot += self.mark_one_gray();
        }

        return tot;
    }

    unsafe fn remark_upvalues(&self) -> c_int {
        let mut p = self.twups.as_ptr();
        let mut work: c_int = 0;

        loop {
            let th = *p;

            if th.is_null() {
                break;
            }

            work += 1;

            if (*th).hdr.marked.get() & (1 << 3 | 1 << 4) == 0 && !(*th).openupval.get().is_null() {
                p = (*th).twups.as_ptr();
            } else {
                let mut uv = (*th).openupval.get();

                *p = (*th).twups.replace(th);

                while !uv.is_null() {
                    work += 1;

                    if (*uv).hdr.marked.get() & (1 << 3 | 1 << 4) == 0 {
                        if (*(*uv).v.get()).tt_ & 1 << 6 != 0 {
                            if (*(*(*uv).v.get()).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 {
                                self.mark((*(*uv).v.get()).value_.gc);
                            }
                        }
                    }

                    uv = (*(*uv).u.get()).open.next;
                }
            }
        }

        return work;
    }

    unsafe fn converge_ephemerons(&self) {
        let mut changed: c_int = 0;
        let mut dir: c_int = 0 as c_int;

        loop {
            let mut next = self.ephemeron.replace(null());

            changed = 0 as c_int;

            loop {
                let w = next.cast::<Table>();

                if w.is_null() {
                    break;
                }

                next = (*w).hdr.gclist.get();

                (*w).hdr.marked.set((*w).hdr.marked.get() | 1 << 5);

                if self.mark_ephemeron(w, dir) != 0 {
                    self.mark_all_gray();
                    changed = 1 as c_int;
                }
            }

            dir = (dir == 0) as c_int;

            if !(changed != 0) {
                break;
            }
        }
    }

    unsafe fn clear_by_keys(&self, mut l: *const Object) {
        while !l.is_null() {
            let h: *mut Table = l as *mut Table;
            let limit: *mut Node = ((*h).node.get())
                .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
                as *mut Node;
            let mut n: *mut Node = 0 as *mut Node;
            n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
            while n < limit {
                if self.is_cleared(
                    if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
                        (*n).u.key_val.gc
                    } else {
                        0 as *mut Object
                    },
                ) {
                    (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }
                if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                    Self::clear_key(n);
                }
                n = n.offset(1);
            }
            l = (*(l as *mut Table)).hdr.gclist.get();
        }
    }

    unsafe fn clear_by_values(&self, mut l: *const Object, f: *const Object) {
        while l != f {
            let h: *mut Table = l as *mut Table;
            let mut n: *mut Node = 0 as *mut Node;
            let limit: *mut Node = ((*h).node.get())
                .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
                as *mut Node;
            let mut i: c_uint = 0;
            let asize: c_uint = luaH_realasize(h);
            i = 0 as c_int as c_uint;
            while i < asize {
                let o: *mut UnsafeValue = (*h).array.get().offset(i as isize) as *mut UnsafeValue;
                if self.is_cleared(if (*o).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*o).value_.gc
                } else {
                    0 as *mut Object
                }) {
                    (*o).tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }
                i = i.wrapping_add(1);
            }
            n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
            while n < limit {
                if self.is_cleared(
                    if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        (*n).i_val.value_.gc
                    } else {
                        0 as *mut Object
                    },
                ) {
                    (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }
                if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                    Self::clear_key(n);
                }
                n = n.offset(1);
            }
            l = (*(l as *mut Table)).hdr.gclist.get();
        }
    }

    unsafe fn clear_key(n: *mut Node) {
        if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
            (*n).u.key_tt = (9 as c_int + 2 as c_int) as u8;
        }
    }

    unsafe fn sweep(&self, mut p: *mut *const Object) -> *mut *const Object {
        let pw = self.currentwhite.get() ^ (1 << 3 | 1 << 4);
        let cw = self.currentwhite.get() & (1 << 3 | 1 << 4);
        let mut i = 0;

        while !(*p).is_null() && i < 100 {
            let o = *p;
            let m = (*o).marked.get();

            if m & pw != 0 {
                *p = (*o).next.replace(null());
                self.free(o.cast_mut());
            } else {
                (*o).marked.set(m & !(1 << 5 | (1 << 3 | 1 << 4) | 7) | cw);
                p = (*o).next.as_ptr();
            }

            i += 1;
        }

        if (*p).is_null() { null_mut() } else { p }
    }

    unsafe fn free(&self, o: *mut Object) {
        self.paused.set(true);

        match (*o).tt {
            10 => unsafe {
                core::ptr::drop_in_place(o.cast::<Proto>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Proto>());
            },
            9 => unsafe {
                let o = o.cast::<UpVal>();

                if (*o).v.get() != &raw mut (*(*o).u.get()).value {
                    luaF_unlinkupval(o);
                }

                alloc::alloc::dealloc(o.cast(), Layout::new::<UpVal>());
            },
            6 => unsafe {
                core::ptr::drop_in_place(o.cast::<LuaFn>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<LuaFn>());
            },
            38 => unsafe {
                let cl_0: *mut CClosure = o as *mut CClosure;
                let nupvalues = usize::from((*cl_0).nupvalues);
                let size = offset_of!(CClosure, upvalue) + size_of::<UnsafeValue>() * nupvalues;
                let align = align_of::<CClosure>();
                let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

                alloc::alloc::dealloc(cl_0.cast(), layout);
            },
            5 => unsafe {
                core::ptr::drop_in_place(o.cast::<Table>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Table>());
            },
            8 => unsafe {
                core::ptr::drop_in_place(o.cast::<Thread>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Thread>());
            },
            7 => unsafe {
                let u: *mut Udata = o as *mut Udata;
                let layout = Layout::from_size_align(
                    offset_of!(Udata, uv)
                        + size_of::<UnsafeValue>()
                            .wrapping_mul((*u).nuvalue.into())
                            .wrapping_add((*u).len),
                    align_of::<Udata>(),
                )
                .unwrap()
                .pad_to_align();

                alloc::alloc::dealloc(o.cast(), layout);
            },
            4 => unsafe {
                let ts: *mut Str = o as *mut Str;
                let size = offset_of!(Str, contents) + usize::from((*ts).shrlen.get()) + 1;
                let align = align_of::<Str>();
                let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

                core::ptr::drop_in_place(ts);
                alloc::alloc::dealloc(ts.cast(), layout);
            },
            20 => unsafe {
                let ts_0: *mut Str = o as *mut Str;
                let size = offset_of!(Str, contents) + (*(*ts_0).u.get()).lnglen + 1;
                let align = align_of::<Str>();
                let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

                core::ptr::drop_in_place(ts_0);
                alloc::alloc::dealloc(ts_0.cast(), layout);
            },
            11 => unsafe {
                core::ptr::drop_in_place(o.cast::<UserId>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<UserId>());
            },
            15 => unsafe {
                let p = self.sweep_mark.replace(o).cast_mut();

                if !p.is_null() {
                    core::ptr::drop_in_place(p);
                    alloc::alloc::dealloc(p.cast(), Layout::new::<Object>());
                }
            },
            _ => unreachable!(),
        }

        self.paused.set(false);
    }

    #[inline(always)]
    unsafe fn getgclist(o: *const Object) -> *mut *const Object {
        match (*o).tt {
            5 | 6 | 7 | 8 | 10 | 38 => (*o).gclist.as_ptr(),
            _ => null_mut(),
        }
    }

    unsafe fn linkgclist_(o: *const Object, pnext: *mut *const Object, list: *mut *const Object) {
        *pnext = *list;
        *list = o;

        (*o).marked.set_gray();
    }

    unsafe fn is_cleared(&self, o: *const Object) -> bool {
        if o.is_null() {
            false
        } else if (*o).tt & 0xf == 4 {
            if (*o).marked.get() & (1 << 3 | 1 << 4) != 0 {
                self.mark(o);
            }

            false
        } else {
            (*o).marked.is_white()
        }
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        // Free all objects.
        let mut p = self.all.get();

        while !p.is_null() {
            let n = unsafe { (*p).next.replace(null()) };
            unsafe { self.free(p.cast_mut()) };
            p = n;
        }

        // Free remaining sweep mark.
        let m = self.sweep_mark.get().cast_mut();

        if !m.is_null() {
            unsafe { core::ptr::drop_in_place(m) };
            unsafe { alloc::alloc::dealloc(m.cast(), Layout::new::<Object>()) };
        }
    }
}
