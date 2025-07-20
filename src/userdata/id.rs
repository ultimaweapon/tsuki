use crate::Lua;
use crate::gc::Object;
use core::alloc::Layout;
use core::any::TypeId;
use core::ptr::addr_of_mut;

/// Encapsulates [`TypeId`] of a userdata.
#[repr(C)]
pub(crate) struct UserId {
    hdr: Object,
    value: TypeId,
}

impl UserId {
    pub unsafe fn new(g: *const Lua, value: TypeId) -> *const Self {
        let layout = Layout::new::<Self>();
        let o = unsafe { Object::new(g, 11 | 0 << 4, layout).cast::<Self>() };

        unsafe { addr_of_mut!((*o).value).write(value) };

        o
    }

    #[inline(always)]
    pub fn value(&self) -> &TypeId {
        &self.value
    }
}
