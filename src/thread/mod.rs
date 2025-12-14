pub use self::input::DynamicInputs;

pub(crate) use self::input::Inputs;
pub(crate) use self::output::Outputs;
pub(crate) use self::stack::*;

use crate::lapi::lua_checkstack;
use crate::ldo::luaD_call;
use crate::lfunc::luaF_closeupval;
use crate::lmem::luaM_free_;
use crate::lobject::UpVal;
use crate::lstate::CallInfo;
use crate::value::UnsafeValue;
use crate::vm::luaV_finishget;
use crate::{
    CallError, Lua, LuaFn, NON_YIELDABLE_WAKER, Object, StackOverflow, Table, Value,
    YIELDABLE_WAKER, luaH_get,
};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::{Cell, RefCell, RefMut, UnsafeCell};
use core::error::Error;
use core::marker::PhantomPinned;
use core::mem::transmute;
use core::num::NonZero;
use core::pin::{Pin, pin};
use core::ptr::{addr_of_mut, null, null_mut};
use core::task::{Context, Poll, Waker};
use thiserror::Error;

mod input;
mod output;
mod stack;

/// Lua thread.
///
/// Use [Lua::create_thread()] or [Context::create_thread()](crate::Context::create_thread()) to
/// create the value of this type.
///
/// You can also use [Lua::call()] to call any Lua function without creating a new thread. You only
/// need to create a new thread when you need to call into async function.
#[repr(C)]
pub struct Thread<A> {
    pub(crate) hdr: Object<A>,
    pub(crate) nci: Cell<u16>,
    pub(crate) top: StackPtr<A>,
    pub(crate) ci: Cell<*mut CallInfo>,
    pub(crate) stack_last: Cell<*mut StackValue<A>>,
    pub(crate) stack: Cell<*mut StackValue<A>>,
    pub(crate) openupval: Cell<*mut UpVal<A>>,
    pub(crate) tbclist: Cell<*mut StackValue<A>>,
    pub(crate) twups: Cell<*const Self>,
    pub(crate) base_ci: UnsafeCell<CallInfo>,
    pub(crate) yielding: Cell<Option<usize>>,
    pending: RefCell<Option<Pin<Box<dyn Future<Output = Result<(), Box<CallError>>>>>>>,
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
        unsafe { addr_of_mut!((*th).openupval).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).yielding).write(Cell::new(None)) };
        unsafe { addr_of_mut!((*th).pending).write(RefCell::default()) };

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
        unsafe { addr_of_mut!((*ci).func).write(0) };
        unsafe { addr_of_mut!((*ci).pc).write(0) };
        unsafe { (*ci).nresults = 0 };
        unsafe { (*th).top.write_nil() };
        unsafe { (*th).top.add(1) };
        unsafe { addr_of_mut!((*ci).top).write(NonZero::new(1).unwrap()) };
        unsafe { (*th).ci.set(ci) };

        th
    }

    /// Sets entry point to be start by [Self::resume()] or [Self::async_resume()].
    ///
    /// # Panics
    /// If `f` was created from different [Lua] instance.
    pub fn set_entry(&self, f: impl Into<UnsafeValue<A>>) -> Result<(), Box<dyn Error>> {
        // Only allows from top-level.
        let top = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };

        if top != 1 {
            return Err(Box::new(ThreadBusy));
        }

        // Check if function created from the same Lua.
        let f = f.into();

        if unsafe { (f.tt_ & 1 << 6) != 0 && (*f.value_.gc).global != self.hdr.global } {
            panic!("attempt to set entry point created from a different Lua");
        }

        // Write function.
        unsafe { lua_checkstack(self, 1, 0)? };

        unsafe { self.top.write(f) };
        unsafe { self.top.add(1) };

        Ok(())
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
        // Only allows from top-level otherwise Lua stack can be corrupted when the future is
        // suspend.
        let top = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };

        if top != 1 {
            return Err(Box::new(ThreadBusy));
        }

        // Check if function created from the same Lua.
        if f.hdr.global != self.hdr.global {
            panic!("attempt to call a function created from a different Lua");
        }

        // Push function and its arguments.
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
                let ot = self.stack.get().add(top);
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

    /// Start of resume a function that was set with [Self::set_entry()].
    ///
    /// # Panics
    /// If some of `args` was created from different [Lua] instance.
    pub fn resume<'a, R: Outputs<'a, A>>(
        &'a self,
        args: impl Inputs<A>,
    ) -> Result<Coroutine<'a, A, R>, Box<dyn Error>> {
        // Get pending call.
        let top = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };
        let mut f = if top == 1 {
            return Err("attempt to resume a thread without entry point".into());
        } else if self.ci.get() != self.base_ci.get() {
            let f = match self.pending.try_borrow_mut() {
                Ok(v) => v,
                Err(_) => return Err(Box::new(ThreadBusy)), // Recursive call.
            };

            // Check if called while async call is active.
            let f = match RefMut::filter_map(f, |v| v.as_mut()) {
                Ok(v) => v,
                Err(_) => return Err("attempt to resume a thread without entry point".into()),
            };

            // Push arguments.
            let nargs = args.len();

            unsafe { lua_checkstack(self, nargs, 0)? };
            unsafe { args.push_to(self) };

            self.yielding.set(Some(nargs));

            f
        } else {
            // Push arguments.
            let nargs = args.len();

            unsafe { lua_checkstack(self, nargs, 0)? };
            unsafe { args.push_to(self) };

            // Start coroutine.
            let f = unsafe { self.top.get().sub(nargs + 1) };
            let f = unsafe { Box::pin(luaD_call(self, f, R::N)) };
            let f = f as Pin<Box<dyn Future<Output = Result<(), Box<CallError>>>>>;
            let p = self.pending.borrow_mut();

            RefMut::map(p, move |v| v.insert(unsafe { transmute(f) }))
        };

        // Resume.
        let r = {
            let w = unsafe { Waker::new(null(), &YIELDABLE_WAKER) };

            match f.as_mut().poll(&mut Context::from_waker(&w)) {
                Poll::Ready(v) => v,
                Poll::Pending => {
                    // Take values from yield.
                    let yields = self.yielding.take().unwrap();
                    let yields = unsafe {
                        self.top.sub(yields);
                        Outputs::new(self, yields)
                    };

                    // Reset stack.
                    let ci = self.ci.get();
                    let top = unsafe { self.stack.get().add((*ci).func + 1) };

                    unsafe { self.top.set(top) };

                    return Ok(Coroutine::Suspended(yields));
                }
            }
        };

        drop(f);

        *self.pending.borrow_mut() = None;

        if let Err(e) = r {
            return Err(e);
        }

        // Get number of results.
        let n = match R::N {
            -1 => unsafe {
                let ot = self.stack.get().add(1);
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

        Ok(Coroutine::Finished(unsafe { R::new(self, n) }))
    }

    /// Start of resume a function that was set with [Self::set_entry()].
    ///
    /// # Panics
    /// If some of `args` was created from different [Lua] instance.
    pub async fn async_resume<'a, R: Outputs<'a, A>>(
        &'a self,
        args: impl Inputs<A>,
    ) -> Result<Coroutine<'a, A, R>, Box<dyn Error>> {
        // Get pending call.
        let top = unsafe { self.top.get().offset_from_unsigned(self.stack.get()) };
        let f = if top == 1 {
            return Err("attempt to resume a thread without entry point".into());
        } else if self.ci.get() != self.base_ci.get() {
            let f = match self.pending.try_borrow_mut() {
                Ok(v) => v,
                Err(_) => return Err(Box::new(ThreadBusy)), // Recursive call.
            };

            // Check if called while async call is active.
            let f = match RefMut::filter_map(f, |v| v.as_mut()) {
                Ok(v) => v,
                Err(_) => return Err("attempt to resume a thread without entry point".into()),
            };

            // Push arguments.
            let nargs = args.len();

            unsafe { lua_checkstack(self, nargs, 0)? };
            unsafe { args.push_to(self) };

            self.yielding.set(Some(nargs));

            f
        } else {
            // Push arguments.
            let nargs = args.len();

            unsafe { lua_checkstack(self, nargs, 0)? };
            unsafe { args.push_to(self) };

            // Start coroutine.
            let f = unsafe { self.top.get().sub(nargs + 1) };
            let f = unsafe { Box::pin(luaD_call(self, f, R::N)) };
            let f = f as Pin<Box<dyn Future<Output = Result<(), Box<CallError>>>>>;
            let p = self.pending.borrow_mut();

            RefMut::map(p, move |v| v.insert(unsafe { transmute(f) }))
        };

        // Resume.
        let r = Resume {
            f,
            y: &self.yielding,
        }
        .await;

        match r {
            Ok(Some(yields)) => {
                // Take values from yield.
                let yields = unsafe {
                    self.top.sub(yields);
                    Outputs::new(self, yields)
                };

                // Reset stack.
                let ci = self.ci.get();
                let top = unsafe { self.stack.get().add((*ci).func + 1) };

                unsafe { self.top.set(top) };

                return Ok(Coroutine::Suspended(yields));
            }
            r => {
                *self.pending.borrow_mut() = None;

                if let Err(e) = r {
                    return Err(e);
                }
            }
        }

        // Get number of results.
        let n = match R::N {
            -1 => unsafe {
                let ot = self.stack.get().add(1);
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

        Ok(Coroutine::Finished(unsafe { R::new(self, n) }))
    }

    /// Index `t` with `k` and returns the result.
    ///
    /// This method honor `__index` metavalue.
    ///
    /// # Panics
    /// If `t` or `k` was created from different [Lua] instance.
    #[inline]
    pub fn index(
        &self,
        t: impl Into<UnsafeValue<A>>,
        k: impl Into<UnsafeValue<A>>,
    ) -> Result<Value<'_, A>, Box<dyn core::error::Error>> {
        // Check if table come from the same Lua.
        let t = t.into();

        if unsafe { (t.tt_ & 1 << 6 != 0) && (*t.value_.gc).global != self.hdr.global } {
            panic!("attempt to index a value created from different Lua");
        }

        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to index a value with key created from different Lua");
        }

        // Try table.
        let mut slot = null();
        let ok = if !(t.tt_ == 5 | 0 << 4 | 1 << 6) {
            false
        } else {
            let t = unsafe { t.value_.gc.cast::<Table<A>>() };

            slot = unsafe { luaH_get(t, &k) };

            unsafe { !((*slot).tt_ & 0xf == 0) }
        };

        // Get value.
        if ok {
            return Ok(unsafe { Value::from_unsafe(slot) });
        }

        // Try __index.
        let v = unsafe { luaV_finishget(self, &t, &k, false)? };

        Ok(unsafe { Value::from_unsafe(&v) })
    }

    /// Reserves capacity for at least `additional` more elements to be pushed.
    ///
    /// Usually you don't need this method unless you want to distinguished [StackOverflow] caused by too many arguments.
    ///
    /// This has the same semantic as `lua_checkstack`.
    #[inline(always)]
    pub fn reserve(&self, additional: usize) -> Result<(), StackOverflow> {
        unsafe { lua_checkstack(self, additional, 0) }
    }
}

impl<A> Drop for Thread<A> {
    #[inline(never)]
    fn drop(&mut self) {
        *self.pending.get_mut() = None;

        unsafe { luaF_closeupval(self, self.stack.get()) };

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

            unsafe { luaM_free_(ci.cast(), size_of::<CallInfo>()) };
            self.nci.set(self.nci.get().wrapping_sub(1));
        }

        // Free stack.
        let layout = Layout::array::<StackValue<A>>(unsafe {
            self.stack_last.get().offset_from_unsigned(self.stack.get()) + 5
        })
        .unwrap();

        unsafe { alloc::alloc::dealloc(self.stack.get().cast(), layout) };
    }
}

/// Result of [Thread::resume()] or [Thread::async_resume()].
pub enum Coroutine<'a, A, R> {
    Suspended(Vec<Value<'a, A>>),
    Finished(R),
}

/// Implementation of [Future] to resume coroutine.
struct Resume<'a> {
    f: RefMut<'a, Pin<Box<dyn Future<Output = Result<(), Box<CallError>>>>>>,
    y: &'a Cell<Option<usize>>,
}

impl<'a> Future for Resume<'a> {
    type Output = Result<Option<usize>, Box<CallError>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Poll.
        let w = unsafe { Waker::new(cx as *mut Context as *const (), &YIELDABLE_WAKER) };

        if let Poll::Ready(r) = self.f.as_mut().poll(&mut Context::from_waker(&w)) {
            return Poll::Ready(r.map(|_| None));
        }

        // Check if yield.
        match self.y.take() {
            Some(v) => Poll::Ready(Ok(Some(v))),
            None => Poll::Pending,
        }
    }
}

/// Represents an error when attempt to use a thread that has active call.
#[derive(Debug, Error)]
#[error("thread busy")]
pub struct ThreadBusy;
