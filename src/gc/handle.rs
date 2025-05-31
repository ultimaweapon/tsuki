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
    lua: Pin<Rc<Lua>>,
    obj: *mut T,
}

impl<T> Handle<T> {
    #[inline(always)]
    pub(crate) unsafe fn new(lua: Pin<Rc<Lua>>, obj: *mut T) -> Self {
        // Allocate a handle.
        let b = obj as *mut Object;

        if (*b).refs.get() == 0 {
            (*b).handle.set(Self::alloc_handle(&lua, b));
        }

        // Increase references.
        (*b).refs.set((*b).refs.get().checked_add(1).unwrap());

        Self { lua, obj }
    }

    #[inline(never)]
    fn alloc_handle(lua: &Lua, obj: *mut Object) -> usize {
        let mut t = lua.handle_table.borrow_mut();

        match lua.handle_free.borrow_mut().pop() {
            Some(h) => {
                debug_assert!(std::mem::replace(&mut t[h], obj).is_null());
                h
            }
            None => {
                let h = t.len();
                t.push(obj);
                h
            }
        }
    }

    #[inline(never)]
    unsafe fn free_handle(&mut self) {
        let mut t = self.lua.handle_table.borrow_mut();
        let b = self.obj as *mut Object;
        let h = (*b).handle.get();

        if h == t.len() - 1 {
            t.pop();
        } else {
            debug_assert_eq!(std::mem::replace(&mut t[h], null_mut()), b);
            self.lua.handle_free.borrow_mut().push(h);
        }
    }
}

impl<T> Drop for Handle<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let b = self.obj as *mut Object;

        unsafe { (*b).refs.set((*b).refs.get() - 1) };

        if unsafe { (*b).refs.get() == 0 } {
            unsafe { self.free_handle() };
        }
    }
}

impl<T> Clone for Handle<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let b = self.obj as *mut Object;

        unsafe { (*b).refs.set((*b).refs.get().checked_add(1).unwrap()) };

        Self {
            lua: self.lua.clone(),
            obj: self.obj,
        }
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.obj }
    }
}
