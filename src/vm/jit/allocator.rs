use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::mem::transmute;
use core::num::NonZero;
use core::ops::{Deref, DerefMut};
use core::ptr::null_mut;
use std::io::Error;

/// Manage executable pages for jitted functions.
pub struct CodeAllocator {
    free: Vec<BTreeSet<(NonZero<usize>, *mut u8)>>,
    page_size: NonZero<usize>,
}

impl CodeAllocator {
    pub fn new() -> Self {
        Self {
            free: Vec::new(),
            page_size: Self::get_page_size(),
        }
    }

    /// # Safety
    /// This methos will changes page protection to non-executable.
    ///
    /// # Panics
    /// - `align` is not power of 2.
    /// - `align` is larger than system page size.
    /// - `size` is overflow when rounded to page size.
    pub unsafe fn allocate(
        &mut self,
        size: NonZero<usize>,
        align: NonZero<usize>,
    ) -> Result<WritableBlock<'_>, Error> {
        assert!(align.is_power_of_two());
        assert!(align <= self.page_size);

        // Check if pool available.
        let pool = u8::try_from(align.trailing_zeros()).unwrap();

        if usize::from(pool) >= self.free.len() {
            self.free.resize_with(usize::from(pool) + 1, BTreeSet::new);
        }

        // Find from allocated pages.
        let free = &mut self.free[usize::from(pool)];

        if let Some(&(bs, lb)) = free.range((size, null_mut())..).next() {
            // Set pages to writable.
            let bm = lb.add(bs.get()).cast::<BlockMeta>();
            let pages = (*bm).pages;

            Self::set_writable(pages).unwrap();

            assert!(free.remove(&(bs, lb)));

            // Check if block size and the requested size is exactly match.
            if (bs.get() - size.get()) == 0 {
                (*bm).active = true;

                return Ok(WritableBlock {
                    pages,
                    ptr: lb,
                    size,
                    phantom: PhantomData,
                });
            }

            // Get location for right block.
            let lm = lb.add(size.get());
            let le = lm.add(size_of::<BlockMeta>());
            let off = le.align_offset(align.get());
            let rb = le.add(off);

            // Move BlockMeta for left block.
            let lm = lm.cast();
            let bm = bm.read();
            let pb = bm.prev;
            let nb = bm.next;

            if !pb.is_null() {
                (*pb).next = lm;
            }

            lm.write(bm);

            (*lm).size = size;

            // Check if space is enough to split the block.
            let limit = lb.add(bs.get()).add(size_of::<BlockMeta>()).add(bm.waste);
            let rm = limit.sub(size_of::<BlockMeta>());
            let rs = match rm.offset_from(rb).try_into().ok().and_then(NonZero::new) {
                Some(v) => v,
                None => {
                    // Take the whole block.
                    if !nb.is_null() {
                        (*nb).prev = lm;
                    }

                    (*lm).active = true;
                    (*lm).waste = limit.offset_from_unsigned(le);

                    return Ok(WritableBlock {
                        pages,
                        ptr: lb,
                        size,
                        phantom: PhantomData,
                    });
                }
            };

            // Create left block.
            (*lm).next = rm.cast();
            (*lm).active = true;
            (*lm).waste = rb.offset_from_unsigned(le);

            // Create right block.
            let rm = rm.cast();

            if !nb.is_null() {
                (*nb).prev = rm;
            }

            rm.write(bm);

            (*rm).prev = lm;
            (*rm).size = rs;
            (*rm).waste = 0;

            assert!(free.insert((rs, rb)));

            return Ok(WritableBlock {
                pages,
                ptr: lb,
                size,
                phantom: PhantomData,
            });
        }

        // Allocate new pages.
        let len = size
            .checked_add(size_of::<BlockMeta>())
            .and_then(|v| v.get().checked_next_multiple_of(self.page_size.get()))
            .unwrap()
            .try_into()
            .unwrap();
        let pages = Self::allocate_pages(len)?;
        let lb = pages;
        let limit = lb.add(len.get());
        let pages = core::ptr::slice_from_raw_parts_mut(pages, len.get());

        // Check if waste space is enough for another block.
        let lm = lb.add(size.get());
        let le = lm.add(size_of::<BlockMeta>());
        let off = le.align_offset(align.get());
        let rb = le.add(off);
        let rm = limit.sub(size_of::<BlockMeta>());
        let lm = lm.cast::<BlockMeta>();
        let (next, waste) = match rm.offset_from(rb).try_into().ok().and_then(NonZero::new) {
            Some(rs) => {
                let rm = rm.cast::<BlockMeta>();

                rm.write(BlockMeta {
                    pages,
                    prev: lm,
                    next: null_mut(),
                    size: rs,
                    waste: 0,
                    active: false,
                    pool,
                });

                assert!(free.insert((rs, rb)));

                (rm, rb.offset_from_unsigned(le))
            }
            None => (null_mut(), limit.offset_from_unsigned(le)),
        };

        lm.write(BlockMeta {
            pages,
            prev: null_mut(),
            next,
            size,
            waste,
            active: true,
            pool,
        });

        Ok(WritableBlock {
            pages,
            ptr: lb,
            size,
            phantom: PhantomData,
        })
    }

    pub unsafe fn deallocate(&mut self, ptr: *mut [u8]) {
        // Change pages to writable.
        let Slice(mb, ms) = transmute(ptr);
        let mm = mb.add(ms).cast::<BlockMeta>();
        let pages = (*mm).pages;

        Self::set_writable(pages).unwrap();

        // Join with the previous blocks.
        let free = &mut self.free[usize::from((*mm).pool)];
        let mut size = (*mm).size;
        let mut pm = (*mm).prev;

        while !pm.is_null() && !(*pm).active {
            let ps = (*pm).size;
            let pb = pm.cast::<u8>().sub(ps.get());

            assert!(free.remove(&(ps, pb)));

            size = size
                .checked_add(ps.get())
                .and_then(|v| v.checked_add(size_of::<BlockMeta>()))
                .and_then(move |v| v.checked_add((*pm).waste))
                .unwrap();

            pm = (*pm).prev;
        }

        // Join with next blocks.
        let lb = mm.cast::<u8>().sub(size.get());
        let mut nm = (*mm).next;

        while !nm.is_null() && !(*nm).active {
            let ns = (*nm).size;
            let nb = nm.cast::<u8>().sub(ns.get());

            assert!(free.remove(&(ns, nb)));

            size = size
                .checked_add(ns.get())
                .and_then(|v| v.checked_add(size_of::<BlockMeta>()))
                .and_then(move |v| v.checked_add((*nm).waste))
                .unwrap();

            nm = (*nm).next;
        }

        size = size.checked_add((*mm).waste).unwrap();

        // Check if no more active blocks.
        if pm.is_null() && nm.is_null() {
            Self::free_pages(pages).unwrap();
        } else {
            let mm = mm.read();
            let lm = lb.add(size.get()).cast::<BlockMeta>();

            lm.write(mm);

            (*lm).active = false;
            (*lm).prev = pm;
            (*lm).next = nm;
            (*lm).size = size;
            (*lm).waste = 0;

            assert!(free.insert((size, lb)));

            Self::clear_writable(pages).unwrap();
        }
    }

    #[cfg(unix)]
    fn allocate_pages(len: NonZero<usize>) -> Result<*mut u8, Error> {
        let prot = libc::PROT_READ | libc::PROT_WRITE;
        let flags = libc::MAP_ANON | libc::MAP_PRIVATE;
        let ptr = unsafe { libc::mmap(null_mut(), len.get(), prot, flags, -1, 0) };

        if ptr == libc::MAP_FAILED {
            Err(Error::last_os_error())
        } else {
            Ok(ptr.cast())
        }
    }

    #[cfg(unix)]
    unsafe fn free_pages(pages: *mut [u8]) -> Result<(), Error> {
        let Slice(ptr, len) = transmute(pages);

        if unsafe { libc::munmap(ptr.cast(), len) < 0 } {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(unix)]
    unsafe fn set_writable(pages: *mut [u8]) -> Result<(), Error> {
        let Slice(ptr, len) = transmute(pages);

        if unsafe { libc::mprotect(ptr.cast(), len, libc::PROT_READ | libc::PROT_WRITE) < 0 } {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(unix)]
    unsafe fn clear_writable(pages: *mut [u8]) -> Result<(), Error> {
        let Slice(ptr, len) = transmute(pages);

        if unsafe { libc::mprotect(ptr.cast(), len, libc::PROT_READ | libc::PROT_EXEC) < 0 } {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(unix)]
    fn get_page_size() -> NonZero<usize> {
        let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };

        usize::try_from(v).unwrap().try_into().unwrap()
    }
}

/// Contains information for allocated block.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct BlockMeta {
    pages: *mut [u8],
    prev: *mut Self,
    next: *mut Self,
    size: NonZero<usize>,
    waste: usize,
    active: bool,
    pool: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Slice(*mut u8, usize);

/// RAII struct to restore page protection.
pub struct WritableBlock<'a> {
    pages: *mut [u8],
    ptr: *mut u8,
    size: NonZero<usize>,
    phantom: PhantomData<&'a CodeAllocator>,
}

impl<'a> Drop for WritableBlock<'a> {
    fn drop(&mut self) {
        unsafe { CodeAllocator::clear_writable(self.pages).unwrap() };
    }
}

impl<'a> Deref for WritableBlock<'a> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr, self.size.get()) }
    }
}

impl<'a> DerefMut for WritableBlock<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.size.get()) }
    }
}
