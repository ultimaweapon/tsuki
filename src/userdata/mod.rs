pub use self::class::*;

use self::property::PropertyKey;
use crate::gc::Object;
use crate::value::UnsafeValue;
use crate::{Lua, Nil, Table, Value, luaH_getid};
use core::alloc::Layout;
use core::any::{Any, TypeId};
use core::cell::Cell;
use core::marker::{PhantomData, PhantomPinned};
use core::mem::transmute;
use core::ptr::{addr_of_mut, null};

mod class;
mod property;

/// Lua full userdata.
///
/// Use [Lua::create_ud()] or [Context::create_ud()](crate::Context::create_ud()) to create the
/// value of this type.
///
/// # Types that implement [PropertyKey]
///
/// You can pass a value of the following types for property name:
///
/// - Reference to [str].
#[repr(C)]
pub struct UserData<A, T: ?Sized> {
    pub(crate) hdr: Object<A>,
    pub(crate) props: Cell<*const Table<A>>,
    pub(crate) mt: *const Table<A>,
    pub(crate) uv: UnsafeValue<A>,
    pub(crate) ptr: *const dyn Any,
    phantom: PhantomData<T>,
    pin: PhantomPinned,
}

impl<A, T: Any> UserData<A, T> {
    #[inline(never)]
    pub(crate) unsafe fn new(g: *const Lua<A>, value: T) -> *const UserData<A, ()> {
        // Get layout.
        let layout = Layout::new::<T>();
        let (layout, offset) = Layout::new::<UserData<A, ()>>().extend(layout).unwrap();
        let layout = layout.pad_to_align();

        // Load metatable before construct an incomplete object.
        let id = TypeId::of::<T>();
        let mt = unsafe { luaH_getid((*g).metatables(), &id) };
        let mt = match unsafe { (*mt).tt_ & 0xf } {
            5 => unsafe { (*mt).value_.gc.cast::<Table<A>>() },
            _ => null(),
        };

        // Create object.
        let o = unsafe { (*g).gc.alloc(7 | 0 << 4, layout).cast::<UserData<A, ()>>() };

        unsafe { addr_of_mut!((*o).props).write(Cell::new(null())) };
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

impl<A, T: ?Sized> UserData<A, T> {
    /// Gets a property of this userdata.
    ///
    /// See [UserData] for a list of supported value on `name`.
    #[inline(always)]
    pub fn get<K: PropertyKey>(&self, name: K) -> Value<'_, A> {
        let props = self.props.get();
        let value = match props.is_null() {
            true => return Value::Nil,
            false => unsafe { name.get(props) },
        };

        unsafe { Value::from_unsafe(value) }
    }

    /// Sets a property of this userdata.
    ///
    /// Once this method is called the index operation on this userdata no longer report an error
    /// even if this userdata does not have a metatable or no `__index` on its metatable.
    ///
    /// See [UserData] for a list of supported value on `name`.
    ///
    /// # Panics
    /// If `value` was created from different [Lua](crate::Lua) instance.
    #[inline(never)]
    pub fn set<K: PropertyKey>(&self, name: K, value: impl Into<UnsafeValue<A>>) {
        // Check if value was created from the same Lua.
        let value = value.into();

        if unsafe { (value.tt_ & 1 << 6 != 0) && (*value.value_.gc).global != self.hdr.global } {
            panic!("attempt to set a property with value from a different Lua");
        }

        // Set property.
        let mut props = self.props.get();

        if props.is_null() {
            let g = self.hdr.global();

            props = unsafe { Table::new(g) };

            if self.hdr.marked.get() & 1 << 5 != 0 {
                unsafe { g.gc.barrier(&self.hdr, props.cast()) };
            }

            self.props.set(props);
        }

        unsafe { name.set(props, value) };
    }
}

impl<A> UserData<A, dyn Any> {
    /// Returns `true` if the encapsulated value is `T`.
    #[inline(always)]
    pub fn is<T: Any>(&self) -> bool {
        unsafe { (*self.ptr).is::<T>() }
    }

    /// Attempts to downcast the userdata to a concrete type.
    #[inline(always)]
    pub fn downcast<T: Any>(&self) -> Option<&UserData<A, T>> {
        match self.is::<T>() {
            true => Some(unsafe { transmute(self) }),
            false => None,
        }
    }
}
