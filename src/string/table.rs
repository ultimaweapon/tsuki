use super::Str;
use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::null;
use std::alloc::handle_alloc_error;

/// Hash table contains all allocated short strings.
pub(crate) struct StringTable {
    hash: Cell<*mut *const Str>,
    nuse: Cell<usize>,
    size: Cell<usize>,
}

impl StringTable {
    pub fn new() -> Self {
        let size = 128;
        let layout = Layout::array::<*const Str>(size).unwrap();
        let hash = unsafe { alloc::alloc::alloc_zeroed(layout) };

        if hash.is_null() {
            handle_alloc_error(layout);
        }

        Self {
            hash: Cell::new(hash.cast()),
            nuse: Cell::new(0),
            size: Cell::new(size),
        }
    }

    /// # Safety
    /// The returned [`Str`] may already dead but not collected yet so the caller must check and
    /// resurrect it.
    pub(super) unsafe fn insert(&self, h: u32, str: &[u8]) -> Result<*const Str, *mut *const Str> {
        // Check if same string exists.
        let mut e = self.entry(h);
        let mut s = unsafe { *e };

        while !s.is_null() {
            if unsafe { (*s).as_bytes() == str } {
                return Ok(s);
            }

            s = unsafe { (*(*s).u.get()).hnext };
        }

        // Check if we need to increase table size.
        if self.nuse >= self.size {
            self.resize(self.size.get().checked_mul(2).unwrap());
            e = self.entry(h);
        }

        self.nuse.set(self.nuse.get() + 1);

        Err(e)
    }

    pub(super) unsafe fn remove(&self, s: *mut Str) {
        let mut e = self.entry(unsafe { (*s).hash.get() });

        while unsafe { *e != s } {
            e = unsafe { &raw mut (*(**e).u.get()).hnext };
        }

        unsafe { *e = (*(**e).u.get()).hnext };
        self.nuse.set(self.nuse.get() - 1);
    }

    #[inline(always)]
    fn entry(&self, h: u32) -> *mut *const Str {
        let t = self.hash.get();

        unsafe { t.add(usize::try_from(h).unwrap() & (self.size.get() - 1)) }
    }

    fn resize(&self, nsize: usize) {
        // Move all items out of the area to shrink.
        let table = self.hash.get();
        let osize = self.size.get();

        if nsize < osize {
            unsafe { Self::rehash(table, osize, nsize) };
        }

        // Re-allocate.
        let layout = Layout::array::<*const Str>(osize).unwrap();
        let newvect =
            unsafe { alloc::alloc::realloc(table.cast(), layout, nsize * size_of::<*const Str>()) };

        if newvect.is_null() {
            handle_alloc_error(Layout::array::<*const Str>(nsize).unwrap());
        }

        self.hash.set(newvect.cast());
        self.size.set(nsize);

        // Spread all items to new area.
        if nsize > osize {
            unsafe { Self::rehash(self.hash.get(), osize, nsize) };
        }
    }

    unsafe fn rehash(vect: *mut *const Str, osize: usize, nsize: usize) {
        // Fill new space with null.
        for i in osize..nsize {
            unsafe { vect.add(i).write(null()) };
        }

        // Rehash all items.
        for i in 0..osize {
            let mut p = unsafe { vect.add(i).replace(null()) };

            while !p.is_null() {
                let hnext = unsafe { (*(*p).u.get()).hnext };
                let h = usize::try_from(unsafe { (*p).hash.get() }).unwrap() & (nsize - 1);

                unsafe { (*(*p).u.get()).hnext = *vect.add(h) };
                unsafe { vect.add(h).write(p) };

                p = hnext;
            }
        }
    }
}

impl Drop for StringTable {
    fn drop(&mut self) {
        let layout = Layout::array::<*const Str>(self.size.get()).unwrap();

        unsafe { alloc::alloc::dealloc(self.hash.get().cast(), layout) };
    }
}
