use super::Mark;
use crate::Lua;
use alloc::alloc::handle_alloc_error;
use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::null;

/// Header of all object managed by Garbage Collector.
///
/// All object must have this struct at the beginning of its memory block.
pub struct Object {
    pub(crate) global: *const Lua,
    pub(super) next: Cell<*const Object>,
    pub(crate) tt: u8,
    pub(crate) marked: Mark,
    pub(super) refs: Cell<usize>,
    pub(super) refn: Cell<*const Object>,
    pub(super) refp: Cell<*const Object>,
    pub(super) gclist: Cell<*const Object>,
}

impl Object {
    /// # Safety
    /// `layout` must have the layout of [`Object`] at the beginning.
    pub unsafe fn new(g: *const Lua, tt: u8, layout: Layout) -> *mut Object {
        let g = &*g;
        let o = unsafe { alloc::alloc::alloc(layout) as *mut Object };

        if o.is_null() {
            handle_alloc_error(layout);
        }

        o.write(Object {
            global: g,
            next: Cell::new(g.all.get()),
            tt,
            marked: Mark::new(g.currentwhite.get() & (1 << 3 | 1 << 4)),
            refs: Cell::new(0),
            refn: Cell::new(null()),
            refp: Cell::new(null()),
            gclist: Cell::new(null()),
        });

        g.all.set(o);
        g.gc.debt
            .set(g.gc.debt.get().checked_add_unsigned(layout.size()).unwrap());

        o
    }

    #[inline(always)]
    pub fn global(&self) -> &Lua {
        unsafe { &*self.global }
    }
}

impl Default for Object {
    #[inline(always)]
    fn default() -> Self {
        Self {
            global: null(),
            next: Cell::new(null()),
            tt: 0,
            marked: Mark::default(),
            refs: Cell::new(0),
            refn: Cell::new(null()),
            refp: Cell::new(null()),
            gclist: Cell::new(null()),
        }
    }
}
