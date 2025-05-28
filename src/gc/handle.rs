use crate::Lua;
use crate::lobject::GCObject;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::null_mut;
use std::rc::Rc;

/// RAII struct to prevent the value from GC.
pub struct Handle<T> {
    lua: Pin<Rc<Lua>>,
    obj: *mut T,
}

impl<T> Handle<T> {
    #[inline(always)]
    pub(crate) unsafe fn new(lua: Pin<Rc<Lua>>, obj: *mut T) -> Self {
        // Allocate a handle.
        let b = obj as *mut GCObject;

        if (*b).refs == 0 {
            (*b).handle = Self::alloc_handle(&lua, b);
        }

        // Increase references.
        (*b).refs = (*b).refs.checked_add(1).unwrap();

        Self { lua, obj }
    }

    #[inline(never)]
    fn alloc_handle(lua: &Lua, obj: *mut GCObject) -> usize {
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
        let b = self.obj as *mut GCObject;
        let h = (*b).handle;

        debug_assert_eq!(std::mem::replace(&mut t[h], null_mut()), b);

        self.lua.handle_free.borrow_mut().push(h);
    }
}

impl<T> Drop for Handle<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let b = self.obj as *mut GCObject;

        unsafe { (*b).refs -= 1 };

        if unsafe { (*b).refs == 0 } {
            unsafe { self.free_handle() };
        }
    }
}

impl<T> Clone for Handle<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let b = self.obj as *mut GCObject;

        unsafe { (*b).refs = (*b).refs.checked_add(1).unwrap() };

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
