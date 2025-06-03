use super::Mark;
use crate::Lua;
use std::alloc::{Layout, handle_alloc_error};
use std::cell::Cell;
use std::ptr::null;

/// Header of all object managed by Garbage Collector.
///
/// All object must have this struct at the beginning of its memory block.
pub(crate) struct Object {
    pub(super) next: Cell<*const Object>,
    pub tt: u8,
    pub marked: Mark,
    pub refs: Cell<usize>,
    pub refn: Cell<*const Object>,
    pub refp: Cell<*const Object>,
    pub gclist: Cell<*const Object>,
}

impl Object {
    /// # Safety
    /// `layout` must have the layout of [`Object`] at the beginning.
    pub unsafe fn new(g: *const Lua, tt: u8, layout: Layout) -> *mut Object {
        let g = &*g;
        let o = unsafe { std::alloc::alloc(layout) as *mut Object };

        if o.is_null() {
            handle_alloc_error(layout);
        }

        o.write(Object {
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
}

impl Default for Object {
    #[inline(always)]
    fn default() -> Self {
        Self {
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
