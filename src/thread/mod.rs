pub use self::args::DynamicArgs;
pub(crate) use self::stack::*;

use self::args::Args;
use crate::lapi::lua_pcall;
use crate::lauxlib::luaL_checkstack;
use crate::lfunc::luaF_closeupval;
use crate::lmem::luaM_free_;
use crate::lobject::{StackValue, StkId, UpVal};
use crate::lstate::{CallInfo, lua_Hook};
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, NON_YIELDABLE_WAKER, Object, Ref, Value};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::{Pin, pin};
use core::ptr::{addr_of_mut, null, null_mut};
use core::task::{Context, Poll, Waker};

mod args;
mod stack;

/// Lua thread (AKA coroutine).
#[repr(C)]
pub struct Thread {
    pub(crate) hdr: Object,
    pub(crate) allowhook: Cell<u8>,
    pub(crate) nci: Cell<libc::c_ushort>,
    pub(crate) top: StackPtr,
    pub(crate) ci: Cell<*mut CallInfo>,
    pub(crate) stack_last: Cell<StkId>,
    pub(crate) stack: Cell<StkId>,
    pub(crate) openupval: Cell<*mut UpVal>,
    pub(crate) tbclist: Cell<StkId>,
    pub(crate) twups: Cell<*const Thread>,
    pub(crate) base_ci: UnsafeCell<CallInfo>,
    pub(crate) hook: Cell<lua_Hook>,
    pub(crate) oldpc: Cell<libc::c_int>,
    pub(crate) basehookcount: Cell<libc::c_int>,
    pub(crate) hookcount: Cell<libc::c_int>,
    pub(crate) hookmask: Cell<libc::c_int>,
    phantom: PhantomPinned,
}

impl Thread {
    #[inline(never)]
    pub(crate) fn new(g: &Pin<Rc<Lua>>) -> Ref<Self> {
        // Create new thread.
        let layout = Layout::new::<Thread>();
        let th = unsafe { Object::new(g.deref(), 8, layout).cast::<Thread>() };

        unsafe { addr_of_mut!((*th).stack).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).ci).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).nci).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).twups).write(Cell::new(th)) };
        unsafe { addr_of_mut!((*th).hook).write(Cell::new(None)) };
        unsafe { addr_of_mut!((*th).hookmask).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).basehookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).allowhook).write(Cell::new(1)) };
        unsafe { addr_of_mut!((*th).hookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).openupval).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).oldpc).write(Cell::new(0)) };

        // Allocate stack.
        let layout = Layout::array::<StackValue>(2 * 20 + 5).unwrap();
        let stack = unsafe { alloc::alloc::alloc(layout) as *mut StackValue };

        if stack.is_null() {
            handle_alloc_error(layout);
        }

        for i in 0..(2 * 20 + 5) {
            unsafe { (*stack.offset(i)).val.tt_ = 0 | 0 << 4 };
        }

        unsafe { (*th).stack.set(stack) };
        unsafe { addr_of_mut!((*th).top).write(StackPtr::new((*th).stack.get())) };
        unsafe { addr_of_mut!((*th).stack_last).write(Cell::new((*th).stack.get().add(2 * 20))) };
        unsafe { addr_of_mut!((*th).tbclist).write(Cell::new((*th).stack.get())) };

        // Setup base CI.
        let ci = unsafe { (*th).base_ci.get() };

        unsafe { (*ci).previous = null_mut() };
        unsafe { (*ci).next = (*ci).previous };
        unsafe { (*ci).callstatus = 1 << 1 };
        unsafe { (*ci).func = (*th).top.get() };
        unsafe { (*ci).u.savedpc = null() };
        unsafe { (*ci).nresults = 0 };
        unsafe { (*th).top.write_nil() };
        unsafe { (*th).top.add(1) };
        unsafe { (*ci).top = ((*th).top.get()).offset(20) };
        unsafe { (*th).ci.set(ci) };

        unsafe { Ref::new(g.clone(), th) }
    }

    /// Call a Lua function.
    ///
    /// # Panics
    /// If `f` or some of `args` come from different [`Lua`] instance.
    pub fn call(
        &self,
        f: &LuaFn,
        args: impl Args,
    ) -> Result<Vec<Value>, Box<dyn core::error::Error>> {
        if f.hdr.global != self.hdr.global {
            panic!("attempt to call a function created from a different Lua");
        }

        // Push function and its arguments.
        let nargs = args.len();

        unsafe { luaL_checkstack(self, 1 + nargs, null())? };

        self.top.write_lua(f);
        unsafe { self.top.add(1) };
        unsafe { args.push_to(self) };

        // Call.
        let f = unsafe { pin!(lua_pcall(self, nargs, 0)) };
        let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

        match f.poll(&mut Context::from_waker(&w)) {
            Poll::Ready(v) => v?,
            Poll::Pending => unreachable!(),
        }

        Ok(Vec::new())
    }

    #[inline(always)]
    pub(crate) unsafe fn get(&self, i: usize) -> UnsafeValue {
        let v = unsafe { (*self.ci.get()).func.add(i) };

        unsafe { (*v).val }
    }
}

impl Drop for Thread {
    #[inline(never)]
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

            unsafe {
                luaM_free_(
                    self.hdr.global,
                    ci as *mut libc::c_void,
                    size_of::<CallInfo>(),
                )
            };
            self.nci.set(self.nci.get().wrapping_sub(1));
        }

        // Free stack.
        let layout = Layout::array::<StackValue>(unsafe {
            self.stack_last.get().offset_from_unsigned(self.stack.get()) + 5
        })
        .unwrap();

        unsafe { alloc::alloc::dealloc(self.stack.get().cast(), layout) };
    }
}
