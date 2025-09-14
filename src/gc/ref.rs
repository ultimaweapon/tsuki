use super::Object;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::null;

/// Strong reference to Lua object.
///
/// The value of this struct will prevent Garbage Collector from collect the encapsulated value.
pub struct Ref<'a, T> {
    obj: *const T,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> Ref<'a, T> {
    #[inline(never)]
    pub(crate) unsafe fn new(o: *const T) -> Self {
        let h = o.cast::<Object<()>>();
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

        Self {
            obj: o,
            phantom: PhantomData,
        }
    }

    #[cold]
    #[inline(never)]
    fn too_many_refs() -> ! {
        panic!("too many strong references to Lua object");
    }
}

impl<'a, T> Drop for Ref<'a, T> {
    #[inline(never)]
    fn drop(&mut self) {
        // Decrease references.
        let h = self.obj.cast::<Object<()>>();

        unsafe { (*h).refs.set((*h).refs.get() - 1) };

        if unsafe { (*h).refs.get() != 0 } {
            return;
        }

        // Remove from list.
        let g = unsafe { (*h).global() };
        let n = unsafe { (*h).refn.replace(null()) };
        let p = unsafe { (*h).refp.replace(null()) };

        if !n.is_null() {
            unsafe { (*n).refp.set(p) };
        }

        if !p.is_null() {
            unsafe { (*p).refn.set(n) };
        }

        if g.gc.refs.get() == h {
            g.gc.refs.set(p);
        }
    }
}

impl<'a, T> Clone for Ref<'a, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let h = self.obj.cast::<Object<()>>();

        match unsafe { (*h).refs.get().checked_add(1) } {
            Some(v) => unsafe { (*h).refs.set(v) },
            None => Self::too_many_refs(),
        }

        Self {
            obj: self.obj,
            phantom: PhantomData,
        }
    }
}

impl<'a, T> Deref for Ref<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.obj }
    }
}

impl<'a, T> PartialEq<str> for Ref<'a, T>
where
    T: PartialEq<str>,
{
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        PartialEq::eq(self.deref(), other)
    }
}
