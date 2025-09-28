use crate::gc::Gc;
use crate::llex::{TK_WHILE, luaX_tokens};
use crate::ltm::{
    TM_ADD, TM_BAND, TM_BNOT, TM_BOR, TM_BXOR, TM_CALL, TM_CLOSE, TM_CONCAT, TM_DIV, TM_EQ, TM_GC,
    TM_IDIV, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_MOD, TM_MODE, TM_MUL, TM_NEWINDEX, TM_POW, TM_SHL,
    TM_SHR, TM_SUB, TM_UNM,
};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{Lua, Nil, Node, NodeKey, Str, StringTable, Table, Thread, luaH_resize};
use alloc::rc::Rc;
use core::cell::UnsafeCell;
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null;

/// Struct to build instance of [`Lua`].
pub struct Builder {
    seed: u32,
}

impl Builder {
    /// Create a new [Builder] with a random seed to hash Lua string.
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    #[cfg(feature = "rand")]
    pub fn new() -> Self {
        Self::with_seed(rand::random())
    }

    /// Create a new [Builder] with a seed to hash Lua string.
    ///
    /// You can use [Builder::new()] instead if `rand` feature is enabled (which is default) or you
    /// can pass `0` as a seed if
    /// [HashDoS](https://en.wikipedia.org/wiki/Collision_attack#Hash_flooding) attack is not
    /// possible for your application.
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    pub fn with_seed(seed: u32) -> Self {
        Self { seed }
    }

    /// Create the value of [Lua] from this [Builder].
    ///
    /// You can retrieve `associated_data` later with [Lua::associated_data()] or
    /// [Context::associated_data()](crate::Context::associated_data()).
    pub fn build<A>(self, associated_data: A) -> Pin<Rc<Lua<A>>> {
        let g = Rc::pin(Lua {
            gc: unsafe { Gc::new() }, // SAFETY: gc in the first field on Lua.
            strt: StringTable::new(),
            l_registry: UnsafeCell::new(Nil.into()),
            nilvalue: UnsafeCell::new(Nil.into()),
            dummy_node: Node {
                u: NodeKey {
                    value_: UntaggedValue { gc: null() },
                    tt_: 0 | 1 << 4,
                    key_tt: 0 | 0 << 4,
                    next: 0,
                    key_val: UntaggedValue { gc: null() },
                },
            },
            seed: self.seed,
            associated_data,
            _phantom: PhantomPinned,
        });

        // Setup registry.
        let reg = unsafe { Table::new(g.deref()) };

        unsafe { g.gc.set_root(reg.cast()) };
        unsafe { g.l_registry.get().write(UnsafeValue::from_obj(reg.cast())) };
        unsafe { luaH_resize(reg, 5, 0) };

        // Create main thread.
        let reg = unsafe { (*reg).array.get() };
        let main = Thread::new(g.deref());

        unsafe { reg.add(0).write(UnsafeValue::from_obj(main.cast())) };

        // Create LUA_RIDX_GLOBALS.
        let glb = unsafe { Table::new(g.deref()) };

        unsafe { reg.add(1).write(UnsafeValue::from_obj(glb.cast())) };

        // Create table for metatables.
        let mts = unsafe { Table::new(g.deref()) };

        unsafe { luaH_resize(mts, 9, 0) };
        unsafe { reg.add(2).write(UnsafeValue::from_obj(mts.cast())) };

        // Create table for event names.
        let events = unsafe { Table::new(g.deref()) };
        let entries = [
            (TM_INDEX, "__index"),
            (TM_NEWINDEX, "__newindex"),
            (TM_GC, "__gc"),
            (TM_MODE, "__mode"),
            (TM_LEN, "__len"),
            (TM_EQ, "__eq"),
            (TM_ADD, "__add"),
            (TM_SUB, "__sub"),
            (TM_MUL, "__mul"),
            (TM_MOD, "__mod"),
            (TM_POW, "__pow"),
            (TM_DIV, "__div"),
            (TM_IDIV, "__idiv"),
            (TM_BAND, "__band"),
            (TM_BOR, "__bor"),
            (TM_BXOR, "__bxor"),
            (TM_SHL, "__shl"),
            (TM_SHR, "__shr"),
            (TM_UNM, "__unm"),
            (TM_BNOT, "__bnot"),
            (TM_LT, "__lt"),
            (TM_LE, "__le"),
            (TM_CONCAT, "__concat"),
            (TM_CALL, "__call"),
            (TM_CLOSE, "__close"),
        ];

        unsafe { luaH_resize(events, entries.len().try_into().unwrap(), 0) };

        for (k, v) in entries {
            let v = unsafe { Str::from_str(g.deref(), v) };
            let v = unsafe { UnsafeValue::from_obj(v.cast()) };

            unsafe { (*events).set_unchecked(k, v).unwrap_unchecked() };
        }

        unsafe { reg.add(3).write(UnsafeValue::from_obj(events.cast())) };

        // Create table for Lua tokens.
        let tokens = unsafe { Table::new(g.deref()) };
        let n = TK_WHILE - (255 + 1) + 1;

        unsafe { luaH_resize(tokens, 0, n.try_into().unwrap()) };

        for i in 0..n {
            let k = unsafe { Str::from_str(g.deref(), luaX_tokens[i as usize]) };
            let k = unsafe { UnsafeValue::from_obj(k.cast()) };

            unsafe { (*tokens).set_unchecked(k, i + 1).unwrap_unchecked() };
        }

        unsafe { reg.add(4).write(UnsafeValue::from_obj(tokens.cast())) };

        g
    }
}
