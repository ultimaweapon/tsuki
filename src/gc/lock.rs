use super::Gc;

/// RAII struct to prevent GC from running.
pub struct GcLock<'a, D>(&'a Gc<D>);

impl<'a, D> GcLock<'a, D> {
    #[inline(always)]
    pub(super) fn new(gc: &'a Gc<D>) -> Self {
        gc.locks.update(|v| v.checked_add(1).unwrap());

        Self(gc)
    }
}

impl<'a, D> Drop for GcLock<'a, D> {
    #[inline(always)]
    fn drop(&mut self) {
        self.0.locks.update(|v| v - 1);

        if self.0.locks.get() == 0 {
            self.0.step();
        }
    }
}
