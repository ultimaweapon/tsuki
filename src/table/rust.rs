use crate::Lua;
use crate::gc::Object;
use core::alloc::Layout;
use core::any::TypeId;
use core::ptr::addr_of_mut;

/// Encapsulates [`TypeId`] to be used as a table key.
#[repr(C)]
pub(crate) struct RustId<D> {
    hdr: Object<D>,
    value: TypeId,
}

impl<D> RustId<D> {
    pub unsafe fn new(g: *const Lua<D>, value: TypeId) -> *const Self {
        let layout = Layout::new::<Self>();
        let o = unsafe { (*g).gc.alloc(11 | 0 << 4, layout).cast::<Self>() };

        unsafe { addr_of_mut!((*o).value).write(value) };

        o
    }

    #[inline(always)]
    pub fn value(&self) -> &TypeId {
        &self.value
    }
}
