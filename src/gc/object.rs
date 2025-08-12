use super::Mark;
use crate::Lua;
use core::cell::Cell;
use core::ptr::null;

/// Header of all object managed by Garbage Collector.
///
/// All object must have this struct at the beginning of its memory block.
pub struct Object<D> {
    pub(crate) global: *const Lua<D>,
    pub(super) next: Cell<*const Self>,
    pub(crate) tt: u8,
    pub(crate) marked: Mark,
    pub(super) refs: Cell<usize>,
    pub(super) refn: Cell<*const Self>,
    pub(super) refp: Cell<*const Self>,
    pub(super) gclist: Cell<*const Self>,
}

impl<D> Object<D> {
    #[inline(always)]
    pub fn global(&self) -> &Lua<D> {
        unsafe { &*self.global }
    }
}

impl<D> Default for Object<D> {
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
