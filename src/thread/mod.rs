pub use self::stack::*;

use crate::Lua;
use crate::lfunc::luaF_closeupval;
use crate::lmem::luaM_free_;
use crate::lobject::{GCObject, StackValue, StkId, UpVal};
use crate::lstate::{CallInfo, lua_Hook};
use std::alloc::Layout;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomPinned;

mod stack;

/// Lua thread (AKA coroutine).
#[repr(C)]
pub struct Thread {
    pub(crate) next: Cell<*mut GCObject>,
    pub(crate) tt: Cell<u8>,
    pub(crate) marked: Cell<u8>,
    pub(crate) refs: Cell<usize>,
    pub(crate) handle: Cell<usize>,
    pub(crate) allowhook: Cell<u8>,
    pub(crate) nci: Cell<libc::c_ushort>,
    pub(crate) top: StackPtr,
    pub(crate) ci: Cell<*mut CallInfo>,
    pub(crate) stack_last: Cell<StkId>,
    pub(crate) stack: Cell<StkId>,
    pub(crate) openupval: Cell<*mut UpVal>,
    pub(crate) tbclist: Cell<StkId>,
    pub(crate) gclist: Cell<*mut GCObject>,
    pub(crate) twups: Cell<*mut Thread>,
    pub(crate) base_ci: UnsafeCell<CallInfo>,
    pub(crate) hook: Cell<lua_Hook>,
    pub(crate) oldpc: Cell<libc::c_int>,
    pub(crate) basehookcount: Cell<libc::c_int>,
    pub(crate) hookcount: Cell<libc::c_int>,
    pub(crate) hookmask: Cell<libc::c_int>,
    pub(crate) global: *const Lua,
    phantom: PhantomPinned,
}

impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { luaF_closeupval(self, self.stack.get()) };

        if self.stack.get().is_null() {
            return;
        }

        // Free CI.
        self.ci.set(self.base_ci.get());
        let mut ci: *mut CallInfo = self.ci.get();
        let mut next: *mut CallInfo = unsafe { (*ci).next };

        unsafe { (*ci).next = 0 as *mut CallInfo };

        loop {
            ci = next;

            if ci.is_null() {
                break;
            }

            next = unsafe { (*ci).next };

            unsafe { luaM_free_(self.global, ci as *mut libc::c_void, size_of::<CallInfo>()) };
            self.nci.set(self.nci.get().wrapping_sub(1));
        }

        // Free stack.
        let layout = Layout::array::<StackValue>(unsafe {
            self.stack_last.get().offset_from_unsigned(self.stack.get()) + 5
        })
        .unwrap();

        unsafe { std::alloc::dealloc(self.stack.get().cast(), layout) };
    }
}
