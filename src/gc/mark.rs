use core::cell::Cell;

/// Mark on each GC object for identify its collectable state.
#[derive(Default)]
pub(crate) struct Mark(Cell<u8>);

impl Mark {
    #[inline(always)]
    pub unsafe fn new(v: u8) -> Self {
        Self(Cell::new(v))
    }

    #[inline(always)]
    pub fn get(&self) -> u8 {
        self.0.get()
    }

    #[inline(always)]
    pub unsafe fn set(&self, v: u8) {
        self.0.set(v);
    }

    #[inline(always)]
    pub unsafe fn set_gray(&self) {
        self.0.set(self.0.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
    }

    #[inline(always)]
    pub fn is_white(&self) -> bool {
        (self.0.get() & (1 << 3 | 1 << 4)) != 0
    }
}
