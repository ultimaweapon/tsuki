pub use self::input::DynamicInputs;

pub(crate) use self::input::Inputs;
pub(crate) use self::output::Outputs;
pub(crate) use self::stack::*;

use crate::lapi::lua_checkstack;
use crate::ldo::luaD_call;
use crate::lfunc::luaF_closeupval;
use crate::lmem::luaM_free_;
use crate::lobject::UpVal;
use crate::lstate::{CallInfo, lua_Debug};
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, NON_YIELDABLE_WAKER, Object};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};
use core::error::Error;
use core::marker::PhantomPinned;
use core::pin::pin;
use core::ptr::{addr_of_mut, null, null_mut};
use core::task::{Context, Poll, Waker};

mod input;
mod output;
mod stack;

/// Lua thread (AKA coroutine).
///
/// Use [Lua::create_thread()] or [Context::create_thread()](crate::Context::create_thread()) to
/// create the value of this type.
///
/// You can also use [Lua::call()] to call any Lua function without creating a new thread. You only
/// need to create a new thread when you need to call into async function.
#[repr(C)]
pub struct Thread<A> {
    pub(crate) hdr: Object<A>,
    pub(crate) allowhook: Cell<u8>,
    pub(crate) nci: Cell<u16>,
    pub(crate) top: StackPtr<A>,
    pub(crate) ci: Cell<*mut CallInfo<A>>,
    pub(crate) stack_last: Cell<*mut StackValue<A>>,
    pub(crate) stack: Cell<*mut StackValue<A>>,
    pub(crate) openupval: Cell<*mut UpVal<A>>,
    pub(crate) tbclist: Cell<*mut StackValue<A>>,
    pub(crate) twups: Cell<*const Self>,
    pub(crate) base_ci: UnsafeCell<CallInfo<A>>,
    pub(crate) hook: Cell<Option<unsafe fn(*const Self, *mut lua_Debug<A>)>>,
    pub(crate) oldpc: Cell<i32>,
    pub(crate) basehookcount: Cell<i32>,
    pub(crate) hookcount: Cell<i32>,
    pub(crate) hookmask: Cell<i32>,
    phantom: PhantomPinned,
}

impl<A> Thread<A> {
    pub(crate) fn new(g: &Lua<A>) -> *const Self {
        // Create new thread.
        let layout = Layout::new::<Self>();
        let th = unsafe { g.gc.alloc(8, layout).cast::<Self>() };

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
        let layout = Layout::array::<StackValue<A>>(2 * 20 + 5).unwrap();
        let stack = unsafe { alloc::alloc::alloc(layout) as *mut StackValue<A> };

        if stack.is_null() {
            handle_alloc_error(layout);
        }

        for i in 0..(2 * 20 + 5) {
            unsafe { (*stack.offset(i)).tt_ = 0 | 0 << 4 };
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

        th
    }

    /// Call a function or callable value.
    ///
    /// `args` can be either:
    ///
    /// - A unit to represents zero arguments.
    /// - Any value that can be converted to [UnsafeValue] or a tuple of it.
    /// - [DynamicInputs].
    ///
    /// `R` can be either:
    ///
    /// - A unit to discard all results.
    /// - [Value](crate::Value) to extract first result and discard the rest.
    /// - [Vec](alloc::vec::Vec) of [Value](crate::Value) to extract all results.
    ///
    /// The error will be either [CallError](crate::CallError) or something else.
    ///
    /// # Panics
    /// If `f` or some of `args` was created from different [Lua] instance.
    pub fn call<'a, R: Outputs<'a, A>>(
        &'a self,
        f: impl Into<UnsafeValue<A>>,
        args: impl Inputs<A>,
    ) -> Result<R, Box<dyn Error>> {
        // Check if function created from the same Lua.
        let f = f.into();

        if unsafe { (f.tt_ & 1 << 6) != 0 && (*f.value_.gc).global != self.hdr.global } {
            panic!("attempt to call a value created from a different Lua");
        }

        // Push function and its arguments.
        let ot = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };
        let nargs = args.len();

        unsafe { lua_checkstack(self, 1 + nargs, 0)? };

        unsafe { self.top.write(f) };
        unsafe { self.top.add(1) };
        unsafe { args.push_to(self) };

        // Call.
        {
            let f = unsafe { self.top.get().sub(nargs + 1) };
            let f = unsafe { pin!(luaD_call(self, f, R::N)) };
            let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

            match f.poll(&mut Context::from_waker(&w)) {
                Poll::Ready(Ok(_)) => (),
                Poll::Ready(Err(e)) => return Err(e),
                Poll::Pending => unreachable!(),
            }
        }

        // Get number of results.
        let n = match R::N {
            -1 => unsafe {
                let ot = self.stack.get().add(ot);
                let v = self.top.get().offset_from_unsigned(ot);

                self.top.set(ot);

                v
            },
            0 => 0,
            v => unsafe {
                let v = v.try_into().unwrap();
                self.top.sub(v);
                v
            },
        };

        Ok(unsafe { R::new(self, n) })
    }

    /// Call a function with ability to call into [AsyncFp](crate::AsyncFp).
    ///
    /// `args` can be either:
    ///
    /// - A unit to represents zero arguments.
    /// - Any value that can be converted to [UnsafeValue] or a tuple of it.
    /// - [DynamicInputs].
    ///
    /// `R` can be either:
    ///
    /// - A unit to discard all results.
    /// - [Value](crate::Value) to extract first result and discard the rest.
    /// - [Vec](alloc::vec::Vec) of [Value](crate::Value) to extract all results.
    ///
    /// The error will be either [CallError](crate::CallError) or something else.
    ///
    /// This method is not available on main thread so you need to create a [Thread] to use this
    /// method.
    ///
    /// # Panics
    /// If `f` or some of `args` was created from different [Lua] instance.
    pub async fn async_call<'a, R: Outputs<'a, A>>(
        &'a self,
        f: &LuaFn<A>,
        args: impl Inputs<A>,
    ) -> Result<R, Box<dyn Error>> {
        // Only allows from top-level.
        if self.ci.get() != self.base_ci.get() {
            return Err("attempt to do async call within Rust frames".into());
        }

        // Check if function created from the same Lua.
        if f.hdr.global != self.hdr.global {
            panic!("attempt to call a function created from a different Lua");
        }

        // Push function and its arguments.
        let ot = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };
        let nargs = args.len();

        unsafe { lua_checkstack(self, 1 + nargs, 0)? };

        unsafe { self.top.write(f.into()) };
        unsafe { self.top.add(1) };
        unsafe { args.push_to(self) };

        // Call.
        let f = unsafe { self.top.get().sub(nargs + 1) };

        if let Err(e) = unsafe { luaD_call(self, f, R::N).await } {
            return Err(e); // Required for unsized coercion.
        }

        // Get number of results.
        let n = match R::N {
            -1 => unsafe {
                let ot = self.stack.get().add(ot);
                let v = self.top.get().offset_from_unsigned(ot);

                self.top.set(ot);

                v
            },
            0 => 0,
            v => unsafe {
                let v = v.try_into().unwrap();
                self.top.sub(v);
                v
            },
        };

        Ok(unsafe { R::new(self, n) })
    }
}

impl<D> Drop for Thread<D> {
    #[inline(never)]
    fn drop(&mut self) {
        unsafe { luaF_closeupval(self, self.stack.get()) };

        if self.stack.get().is_null() {
            return;
        }

        // Free CI.
        self.ci.set(self.base_ci.get());
        let mut ci = self.ci.get();
        let mut next = unsafe { (*ci).next };

        unsafe { (*ci).next = null_mut() };

        loop {
            ci = next;

            if ci.is_null() {
                break;
            }

            next = unsafe { (*ci).next };

            unsafe { luaM_free_(ci.cast(), size_of::<CallInfo<D>>()) };
            self.nci.set(self.nci.get().wrapping_sub(1));
        }

        // Free stack.
        let layout = Layout::array::<StackValue<D>>(unsafe {
            self.stack_last.get().offset_from_unsigned(self.stack.get()) + 5
        })
        .unwrap();

        unsafe { alloc::alloc::dealloc(self.stack.get().cast(), layout) };
    }
}
