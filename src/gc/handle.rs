use super::Object;
use crate::Lua;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::null_mut;
use std::rc::Rc;

/// RAII struct to prevent the value from GC.
///
/// Beware for memory leak if the value of this type owned by Lua (e.g. put it in a table). If this
/// value has a reference to its parent (either directly or indirectly) it will prevent GC from
/// collect the parent, which also prevent the reference to [`Lua`] from reduce to zero since
/// [`Handle`] also have a strong reference to [`Lua`].
pub struct Handle<T> {
    g: Pin<Rc<Lua>>,
    o: *const T,
}

impl<T> Handle<T> {
    #[inline(always)]
    pub(crate) unsafe fn new(g: Pin<Rc<Lua>>, o: *const T) -> Self {
        let b = o as *const Object;

        if (*b).refs.get() == 0 {
            (*b).handle.set(Self::alloc_handle(&g, b));
            (*b).refs.set(1);
        } else {
            (*b).refs.set((*b).refs.get().checked_add(1).unwrap());
        }

        Self { g, o }
    }

    #[inline(never)]
    fn alloc_handle(g: &Lua, o: *const Object) -> usize {
        let mut t = g.handle_table.borrow_mut();

        match g.handle_free.borrow_mut().pop() {
            Some(h) => {
                debug_assert!(std::mem::replace(&mut t[h], o).is_null());
                h
            }
            None => {
                let h = t.len();
                t.push(o);
                h
            }
        }
    }

    #[inline(never)]
    unsafe fn free_handle(&mut self) {
        let mut t = self.g.handle_table.borrow_mut();
        let o = self.o as *const Object;
        let h = (*o).handle.get();

        if h == t.len() - 1 {
            t.pop();
        } else {
            debug_assert_eq!(std::mem::replace(&mut t[h], null_mut()), o);
            self.g.handle_free.borrow_mut().push(h);
        }
    }
}

impl<T> Drop for Handle<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let o = self.o as *const Object;

        unsafe { (*o).refs.set((*o).refs.get() - 1) };

        if unsafe { (*o).refs.get() == 0 } {
            unsafe { self.free_handle() };
        }
    }
}

impl<T> Clone for Handle<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let b = self.o as *const Object;

        unsafe { (*b).refs.set((*b).refs.get().checked_add(1).unwrap()) };

        Self {
            g: self.g.clone(),
            o: self.o,
        }
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.o }
    }
}
