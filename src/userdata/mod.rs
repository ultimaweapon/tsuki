use crate::gc::Object;
use crate::value::UnsafeValue;
use crate::{Lua, Nil, Table, luaH_getid};
use core::alloc::Layout;
use core::any::{Any, TypeId};
use core::marker::PhantomData;
use core::mem::transmute;
use core::ptr::{addr_of_mut, null};

/// Lua full userdata.
///
/// Use [Lua::create_ud()] or [Context::create_ud()](crate::Context::create_ud()) to create the
/// value of this type.
#[repr(C)]
pub struct UserData<D, T: ?Sized> {
    pub(crate) hdr: Object<D>,
    pub(crate) mt: *const Table<D>,
    pub(crate) uv: UnsafeValue<D>,
    pub(crate) ptr: *const dyn Any,
    phantom: PhantomData<T>,
}

impl<D, T: Any> UserData<D, T> {
    #[inline(never)]
    pub(crate) unsafe fn new(g: *const Lua<D>, value: T) -> *const UserData<D, ()> {
        // Get layout.
        let layout = Layout::new::<T>();
        let (layout, offset) = Layout::new::<UserData<D, ()>>().extend(layout).unwrap();
        let layout = layout.pad_to_align();

        // Load metatable before construct an incomplete object.
        let id = TypeId::of::<T>();
        let mt = unsafe { luaH_getid((*g).metatables(), &id) };
        let mt = match unsafe { (*mt).tt_ & 0xf } {
            5 => unsafe { (*mt).value_.gc.cast::<Table<D>>() },
            _ => null(),
        };

        // Create object.
        let o = unsafe { (*g).gc.alloc(7 | 0 << 4, layout).cast::<UserData<D, ()>>() };

        unsafe { addr_of_mut!((*o).mt).write(mt) };
        unsafe { addr_of_mut!((*o).uv).write(Nil.into()) };

        // Encapsulate value.
        let v = unsafe { o.byte_add(offset).cast::<T>() };

        unsafe { v.write(value) };

        // Write Ayn pointer.
        let v: &dyn Any = unsafe { &*v };

        unsafe { addr_of_mut!((*o).ptr).write(v) };

        o
    }

    /// Returns a reference to the encapsulated value.
    #[inline(always)]
    pub fn value(&self) -> &T {
        unsafe { &*self.ptr.cast() }
    }
}

impl<D> UserData<D, dyn Any> {
    /// Returns `true` if the encapsulated value is `T`.
    #[inline(always)]
    pub fn is<T: Any>(&self) -> bool {
        unsafe { (*self.ptr).is::<T>() }
    }

    #[inline(always)]
    pub fn downcast<T: Any>(&self) -> Option<&UserData<D, T>> {
        match self.is::<T>() {
            true => Some(unsafe { transmute(self) }),
            false => None,
        }
    }
}
