use crate::Lua;
use crate::lfunc::luaF_closeupval;
use crate::lmem::luaM_free_;
use crate::lobject::{GCObject, StackValue, StkIdRel, UpVal};
use crate::lstate::{CallInfo, lua_Hook};
use std::alloc::Layout;
use std::marker::PhantomPinned;

/// Lua thread (AKA coroutine).
#[repr(C)]
pub struct Thread {
    pub(crate) next: *mut GCObject,
    pub(crate) tt: u8,
    pub(crate) marked: u8,
    pub(crate) refs: usize,
    pub(crate) handle: usize,
    pub(crate) allowhook: u8,
    pub(crate) nci: libc::c_ushort,
    pub(crate) top: StkIdRel,
    pub(crate) l_G: *const Lua,
    pub(crate) ci: *mut CallInfo,
    pub(crate) stack_last: StkIdRel,
    pub(crate) stack: StkIdRel,
    pub(crate) openupval: *mut UpVal,
    pub(crate) tbclist: StkIdRel,
    pub(crate) gclist: *mut GCObject,
    pub(crate) twups: *mut Thread,
    pub(crate) base_ci: CallInfo,
    pub(crate) hook: lua_Hook,
    pub(crate) oldpc: libc::c_int,
    pub(crate) basehookcount: libc::c_int,
    pub(crate) hookcount: libc::c_int,
    pub(crate) hookmask: libc::c_int,
    phantom: PhantomPinned,
}

impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { luaF_closeupval(self, self.stack.p) };

        if unsafe { self.stack.p.is_null() } {
            return;
        }

        // Free CI.
        self.ci = &raw mut self.base_ci;
        let mut ci: *mut CallInfo = self.ci;
        let mut next: *mut CallInfo = unsafe { (*ci).next };

        unsafe { (*ci).next = 0 as *mut CallInfo };

        loop {
            ci = next;

            if ci.is_null() {
                break;
            }

            next = unsafe { (*ci).next };

            unsafe { luaM_free_(self.l_G, ci as *mut libc::c_void, size_of::<CallInfo>()) };
            self.nci = (self.nci).wrapping_sub(1);
        }

        // Free stack.
        let layout = Layout::array::<StackValue>(unsafe {
            (self.stack_last.p.offset_from(self.stack.p) + 5) as usize
        })
        .unwrap();

        unsafe { std::alloc::dealloc(self.stack.p.cast(), layout) };
        unsafe { (*self.l_G).gc.decrease_debt(layout.size()) };
    }
}
