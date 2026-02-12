#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

pub use self::r#ref::*;

pub(crate) use self::mark::*;
pub(crate) use self::object::*;

use crate::ldo::luaD_shrinkstack;
use crate::lfunc::luaF_unlinkupval;
use crate::lobject::{CClosure, Proto, UpVal};
use crate::ltm::{TM_MODE, luaT_gettm};
use crate::table::luaH_realasize;
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, Node, RustId, Str, Table, Thread, UserData};
use alloc::alloc::handle_alloc_error;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::offset_of;
use core::ptr::{null, null_mut};

mod mark;
mod object;
mod r#ref;

type c_int = i32;
type c_uint = u32;

/// Garbage Collector for Lua objects.
pub(crate) struct Gc<A> {
    state: Cell<u8>,
    currentwhite: Cell<u8>,
    all: Cell<*const Object<A>>,
    gray: Cell<*const Object<A>>,
    grayagain: Cell<*const Object<A>>,
    weak: Cell<*const Object<A>>,
    ephemeron: Cell<*const Object<A>>,
    allweak: Cell<*const Object<A>>,
    twups: Cell<*const Thread<A>>,
    sweep: Cell<*mut *const Object<A>>,
    sweep_mark: Cell<*const Object<A>>,
    refs: Cell<*const Object<A>>,
    marked_refs: Cell<*const Object<A>>,
    root: Cell<*const Object<A>>,
    debt: Cell<isize>,
    paused: Cell<bool>,
}

impl<A> Gc<A> {
    /// # Safety
    /// - The returned [Gc] must be the first value on [Lua].
    /// - [Lua] must not moved for its entire lifetime.
    /// - [Self::set_root()] must be called before the first call to [Self::step()].
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
            marked_refs: Cell::new(null()),
            root: Cell::new(null()),
            debt: Cell::new(0),
            paused: Cell::new(false),
        }
    }

    #[inline(always)]
    pub unsafe fn set_root(&self, o: *const Object<A>) {
        self.root.set(o);
    }

    #[inline(always)]
    pub unsafe fn set_twups(&self, th: *const Thread<A>) {
        (*th).twups.set(self.twups.get());
        self.twups.set(th);
    }

    #[inline(always)]
    pub unsafe fn is_dead(&self, o: *const Object<A>) -> bool {
        (*o).marked.get() & (self.currentwhite.get() ^ (1 << 3 | 1 << 4)) != 0
    }

    /// Resurrects `o` if it dead.
    #[inline(always)]
    pub unsafe fn resurrect(&self, o: *const Object<A>) {
        if self.is_dead(o) {
            (*o).marked.set((*o).marked.get() ^ (1 << 3 | 1 << 4));
        }
    }

    #[inline(always)]
    pub unsafe fn barrier(&self, o: *const Object<A>, v: *const Object<A>) {
        if self.state.get() <= 2 {
            self.mark(v);
        } else {
            (*o).marked.set(
                (*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4))
                    | (self.currentwhite.get() & (1 << 3 | 1 << 4)),
            );
        }
    }

    #[inline(always)]
    pub unsafe fn barrier_back(&self, o: *const Object<A>) {
        self.linkgclist_(o, (*o).gclist.as_ptr(), self.grayagain.as_ptr());
    }

    /// # Safety
    /// `layout` must have the layout of [`Object`] at the beginning.
    #[inline(always)]
    pub unsafe fn alloc(&self, tt: u8, layout: Layout) -> *mut Object<A> {
        let o = unsafe { alloc::alloc::alloc(layout).cast::<Object<A>>() };

        if o.is_null() {
            handle_alloc_error(layout);
        }

        o.write(Object {
            global: self as *const Self as *const Lua<A>,
            next: Cell::new(self.all.get()),
            tt,
            marked: Mark::new(self.currentwhite.get() & (1 << 3 | 1 << 4)),
            refs: Cell::new(0),
            refn: Cell::new(null_mut()),
            refp: Cell::new(null()),
            gclist: Cell::new(null()),
        });

        self.all.set(o);
        self.debt.update(|v| v + 1);

        o
    }

    #[inline(never)]
    pub fn step(&self) {
        if self.paused.get() {
            return;
        }

        match self.state.get() {
            8 => unsafe {
                // Reset lists.
                self.gray.set(null());
                self.grayagain.set(null());
                self.weak.set(null());
                self.ephemeron.set(null());
                self.allweak.set(null());

                // Mark root.
                let o = self.root.get();

                if (*o).marked.is_white() {
                    self.mark(o);
                }

                // Mark strong references.
                let mut o = self.refs.replace(null());

                if !o.is_null() {
                    self.marked_refs.set(o);

                    (*o).refn.set(self.marked_refs.as_ptr());

                    while !o.is_null() {
                        if (*o).marked.is_white() {
                            self.mark(o);
                        }

                        o = (*o).refp.get();
                    }
                }

                self.state.set(0);
            },
            0 => unsafe {
                if self.gray.get().is_null() {
                    self.state.set(1);
                } else if self.debt.get() > 0 {
                    self.mark_one_gray();
                    self.debt.update(|v| v - 1);
                }
            },
            1 => unsafe {
                self.finish_marking();

                // Insert sweep mark to the head.
                let mut m = self.sweep_mark.replace(null());

                match m.is_null() {
                    true => m = self.alloc(15 | 0 << 4, Layout::new::<Object<A>>()),
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

    #[inline(never)]
    pub unsafe fn mark(&self, o: *const Object<A>) {
        match (*o).tt {
            4 | 14 => {
                (*o).marked
                    .set((*o).marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
                self.debt.update(|v| v - 1);
                return;
            }
            9 => {
                let uv = o as *const UpVal<A>;

                if (*uv).v.get() != &raw mut (*(*uv).u.get()).value as *mut UnsafeValue<A> {
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

                self.debt.update(|v| v - 1);

                if (*(*uv).v.get()).tt_ & 1 << 6 != 0
                    && (*(*(*uv).v.get()).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
                {
                    self.mark((*(*uv).v.get()).value_.gc);
                }

                return;
            }
            7 => {
                let u = o.cast::<UserData<A, ()>>();
                let props = (*u).props.get();
                let mt = (*u).mt;
                let uv = (*u).uv;

                if !props.is_null() && (*props).hdr.marked.is_white() {
                    self.mark(props.cast());
                }

                if !mt.is_null() && (*mt).hdr.marked.is_white() {
                    self.mark(mt.cast());
                }

                if uv.tt_ & 1 << 6 != 0 && (*uv.value_.gc).marked.is_white() {
                    self.mark(uv.value_.gc);
                }

                (*u).hdr
                    .marked
                    .set((*u).hdr.marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);

                self.debt.update(|v| v - 1);

                return;
            }
            6 | 38 | 5 | 8 | 10 | 0x1E => {}
            _ => unreachable!(),
        }

        self.linkgclist_(o, (*o).gclist.as_ptr(), self.gray.as_ptr());
    }

    #[inline(always)]
    unsafe fn mark_one_gray(&self) {
        let o = self.gray.get();

        (*o).marked.set((*o).marked.get() | 1 << 5);

        self.gray.set(*(*o).gclist.as_ptr());

        match (*o).tt {
            5 => self.mark_table(o.cast()),
            6 => self.mark_lf(o.cast()),
            38 => self.mark_rf(o.cast()),
            10 => self.mark_proto(o.cast()),
            8 => self.mark_thread(o.cast()),
            0x1E => unsafe {
                let o = o.cast::<crate::collections::Header<A>>();
                let o = (*o).ptr;

                (*o).mark_items();
            },
            _ => unreachable!(),
        }
    }

    #[inline(never)]
    unsafe fn mark_table(&self, h: *const Table<A>) {
        // Get table mode.
        let mt = (*h).metatable.get();
        let mode = if mt.is_null() {
            null()
        } else if (*mt).flags.get() & 1 << TM_MODE != 0 {
            null()
        } else {
            let s = luaT_gettm(mt, TM_MODE);

            if !s.is_null() && (*s).tt_ == 4 | 0 << 4 | 1 << 6 {
                (*s).value_.gc.cast::<Str<A>>()
            } else {
                null()
            }
        };

        // Mark metatable.
        if !mt.is_null() && (*mt).hdr.marked.is_white() {
            self.mark(mt.cast());
        }

        // Traverse table.
        let (wk, wv) = match mode.as_ref().map(|v| v.as_bytes()) {
            Some(v) => (v.contains(&b'k'), v.contains(&b'v')),
            None => (false, false),
        };

        match (wk, wv) {
            (true, true) => {
                self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.allweak.as_ptr());
            }
            (true, false) => {
                self.mark_ephemeron(h, 0);
            }
            (false, true) => self.mark_weak_value(h),
            (false, false) => self.mark_strong_table(h),
        }
    }

    unsafe fn mark_strong_table(&self, h: *const Table<A>) {
        let mut n = null_mut();
        let limit = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node<A>;
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
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node<A>;
        while n < limit {
            if (*n).tt_ & 0xf == 0 {
                Self::clear_key(n);
            } else {
                if (*n).key_tt & 1 << 6 != 0 && (*(*n).key_val.gc).marked.is_white() {
                    self.mark((*n).key_val.gc);
                }

                if (*n).tt_ & 1 << 6 != 0 && (*(*n).value_.gc).marked.is_white() {
                    self.mark((*n).value_.gc);
                }
            }

            n = n.offset(1);
        }
    }

    unsafe fn mark_weak_value(&self, h: *const Table<A>) {
        let mut n = null_mut();
        let limit = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node<A>;
        let mut hasclears: c_int = ((*h).alimit.get() > 0 as c_int as c_uint) as c_int;
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node<A>;

        while n < limit {
            if (*n).tt_ & 0xf == 0 {
                Self::clear_key(n);
            } else {
                if (*n).key_tt & 1 << 6 != 0 && (*(*n).key_val.gc).marked.is_white() {
                    self.mark((*n).key_val.gc);
                }

                if hasclears == 0
                    && self.is_cleared(if (*n).tt_ & 1 << 6 != 0 {
                        (*n).value_.gc
                    } else {
                        null()
                    })
                {
                    hasclears = 1 as c_int;
                }
            }

            n = n.offset(1);
        }

        if self.state.get() == 2 && hasclears != 0 {
            self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.weak.as_ptr());
        } else {
            self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.grayagain.as_ptr());
        }
    }

    unsafe fn mark_ephemeron(&self, h: *const Table<A>, inv: i32) -> bool {
        let mut marked = false;
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
                marked = true;
                self.mark((*(*h).array.get().offset(i as isize)).value_.gc);
            }

            i = i.wrapping_add(1);
        }

        i = 0 as c_int as c_uint;

        while i < nsize {
            let n = if inv != 0 {
                ((*h).node.get())
                    .offset(nsize.wrapping_sub(1 as c_int as c_uint).wrapping_sub(i) as isize)
                    as *mut Node<A>
            } else {
                ((*h).node.get()).offset(i as isize) as *mut Node<A>
            };

            if (*n).tt_ & 0xf == 0 {
                Self::clear_key(n);
            } else if self.is_cleared(if (*n).key_tt & 1 << 6 != 0 {
                (*n).key_val.gc
            } else {
                null()
            }) {
                hasclears = 1 as c_int;

                if (*n).tt_ & 1 << 6 != 0 && (*(*n).value_.gc).marked.is_white() {
                    hasww = 1 as c_int;
                }
            } else if (*n).tt_ & 1 << 6 != 0 && (*(*n).value_.gc).marked.is_white() {
                marked = true;
                self.mark((*n).value_.gc);
            }

            i = i.wrapping_add(1);
        }

        if self.state.get() == 0 {
            self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.grayagain.as_ptr());
        } else if hasww != 0 {
            self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.ephemeron.as_ptr());
        } else if hasclears != 0 {
            self.linkgclist_(h.cast(), (*h).hdr.gclist.as_ptr(), self.allweak.as_ptr());
        }

        marked
    }

    #[inline(never)]
    unsafe fn mark_lf(&self, cl: *const LuaFn<A>) {
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
    }

    #[inline(never)]
    unsafe fn mark_rf(&self, cl: *const CClosure<A>) {
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
    }

    #[inline(never)]
    unsafe fn mark_proto(&self, f: *const Proto<A>) {
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
            let name = (*(*f).upvalues.offset(i as isize)).name;

            if !name.is_null() && (*name).hdr.marked.is_white() {
                self.mark(name.cast());
            }

            i += 1;
        }

        i = 0 as c_int;

        while i < (*f).sizep {
            let p = *(*f).p.offset(i as isize);

            if !p.is_null() && (*p).hdr.marked.is_white() {
                self.mark(p.cast());
            }

            i += 1;
        }

        i = 0 as c_int;

        while i < (*f).sizelocvars {
            let name = (*(*f).locvars.offset(i as isize)).varname;

            if !name.is_null() && (*name).hdr.marked.is_white() {
                self.mark(name.cast());
            }

            i += 1;
        }
    }

    #[inline(never)]
    unsafe fn mark_thread(&self, th: *const Thread<A>) {
        let mut uv = null_mut();
        let mut o = (*th).stack.get();

        if self.state.get() == 0 {
            self.linkgclist_(
                th.cast(),
                (*th).hdr.gclist.as_ptr(),
                self.grayagain.as_ptr(),
            );

            return;
        } else if o.is_null() {
            return;
        }

        while o < (*th).top.get() {
            if (*o).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*o).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                self.mark((*o).value_.gc);
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

        luaD_shrinkstack(th);

        o = (*th).top.get();

        while o < ((*th).stack_last.get()).offset(5 as c_int as isize) {
            (*o).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
            o = o.offset(1);
        }

        if !((*th).twups.get() != th) && !((*th).openupval.get()).is_null() {
            (*th).twups.set(self.twups.get());
            self.twups.set(th);
        }
    }

    #[inline(never)]
    unsafe fn finish_marking(&self) {
        let grayagain = self.grayagain.replace(null());

        self.state.set(2);

        // Mark object with strong references.
        let mut o = self.refs.get();
        let m = self.marked_refs.replace(null());

        match o.is_null() {
            true => {
                self.refs.set(m);

                if !m.is_null() {
                    (*m).refn.set(self.refs.as_ptr());
                }
            }
            false => loop {
                // Mark.
                if unsafe { (*o).marked.is_white() } {
                    self.mark(o);
                }

                // Move to previous object.
                let p = unsafe { (*o).refp.get() };

                if p.is_null() {
                    // Link refs list.
                    (*o).refp.set(m);

                    if !m.is_null() {
                        (*m).refn.set((*o).refp.as_ptr());
                    }

                    break;
                }

                o = p;
            },
        }

        self.mark_all_gray();
        self.remark_upvalues();
        self.mark_all_gray();

        self.gray.set(grayagain);
        self.mark_all_gray();

        self.converge_ephemerons();
        self.clear_by_values(self.weak.get(), 0 as *mut Object<A>);
        self.clear_by_values(self.allweak.get(), 0 as *mut Object<A>);

        let ow = self.weak.get();
        let oa = self.allweak.get();

        self.mark_all_gray();

        self.converge_ephemerons();
        self.clear_by_keys(self.ephemeron.get());
        self.clear_by_keys(self.allweak.get());
        self.clear_by_values(self.weak.get(), ow);
        self.clear_by_values(self.allweak.get(), oa);

        self.currentwhite
            .set(self.currentwhite.get() ^ (1 << 3 | 1 << 4));
    }

    #[inline(always)]
    unsafe fn mark_all_gray(&self) {
        while !self.gray.get().is_null() {
            self.mark_one_gray();
        }
    }

    #[inline(always)]
    unsafe fn remark_upvalues(&self) {
        let mut p = self.twups.as_ptr();

        loop {
            let th = *p;

            if th.is_null() {
                break;
            }

            if (*th).hdr.marked.get() & (1 << 3 | 1 << 4) == 0 && !(*th).openupval.get().is_null() {
                p = (*th).twups.as_ptr();
            } else {
                let mut uv = (*th).openupval.get();

                *p = (*th).twups.replace(th);

                while !uv.is_null() {
                    let v = (*uv).v.get();

                    if !(*uv).hdr.marked.is_white()
                        && (*v).tt_ & 1 << 6 != 0
                        && (*(*v).value_.gc).marked.is_white()
                    {
                        self.mark((*v).value_.gc);
                    }

                    uv = (*(*uv).u.get()).open.next;
                }
            }
        }
    }

    #[inline(never)]
    unsafe fn converge_ephemerons(&self) {
        let mut changed: c_int = 0;
        let mut dir: c_int = 0 as c_int;

        loop {
            let mut next = self.ephemeron.replace(null());

            changed = 0 as c_int;

            loop {
                let w = next.cast::<Table<A>>();

                if w.is_null() {
                    break;
                }

                next = (*w).hdr.gclist.get();

                (*w).hdr.marked.set((*w).hdr.marked.get() | 1 << 5);

                if self.mark_ephemeron(w, dir) {
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

    #[inline(never)]
    unsafe fn clear_by_keys(&self, mut l: *const Object<A>) {
        while !l.is_null() {
            let h = l as *mut Table<A>;
            let limit = (*h).node.get().add(1 << (*h).lsizenode.get());
            let mut n = (*h).node.get();

            while n < limit {
                if self.is_cleared(if (*n).key_tt & 1 << 6 != 0 {
                    (*n).key_val.gc
                } else {
                    null()
                }) {
                    (*n).tt_ = 0 | 1 << 4;
                }

                if (*n).tt_ & 0xf == 0 {
                    Self::clear_key(n);
                }

                n = n.offset(1);
            }

            l = (*(l as *mut Table<A>)).hdr.gclist.get();
        }
    }

    #[inline(never)]
    unsafe fn clear_by_values(&self, mut l: *const Object<A>, f: *const Object<A>) {
        while l != f {
            let h = l as *mut Table<A>;
            let mut n = null_mut();
            let limit = (*h).node.get().add(1 << (*h).lsizenode.get());
            let asize = luaH_realasize(h);
            let mut i = 0u32;

            while i < asize {
                let o = (*h).array.get().offset(i as isize) as *mut UnsafeValue<A>;
                if self.is_cleared(if (*o).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*o).value_.gc
                } else {
                    null()
                }) {
                    (*o).tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }

                i = i.wrapping_add(1);
            }

            n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node<A>;

            while n < limit {
                if self.is_cleared(if (*n).tt_ & 1 << 6 != 0 {
                    (*n).value_.gc
                } else {
                    null()
                }) {
                    (*n).tt_ = 0 | 1 << 4;
                }

                if (*n).tt_ & 0xf == 0 {
                    Self::clear_key(n);
                }

                n = n.offset(1);
            }

            l = (*(l as *mut Table<A>)).hdr.gclist.get();
        }
    }

    unsafe fn clear_key(n: *mut Node<A>) {
        if (*n).key_tt & 1 << 6 != 0 {
            (*n).key_tt = 11;
        }
    }

    #[inline(never)]
    unsafe fn sweep(&self, mut p: *mut *const Object<A>) -> *mut *const Object<A> {
        // Limit traversal to at most 100 objects per sweep.
        let pw = self.currentwhite.get() ^ (1 << 3 | 1 << 4);
        let cw = self.currentwhite.get() & (1 << 3 | 1 << 4);
        let mut freed = 0;

        for _ in 0..100 {
            // Stop if all debt has been paid.
            if self.debt.get() <= 0 {
                break;
            }

            // Check if no more objects to sweep.
            let o = *p;

            if o.is_null() {
                break;
            }

            // Check if dead.
            let m = (*o).marked.get();

            if m & pw != 0 {
                *p = (*o).next.replace(null());

                self.free(o.cast_mut());
                self.debt.update(|v| v - 1);

                // Lmit at most 2 objects from freeing since Drop implementation might expensive.
                freed += 1;

                if freed == 2 {
                    break;
                }
            } else {
                (*o).marked.set(m & !(1 << 5 | (1 << 3 | 1 << 4) | 7) | cw);

                self.debt.update(|v| v + 1);

                p = (*o).next.as_ptr();
            }
        }

        if (*p).is_null() { null_mut() } else { p }
    }

    #[inline(never)]
    unsafe fn free(&self, o: *mut Object<A>) {
        self.paused.set(true);

        match (*o).tt {
            10 => unsafe {
                core::ptr::drop_in_place(o.cast::<Proto<A>>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Proto<A>>());
            },
            9 => unsafe {
                let o = o.cast::<UpVal<A>>();

                if (*o).v.get() != &raw mut (*(*o).u.get()).value {
                    luaF_unlinkupval(o);
                }

                alloc::alloc::dealloc(o.cast(), Layout::new::<UpVal<A>>());
            },
            6 => unsafe {
                core::ptr::drop_in_place(o.cast::<LuaFn<A>>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<LuaFn<A>>());
            },
            38 => unsafe {
                let cl_0 = o as *mut CClosure<A>;
                let nupvalues = usize::from((*cl_0).nupvalues);
                let size =
                    offset_of!(CClosure<A>, upvalue) + size_of::<UnsafeValue<A>>() * nupvalues;
                let align = align_of::<CClosure<A>>();
                let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

                alloc::alloc::dealloc(cl_0.cast(), layout);
            },
            5 => unsafe {
                core::ptr::drop_in_place(o.cast::<Table<A>>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Table<A>>());
            },
            8 => unsafe {
                core::ptr::drop_in_place(o.cast::<Thread<A>>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<Thread<A>>());
            },
            7 => unsafe {
                let u = o.cast::<UserData<A, ()>>();
                let v = (*u).ptr;
                let layout = Layout::for_value(&*v);
                let layout = Layout::new::<UserData<A, ()>>().extend(layout).unwrap().0;

                core::ptr::drop_in_place(v.cast_mut());
                core::ptr::drop_in_place(u);
                alloc::alloc::dealloc(u.cast(), layout);
            },
            4 => unsafe {
                let ts: *mut Str<A> = o as *mut Str<A>;
                let size = offset_of!(Str<A>, contents) + (*ts).len + 1;
                let align = align_of::<Str<A>>();
                let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

                core::ptr::drop_in_place(ts);
                alloc::alloc::dealloc(ts.cast(), layout);
            },
            0x0E => unsafe {
                core::ptr::drop_in_place(o.cast::<RustId<A>>());
                alloc::alloc::dealloc(o.cast(), Layout::new::<RustId<A>>());
            },
            0x1E => unsafe {
                let o = o.cast::<crate::collections::Header<A>>();
                let v = (*o).ptr.cast_mut();
                let layout = Layout::for_value(&*v);

                core::ptr::drop_in_place(v);
                alloc::alloc::dealloc(v.cast(), layout);
            },
            0x0F => unsafe {
                let p = self.sweep_mark.replace(o).cast_mut();

                if !p.is_null() {
                    core::ptr::drop_in_place(p);
                    alloc::alloc::dealloc(p.cast(), Layout::new::<Object<A>>());
                }
            },
            _ => unreachable!(),
        }

        self.paused.set(false);
    }

    #[inline(always)]
    unsafe fn linkgclist_(
        &self,
        o: *const Object<A>,
        pnext: *mut *const Object<A>,
        list: *mut *const Object<A>,
    ) {
        *pnext = *list;
        *list = o;

        (*o).marked.set_gray();
    }

    unsafe fn is_cleared(&self, o: *const Object<A>) -> bool {
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

impl<D> Drop for Gc<D> {
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
            unsafe { alloc::alloc::dealloc(m.cast(), Layout::new::<Object<D>>()) };
        }
    }
}
