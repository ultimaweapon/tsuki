use super::Object;
use crate::Lua;
use alloc::rc::Rc;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null;

/// Strong reference to Lua object.
///
/// The value of this struct will prevent Garbage Collector from collect the encapsulated value. It
/// also prevent [`Lua`] from dropping.
///
/// Beware for memory leak if the value of this type owned by Lua (e.g. put it in a table). If this
/// value has a reference to its parent (either directly or indirectly) it will prevent GC from
/// collect the parent, which in turn prevent this value from dropped.
pub struct Ref<T, D> {
    g: Pin<Rc<Lua<D>>>,
    o: *const T,
}

impl<T, D> Ref<T, D> {
    pub(crate) unsafe fn new(o: *const T) -> Self {
        let h = o.cast::<Object<D>>();
        let g = (*h).global();
        let r = (*h).refs.get();

        if r == 0 {
            let p = g.gc.refs.get();

            (*h).refs.set(1);
            (*h).refp.set(p);

            if !p.is_null() {
                (*p).refn.set(h);
            }

            g.gc.refs.set(h);
        } else if let Some(v) = r.checked_add(1) {
            (*h).refs.set(v);
        } else {
            Self::too_many_refs();
        }

        Self { g: g.to_rc(), o }
    }

    #[cold]
    #[inline(never)]
    fn too_many_refs() -> ! {
        panic!("too many strong references to Lua object");
    }
}

impl<T, D> Drop for Ref<T, D> {
    #[inline(always)]
    fn drop(&mut self) {
        // Decrease references.
        let h = self.o.cast::<Object<D>>();

        unsafe { (*h).refs.set((*h).refs.get() - 1) };

        if unsafe { (*h).refs.get() != 0 } {
            return;
        }

        // Remove from list.
        let n = unsafe { (*h).refn.replace(null()) };
        let p = unsafe { (*h).refp.replace(null()) };

        if !n.is_null() {
            unsafe { (*n).refp.set(p) };
        }

        if !p.is_null() {
            unsafe { (*p).refn.set(n) };
        }

        if self.g.gc.refs.get() == h {
            self.g.gc.refs.set(p);
        }
    }
}

impl<T, D> Clone for Ref<T, D> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let h = self.o.cast::<Object<D>>();

        match unsafe { (*h).refs.get().checked_add(1) } {
            Some(v) => unsafe { (*h).refs.set(v) },
            None => Self::too_many_refs(),
        }

        Self {
            g: self.g.clone(),
            o: self.o,
        }
    }
}

impl<T, D> Deref for Ref<T, D> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.o }
    }
}
