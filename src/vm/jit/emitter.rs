use super::{Error, HOST, RustFuncs, State, Status};
use crate::lfunc::luaF_closeupval;
use crate::lobject::{Proto, UpVal};
use crate::lstate::CallInfo;
use crate::ltm::{TM_LE, TM_LT};
use crate::value::UnsafeValue;
use crate::vm::{LEnum, LTnum, floatforloop, luaV_modf};
use crate::{
    LuaFn, StackValue, Table, Thread, UserData, luaH_get, luaH_getint, luaH_getshortstr,
    luaH_getstr, luaH_realasize, luaH_resize, luaH_resizearray,
};
use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;
use core::mem::offset_of;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types::{F64, I8, I16, I32, I64};
use cranelift_codegen::ir::{
    Block, BlockArg, BlockCall, FuncRef, InstBuilder, MemFlags, StackSlotData, StackSlotKind, Type,
    Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// Contains state to emit Cranelift instructions for a Lua function.
pub struct Emitter<'a, 'b, A> {
    fb: &'a mut FunctionBuilder<'b>,
    code: &'a [u32],
    st: Variable,
    cx: Variable,
    ret: Variable,
    td: Variable,
    ci: Variable,
    f: Variable,
    p: Variable,
    k: Variable,
    base: Variable,
    adjustvarargs: FuncRef,
    getvarargs: FuncRef,
    finishget: FuncRef,
    finishset: FuncRef,
    getint: FuncRef,
    getshortstr: FuncRef,
    getstr: FuncRef,
    realasize: FuncRef,
    resizearray: FuncRef,
    precall: FuncRef,
    resume_precall: FuncRef,
    pretailcall: FuncRef,
    resume_pretailcall: FuncRef,
    run_lua: FuncRef,
    resume_run_lua: FuncRef,
    close: FuncRef,
    poscall: FuncRef,
    pushclosure: FuncRef,
    gc: FuncRef,
    barrier: FuncRef,
    barrier_back: FuncRef,
    create_table: FuncRef,
    resize_table: FuncRef,
    lookup_table: FuncRef,
    equalobj: FuncRef,
    LTnum: FuncRef,
    LEnum: FuncRef,
    lessthanothers: FuncRef,
    lessequalothers: FuncRef,
    objlen: FuncRef,
    closeupval: FuncRef,
    trybinTM: FuncRef,
    trybiniTM: FuncRef,
    trybinassocTM: FuncRef,
    callorderiTM: FuncRef,
    newtbcupval: FuncRef,
    forprep: FuncRef,
    floatforloop: FuncRef,
    mod_f: FuncRef,
    mod_zero: FuncRef,
    ptr: Type,
    labels: HashMap<usize, Block>,
    resumes: &'a mut Vec<BlockCall>,
    phantom: PhantomData<A>,
}

impl<'a, 'b, A> Emitter<'a, 'b, A> {
    pub unsafe fn new(
        fb: &'a mut FunctionBuilder<'b>,
        code: &'a [u32],
        st: Variable,
        cx: Variable,
        ret: Variable,
        rust: &'a mut RustFuncs<A>,
        resumes: &'a mut Vec<BlockCall>,
        jumper: Block,
    ) -> Self {
        // Load td.
        let ptr = Type::triple_pointer_type(&HOST);
        let td = fb.declare_var(ptr);
        let v = fb.use_var(st);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(State<A>, td) as i32,
        );

        fb.def_var(td, v);

        // Load ci.
        let ci = fb.declare_var(ptr);
        let v = fb.use_var(st);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(State<A>, ci) as i32,
        );

        fb.def_var(ci, v);

        // Get CallInfo::func.
        let v = fb.use_var(ci);
        let f = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(CallInfo, func) as i32,
        );

        // Get Thread::stack.
        let f = fb.ins().imul_imm(f, size_of::<StackValue<A>>() as i64);
        let v = fb.use_var(td);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted(),
            v,
            offset_of!(Thread<A>, stack) as i32,
        );

        // Load function object.
        let v = fb.ins().iadd(v, f);
        let f = fb.declare_var(ptr);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted(),
            v,
            offset_of!(StackValue<A>, value_) as i32,
        );

        fb.def_var(f, v);

        // Load function prototype.
        let p = fb.declare_var(ptr);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(LuaFn<A>, p) as i32,
        );

        fb.def_var(p, v);

        // Load constants.
        let k = fb.declare_var(ptr);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(Proto<A>, k) as i32,
        );

        fb.def_var(k, v);

        // Get base stack.
        let mut e = Self {
            code,
            st,
            cx,
            ret,
            td,
            ci,
            f,
            p,
            k,
            base: fb.declare_var(ptr),
            adjustvarargs: rust.import(
                fb,
                &[ptr, I32, ptr, ptr, ptr],
                None,
                super::adjustvarargs::<A> as *const u8,
            ),
            getvarargs: rust.import(
                fb,
                &[ptr, ptr, ptr, I32, ptr],
                None,
                super::getvarargs::<A> as *const u8,
            ),
            finishget: rust.import(
                fb,
                &[ptr, ptr, ptr, I8, ptr, ptr],
                None,
                super::finishget::<A> as *const u8,
            ),
            finishset: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr, ptr, ptr],
                None,
                super::finishset::<A> as *const u8,
            ),
            getint: rust.import(fb, &[ptr, I64], Some(ptr), luaH_getint::<A> as *const u8),
            getshortstr: rust.import(
                fb,
                &[ptr, ptr],
                Some(ptr),
                luaH_getshortstr::<A> as *const u8,
            ),
            getstr: rust.import(fb, &[ptr, ptr], Some(ptr), luaH_getstr::<A> as *const u8),
            realasize: rust.import(fb, &[ptr], Some(I32), luaH_realasize::<A> as *const u8),
            resizearray: rust.import(fb, &[ptr, I32], None, luaH_resizearray::<A> as *const u8),
            precall: rust.import(
                fb,
                &[ptr, ptr, ptr, I32, ptr, ptr],
                Some(ptr),
                super::precall::<A> as *const u8,
            ),
            resume_precall: rust.import(
                fb,
                &[ptr, ptr, ptr],
                Some(ptr),
                super::resume_precall::<A> as *const u8,
            ),
            pretailcall: rust.import(
                fb,
                &[ptr, ptr, ptr, I32, I32, ptr, ptr],
                Some(I32),
                super::pretailcall::<A> as *const u8,
            ),
            resume_pretailcall: rust.import(
                fb,
                &[ptr, ptr, ptr],
                Some(I32),
                super::resume_pretailcall::<A> as *const u8,
            ),
            run_lua: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr, ptr],
                None,
                super::run_lua::<A> as *const u8,
            ),
            resume_run_lua: rust.import(
                fb,
                &[ptr, ptr, ptr],
                None,
                super::resume_run_lua::<A> as *const u8,
            ),
            close: rust.import(fb, &[ptr, ptr, ptr], None, super::close::<A> as *const u8),
            poscall: rust.import(
                fb,
                &[ptr, ptr, I32, ptr],
                None,
                super::poscall::<A> as *const u8,
            ),
            pushclosure: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr, ptr],
                None,
                super::pushclosure::<A> as *const u8,
            ),
            gc: rust.import(fb, &[ptr], None, super::step_gc::<A> as *const u8),
            barrier: rust.import(fb, &[ptr, ptr], None, super::barrier::<A> as *const u8),
            barrier_back: rust.import(fb, &[ptr, ptr], None, super::barrier_back::<A> as *const u8),
            create_table: rust.import(fb, &[ptr], Some(ptr), super::create_table::<A> as *const u8),
            resize_table: rust.import(fb, &[ptr, I32, I32], None, luaH_resize::<A> as *const u8),
            lookup_table: rust.import(fb, &[ptr, ptr], Some(ptr), luaH_get::<A> as *const u8),
            equalobj: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr],
                Some(I32),
                super::equalobj::<A> as *const u8,
            ),
            LTnum: rust.import(fb, &[ptr, ptr], Some(I8), LTnum::<A> as *const u8),
            LEnum: rust.import(fb, &[ptr, ptr], Some(I8), LEnum::<A> as *const u8),
            lessthanothers: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr],
                Some(I8),
                super::lessthanothers::<A> as *const u8,
            ),
            lessequalothers: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr],
                Some(I8),
                super::lessequalothers::<A> as *const u8,
            ),
            objlen: rust.import(
                fb,
                &[ptr, ptr, ptr, ptr],
                None,
                super::objlen::<A> as *const u8,
            ),
            closeupval: rust.import(fb, &[ptr, ptr], None, luaF_closeupval::<A> as *const u8),
            trybinTM: rust.import(
                fb,
                &[ptr, ptr, ptr, I32, ptr, ptr],
                None,
                super::trybinTM::<A> as *const u8,
            ),
            trybiniTM: rust.import(
                fb,
                &[ptr, ptr, I64, I32, I32, ptr, ptr],
                None,
                super::trybiniTM::<A> as *const u8,
            ),
            trybinassocTM: rust.import(
                fb,
                &[ptr, ptr, ptr, I32, I32, ptr, ptr],
                None,
                super::trybinassocTM::<A> as *const u8,
            ),
            callorderiTM: rust.import(
                fb,
                &[ptr, ptr, I32, I32, I32, I32, ptr],
                Some(I8),
                super::callorderiTM::<A> as *const u8,
            ),
            newtbcupval: rust.import(
                fb,
                &[ptr, ptr, ptr],
                None,
                super::newtbcupval::<A> as *const u8,
            ),
            forprep: rust.import(
                fb,
                &[ptr, ptr, ptr],
                Some(I8),
                super::forprep::<A> as *const u8,
            ),
            floatforloop: rust.import(fb, &[ptr], Some(I8), floatforloop::<A> as *const u8),
            mod_f: rust.import(fb, &[F64, F64], Some(F64), luaV_modf as *const u8),
            mod_zero: rust.import(fb, &[ptr], None, super::mod_zero as *const u8),
            ptr,
            labels: HashMap::new(),
            resumes,
            fb,
            phantom: PhantomData,
        };

        e.update_base_stack();

        e.fb.ins().jump(jumper, []);

        // Create root block.
        let root = e.fb.create_block();

        e.fb.switch_to_block(root);
        e.resumes.push(e.fb.func.dfg.block_call(root, []));

        e
    }

    pub fn prepare(&mut self, pc: usize) {
        let label = match self.labels.get(&pc) {
            Some(v) => *v,
            None => return,
        };

        // Check if we need to fallthrough.
        let current = self.fb.current_block().unwrap();

        if self
            .fb
            .func
            .layout
            .last_inst(current)
            .is_none_or(|i| !self.fb.func.dfg.insts[i].opcode().is_terminator())
        {
            self.fb.ins().jump(label, []);
        }

        self.fb.switch_to_block(label);
    }

    pub unsafe fn move_(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & 0xFF);
        let rb = self.get_reg(i >> 7 + 8 + 1 & 0xFF);

        // Type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            rb,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            rb,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn loadi(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & 0xFF);
        let b: i64 = ((i >> 15 & 0x1FFFF) as i32 - ((1 << 17) - 1 >> 1)) as i64;
        let b = self.fb.ins().iconst(I64, b);
        let tt = self.fb.ins().iconst(I8, 3 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            b,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn loadk(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_const(i >> 7 + 8 & !(!(0u32) << 8 + 8 + 1));

        // Set type.
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            rb,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set value.
        let value = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            rb,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            value,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn loadfalse(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tt = self.fb.ins().iconst(I8, 1 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        pc
    }

    pub unsafe fn lfalseskip(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tt = self.fb.ins().iconst(I8, 1 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Jump.
        let label = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        self.fb.ins().jump(label, []);

        pc
    }

    pub unsafe fn loadtrue(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tt = self.fb.ins().iconst(I8, 1 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        pc
    }

    pub unsafe fn loadnil(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let b = i >> 7 + 8 + 1 & !(!(0u32) << 8);
        let v = self.fb.ins().iconst(I8, 0 | 0 << 4);

        for k in 0..=b {
            self.fb.ins().store(
                MemFlags::trusted(),
                v,
                ra,
                (k as usize * size_of::<StackValue<A>>() + offset_of!(StackValue<A>, tt_)) as i32,
            );
        }

        pc
    }

    pub unsafe fn getupval(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let b = i >> 7 + 8 + 1 & !(!(0u32) << 8);
        let uv = self.load_uv(b);

        // Set type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            uv,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            uv,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn setupval(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        // Load UpVal::v.
        let uv = self.get_uv(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let dst = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            uv,
            offset_of!(UpVal<A>, v) as i32,
        );

        // Set type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            dst,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Set value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            dst,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().call(self.barrier, &[uv, ra]);

        pc
    }

    pub unsafe fn gettabup(&mut self, i: u32, pc: usize) -> usize {
        // Get output register and key.
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let k = self.load_const(
            i >> 24 & !(!(0u32) << 8),
            offset_of!(UnsafeValue<A>, value_),
        );

        // Load table type.
        let tab = self.load_uv(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            tab,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check table type.
        let load_tab = self.fb.create_block();
        let check_ud = self.fb.create_block();
        let test_res = self.fb.create_block();
        let ty = self.fb.ins().band_imm(tt, 0xf);
        let is_tab = self.fb.ins().icmp_imm(IntCC::Equal, ty, 5);

        self.fb.append_block_param(test_res, self.ptr);

        self.fb.ins().brif(is_tab, load_tab, [], check_ud, []);

        self.fb.switch_to_block(load_tab);
        self.fb.seal_block(load_tab);

        // Load table object.
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            tab,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Invoke luaH_getshortstr.
        let v = self.fb.ins().call(self.getshortstr, &[v, k]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().jump(test_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_ud);
        self.fb.seal_block(check_ud);

        // Check if userdata.
        let load_ud = self.fb.create_block();
        let not_ud = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 7);

        self.fb.ins().brif(v, load_ud, [], not_ud, []);

        self.fb.switch_to_block(load_ud);
        self.fb.seal_block(load_ud);

        // Load userdata.
        let ud = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            tab,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Load UserData::props.
        let props = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ud,
            offset_of!(UserData<A, dyn Any>, props) as i32,
        );

        // Check if no properties.
        let load_prop = self.fb.create_block();

        self.fb.ins().brif(props, load_prop, [], not_ud, []);

        self.fb.switch_to_block(load_prop);
        self.fb.seal_block(load_prop);

        // Load property.
        let v = self.fb.ins().call(self.getshortstr, &[props, k]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().jump(test_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(not_ud);
        self.fb.seal_block(not_ud);

        // Set index result to null.
        let v = self.fb.ins().iconst(self.ptr, 0);

        self.fb.ins().jump(test_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(test_res);
        self.fb.seal_block(test_res);

        // Check result.
        let has_res = self.fb.create_block();
        let no_res = self.fb.create_block();
        let next_inst = self.fb.create_block();
        let res = self.fb.block_params(test_res)[0];

        self.fb.ins().brif(res, has_res, [], no_res, []);

        self.fb.switch_to_block(has_res);
        self.fb.seal_block(has_res);

        // Load result type.
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            res,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if not nil.
        let not_nil = self.fb.create_block();
        let v = self.fb.ins().band_imm(tt, 0xf);

        self.fb.ins().brif(v, not_nil, [], no_res, []);

        self.fb.switch_to_block(not_nil);
        self.fb.seal_block(not_nil);

        // Set output register.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            res,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(next_inst, []);

        self.fb.switch_to_block(no_res);
        self.fb.seal_block(no_res);

        // Emit call to luaV_finishget.
        let v = self.fb.ins().iconst(I8, 4 | 0 << 4 | 1 << 6);

        self.finishget_with_key_parts(i, pc, tab, v, k);

        self.fb.ins().jump(next_inst, []);

        self.fb.switch_to_block(next_inst);
        self.fb.seal_block(next_inst);

        pc
    }

    pub unsafe fn gettable(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tab = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let k = self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));

        // Load table type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load table object.
        let o = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if table.
        let ty = self.fb.ins().band_imm(v, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 5);
        let lookup_table = self.fb.create_block();
        let check_ud = self.fb.create_block();

        self.fb.append_block_param(lookup_table, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, &[BlockArg::Value(o)], check_ud, []);

        self.fb.switch_to_block(lookup_table);

        // Invoke luaH_get.
        let v = self.fb.block_params(lookup_table)[0];
        let slot = self.fb.ins().call(self.lookup_table, &[v, k]);
        let slot = self.fb.inst_results(slot)[0];
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if found.
        let v = self.fb.ins().band_imm(tt, 0xf);
        let found = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.ins().brif(v, found, [], not_found, []);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set output register.
        let end = self.fb.create_block();
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(end, []);

        self.fb.switch_to_block(check_ud);
        self.fb.seal_block(check_ud);

        // Check if userdata.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 7);
        let load_ud = self.fb.create_block();

        self.fb.ins().brif(v, load_ud, [], not_found, []);

        self.fb.switch_to_block(load_ud);
        self.fb.seal_block(load_ud);

        // Load UserData::props.
        let props = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            o,
            offset_of!(UserData<A, dyn Any>, props) as i32,
        );

        self.fb.ins().brif(
            props,
            lookup_table,
            &[BlockArg::Value(props)],
            not_found,
            [],
        );

        self.fb.seal_block(lookup_table);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishget.
        self.finishget(i, pc, tab, k);

        self.fb.ins().jump(end, []);

        self.fb.switch_to_block(end);
        self.fb.seal_block(end);

        pc
    }

    pub unsafe fn geti(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tab = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let c = self
            .fb
            .ins()
            .iconst(I64, i64::from(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)));

        // Load table type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load table object.
        let t = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if table.
        let ty = self.fb.ins().band_imm(v, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 5);
        let lookup_table = self.fb.create_block();
        let check_ud = self.fb.create_block();

        self.fb.append_block_param(lookup_table, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, &[BlockArg::Value(t)], check_ud, []);

        self.fb.switch_to_block(lookup_table);

        // Invoke luaH_getint.
        let v = self.fb.block_params(lookup_table)[0];
        let slot = self.fb.ins().call(self.getint, &[v, c]);
        let slot = self.fb.inst_results(slot)[0];
        let vt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if found.
        let v = self.fb.ins().band_imm(vt, 0xf);
        let found = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.ins().brif(v, found, [], not_found, []);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set output register.
        let join = self.fb.create_block();
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            vt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(check_ud);
        self.fb.seal_block(check_ud);

        // Check if userdata.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 7);
        let load_ud = self.fb.create_block();

        self.fb.ins().brif(v, load_ud, [], not_found, []);

        self.fb.switch_to_block(load_ud);
        self.fb.seal_block(load_ud);

        // Load UserData::props.s
        let props = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            t,
            offset_of!(UserData<A, dyn Any>, props) as i32,
        );

        self.fb.ins().brif(
            props,
            lookup_table,
            &[BlockArg::Value(props)],
            not_found,
            [],
        );

        self.fb.seal_block(lookup_table);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishget.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);

        self.finishget_with_key_parts(i, pc, tab, v, c);

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn getfield(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tab = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let k = self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));

        // Load table type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load table object.
        let o = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if table.
        let ty = self.fb.ins().band_imm(v, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 5);
        let lookup_table = self.fb.create_block();
        let check_ud = self.fb.create_block();

        self.fb.append_block_param(lookup_table, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, &[BlockArg::Value(o)], check_ud, []);

        self.fb.switch_to_block(lookup_table);

        // Load key.
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            k,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Invoke luaH_getshortstr.
        let t = self.fb.block_params(lookup_table)[0];
        let slot = self.fb.ins().call(self.getshortstr, &[t, v]);
        let slot = self.fb.inst_results(slot)[0];
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if found.
        let v = self.fb.ins().band_imm(tt, 0xf);
        let found = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.ins().brif(v, found, [], not_found, []);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set output register.
        let join = self.fb.create_block();
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(check_ud);
        self.fb.seal_block(check_ud);

        // Check if userdata.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 7);
        let load_ud = self.fb.create_block();

        self.fb.ins().brif(v, load_ud, [], not_found, []);

        self.fb.switch_to_block(load_ud);
        self.fb.seal_block(load_ud);

        // Load UserData::props.
        let props = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            o,
            offset_of!(UserData<A, dyn Any>, props) as i32,
        );

        self.fb.ins().brif(
            props,
            lookup_table,
            &[BlockArg::Value(props)],
            not_found,
            [],
        );

        self.fb.seal_block(lookup_table);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishget.
        self.finishget(i, pc, tab, k);

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn settabup(&mut self, i: u32, pc: usize) -> usize {
        let uv = self.load_uv(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_const(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let rc = if (i & 1 << 7 + 8) != 0 {
            self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        } else {
            self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        };

        // Load table type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            uv,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if table.
        let null = self.fb.ins().iconst(self.ptr, 0);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 5 | 0 << 4 | 1 << 6);
        let lookup_table = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.append_block_param(not_found, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, [], not_found, &[BlockArg::Value(null)]);

        self.fb.switch_to_block(lookup_table);
        self.fb.seal_block(lookup_table);

        // Set up arguments for luaH_getshortstr.
        let args = [
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                uv,
                offset_of!(UnsafeValue<A>, value_) as i32,
            ),
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                rb,
                offset_of!(UnsafeValue<A>, value_) as i32,
            ),
        ];

        // Invoke luaH_getshortstr.
        let slot = self.fb.ins().call(self.getshortstr, &args);
        let slot = self.fb.inst_results(slot)[0];
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check result.
        let v = self.fb.ins().band_imm(v, 0xf);
        let found = self.fb.create_block();

        self.fb
            .ins()
            .brif(v, found, [], not_found, &[BlockArg::Value(slot)]);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            rc,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Set value.
        let join = self.fb.create_block();
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            rc,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().call(self.barrier_back, &[uv, rc]);
        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishset.
        let slot = self.fb.block_params(not_found)[0];
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.update_top_from_ci();
        self.update_pc(pc);

        self.fb
            .ins()
            .call(self.finishset, &[td, uv, rb, rc, slot, ret]);

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn settable(&mut self, i: u32, pc: usize) -> usize {
        let tab = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let key = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let val = if (i & 1 << 7 + 8) != 0 {
            self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        } else {
            self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        };

        // Load table type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if table.
        let null = self.fb.ins().iconst(self.ptr, 0);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 5 | 0 << 4 | 1 << 6);
        let lookup_table = self.fb.create_block();
        let not_found = self.fb.create_block();
        let join = self.fb.create_block();

        self.fb.append_block_param(not_found, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, [], not_found, &[BlockArg::Value(null)]);

        self.fb.switch_to_block(lookup_table);
        self.fb.seal_block(lookup_table);

        // Set up arguments for luaH_get.
        let args = [
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                tab,
                offset_of!(StackValue<A>, value_) as i32,
            ),
            key,
        ];

        // Invoke luaH_get.
        let slot = self.fb.ins().call(self.lookup_table, &args);
        let slot = self.fb.inst_results(slot)[0];
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check result.
        let v = self.fb.ins().band_imm(v, 0xf);
        let found = self.fb.create_block();

        self.fb
            .ins()
            .brif(v, found, [], not_found, &[BlockArg::Value(slot)]);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            val,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Set value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            val,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().call(self.barrier_back, &[tab, val]);
        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaV_finishset.
        let slot = self.fb.block_params(not_found)[0];
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.finishset, &[td, tab, key, val, slot, ret]);

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn seti(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let c = i >> 7 + 8 + 1 & !(!(0u32) << 8);
        let rc = if (i & 1 << 0 + 7 + 8) != 0 {
            self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        } else {
            self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        };

        // Load type of RA.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if table.
        let null = self.fb.ins().iconst(self.ptr, 0);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 5 | 0 << 4 | 1 << 6);
        let lookup_table = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.append_block_param(not_found, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, [], not_found, &[BlockArg::Value(null)]);

        self.fb.switch_to_block(lookup_table);
        self.fb.seal_block(lookup_table);

        // Set up arguments for luaH_getint.
        let args = [
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted().with_can_move(),
                ra,
                offset_of!(StackValue<A>, value_) as i32,
            ),
            self.fb.ins().iconst(I64, i64::from(c)),
        ];

        // Invoke luaH_getint.
        let slot = self.fb.ins().call(self.getint, &args);
        let slot = self.fb.inst_results(slot)[0];
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if found.
        let v = self.fb.ins().band_imm(v, 0xf);
        let found = self.fb.create_block();

        self.fb
            .ins()
            .brif(v, found, [], not_found, &[BlockArg::Value(slot)]);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set slot type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            rc,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Set slot value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            rc,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Invoke barrier_back.
        let join = self.fb.create_block();

        self.fb.ins().call(self.barrier_back, &[ra, rc]);
        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Set key type.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);
        let key = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        self.fb
            .ins()
            .stack_store(v, key, offset_of!(UnsafeValue<A>, tt_) as i32);

        // Set key value.
        let v = self.fb.ins().iconst(I64, i64::from(c));

        self.fb
            .ins()
            .stack_store(v, key, offset_of!(UnsafeValue<A>, value_) as i32);

        // Invoke luaV_finishset.
        let slot = self.fb.block_params(not_found)[0];
        let td = self.fb.use_var(self.td);
        let key = self.fb.ins().stack_addr(self.ptr, key, 0);
        let ret = self.fb.use_var(self.ret);

        self.update_top_from_ci();
        self.update_pc(pc);

        self.fb
            .ins()
            .call(self.finishset, &[td, ra, key, rc, slot, ret]);

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn setfield(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_const(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let rc = if (i & 1 << 7 + 8) != 0 {
            self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        } else {
            self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        };

        // Load table type.
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if table.
        let null = self.fb.ins().iconst(self.ptr, 0);
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::Equal, tt, 5 | 0 << 4 | 1 << 6);
        let lookup_table = self.fb.create_block();
        let not_found = self.fb.create_block();

        self.fb.append_block_param(not_found, self.ptr);

        self.fb
            .ins()
            .brif(v, lookup_table, [], not_found, &[BlockArg::Value(null)]);

        self.fb.switch_to_block(lookup_table);
        self.fb.seal_block(lookup_table);

        // Set up arguments for luaH_getshortstr.
        let args = [
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                ra,
                offset_of!(StackValue<A>, value_) as i32,
            ),
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                rb,
                offset_of!(UnsafeValue<A>, value_) as i32,
            ),
        ];

        // Invoke luaH_getshortstr.
        let slot = self.fb.ins().call(self.getshortstr, &args);
        let slot = self.fb.inst_results(slot)[0];
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check result.
        let v = self.fb.ins().band_imm(tt, 0xf);
        let found = self.fb.create_block();

        self.fb
            .ins()
            .brif(v, found, [], not_found, &[BlockArg::Value(slot)]);

        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            rc,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Set value.
        let join = self.fb.create_block();
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            rc,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            slot,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().call(self.barrier_back, &[ra, rc]);
        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishset.
        let slot = self.fb.block_params(not_found)[0];
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.update_top_from_ci();
        self.update_pc(pc);

        self.fb
            .ins()
            .call(self.finishset, &[td, ra, rb, rc, slot, ret]);

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn newtable(&mut self, i: u32, mut pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let mut b = i >> 7 + 8 + 1 & !(!(0u32) << 8);
        let mut c = i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8);

        if b > 0 {
            b = 1 << b - 1;
        }

        if (i & 1 << 7 + 8) != 0 {
            let i = self.code[pc];

            c += (i >> 7 & !(!(0u32) << 8 + 8 + 1 + 8)) * ((1 << 8) - 1 + 1);
        }

        pc += 1;

        // Set top.
        let top = self
            .fb
            .ins()
            .iadd_imm(ra, size_of::<StackValue<A>>() as i64);

        self.set_top(top);

        // Create table.
        let td = self.fb.use_var(self.td);
        let t = self.fb.ins().call(self.create_table, &[td]);
        let t = self.fb.inst_results(t)[0];
        let tt = self.fb.ins().iconst(I8, 5 | 0 << 4 | 1 << 6);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            t,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        if b != 0 || c != 0 {
            // Invoke luaH_resize.
            let c = self.fb.ins().iconst(I32, i64::from(c));
            let b = self.fb.ins().iconst(I32, i64::from(b));

            self.fb.ins().call(self.resize_table, &[t, c, b]);
        }

        // Update top.
        let top = self
            .fb
            .ins()
            .iadd_imm(ra, size_of::<StackValue<A>>() as i64);

        self.set_top(top);

        // Trigger GC.
        self.fb.ins().call(self.gc, &[td]);

        self.update_base_stack();

        pc
    }

    pub unsafe fn self_(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tab = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let k = if (i & 1 << 7 + 8) != 0 {
            self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        } else {
            self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8))
        };

        // Store table type.
        let reg = self
            .fb
            .ins()
            .iadd_imm(ra, size_of::<StackValue<A>>() as i64);
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            reg,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Store table value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            reg,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check table type.
        let load_tab = self.fb.create_block();
        let check_ud = self.fb.create_block();
        let lookup_table = self.fb.create_block();
        let not_found = self.fb.create_block();
        let ty = self.fb.ins().band_imm(tt, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 5);

        self.fb.append_block_param(lookup_table, self.ptr);

        self.fb.ins().brif(v, load_tab, [], check_ud, []);

        self.fb.switch_to_block(load_tab);
        self.fb.seal_block(load_tab);

        // Load table.
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(lookup_table, &[BlockArg::Value(v)]);
        self.fb.switch_to_block(check_ud);
        self.fb.seal_block(check_ud);

        // Check if userdata.
        let check_props = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ty, 7);

        self.fb.ins().brif(v, check_props, [], not_found, []);
        self.fb.switch_to_block(check_props);
        self.fb.seal_block(check_props);

        // Load userdata.
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            tab,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load UserData::props.
        let props = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            v,
            offset_of!(UserData<A, dyn Any>, props) as i32,
        );

        self.fb.ins().brif(
            props,
            lookup_table,
            &[BlockArg::Value(props)],
            not_found,
            [],
        );

        self.fb.switch_to_block(lookup_table);
        self.fb.seal_block(lookup_table);

        // Get arguments for luaH_getstr.
        let args = [
            self.fb.block_params(lookup_table)[0],
            self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                k,
                offset_of!(UnsafeValue<A>, value_) as i32,
            ),
        ];

        // Invoke luaH_getstr.
        let v = self.fb.ins().call(self.getstr, &args);
        let res = self.fb.inst_results(v)[0];
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            res,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check result.
        let found = self.fb.create_block();
        let join = self.fb.create_block();
        let v = self.fb.ins().band_imm(tt, 0xf);

        self.fb.ins().brif(v, found, [], not_found, []);
        self.fb.switch_to_block(found);
        self.fb.seal_block(found);

        // Set output register.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            res,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(join, []);
        self.fb.switch_to_block(not_found);
        self.fb.seal_block(not_found);

        // Invoke luaV_finishget.
        self.finishget(i, pc, tab, k);

        self.fb.ins().jump(join, []);
        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn addi(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let v1 = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let imm = (i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)) as i32 - ((1 << 8) - 1 >> 1);
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let int_add = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, int_add, [], check_float, []);

        self.fb.switch_to_block(int_add);
        self.fb.seal_block(int_add);

        // Load integer.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Set output register.
        let v = self.fb.ins().iadd_imm(v, i64::from(imm));
        let set_type = self.fb.create_block();

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(set_type, []);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Check if float.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let float_add = self.fb.create_block();
        let not_num = self.fb.create_block();

        self.fb.ins().brif(v, float_add, [], not_num, []);

        self.fb.switch_to_block(float_add);
        self.fb.seal_block(float_add);

        // Load float.
        let v = self.fb.ins().load(
            F64,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Set output register.
        let imm = self.fb.ins().f64const(imm as f64);
        let v = self.fb.ins().fadd(v, imm);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(set_type, []);

        self.fb.switch_to_block(set_type);
        self.fb.seal_block(set_type);

        // Set output type.
        let label = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().jump(label, []);

        self.fb.switch_to_block(not_num);
        self.fb.seal_block(not_num);

        pc
    }

    pub unsafe fn modk(&mut self, i: u32, pc: usize) -> usize {
        let v1 = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let v2 = self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        // Load type of v1.
        let t1 = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Get metamethod skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if v1 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t1, 3 | 0 << 4);
        let check_v2 = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, check_v2, [], check_float, []);

        self.fb.switch_to_block(check_v2);
        self.fb.seal_block(check_v2);

        // Load type of v2;
        let t2 = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move().with_readonly(),
            v2,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if v2 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t2, 3 | 0 << 4);
        let load_v2 = self.fb.create_block();

        self.fb.ins().brif(v, load_v2, [], check_float, []);

        self.fb.switch_to_block(load_v2);
        self.fb.seal_block(load_v2);

        // Load int from v2.
        let i2 = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move().with_readonly(),
            v2,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Check if v2 zero.
        let mod_int = self.fb.create_block();
        let mod_zero = self.fb.create_block();

        self.fb.ins().brif(i2, mod_int, [], mod_zero, []);

        self.fb.switch_to_block(mod_int);
        self.fb.seal_block(mod_int);

        // Load int from v1.
        let i1 = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Set output type.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Perform v1 % v2.
        let v = self.fb.ins().srem(i1, i2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(mod_zero);
        self.fb.seal_block(mod_zero);
        self.fb.set_cold_block(mod_zero);

        // Set error.
        let ret = self.fb.use_var(self.ret);
        let v = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.update_top_from_ci();
        self.update_pc(pc);

        self.fb.ins().call(self.mod_zero, &[ret]);
        self.fb.ins().return_(&[v]);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Load floats.
        let join = self.fb.create_block();
        let n1 = self.load_num_as_float(v1, join, &[]);
        let n2 = self.load_num_as_float(v2, join, &[]);
        let v = self.fb.ins().iconst(I8, 3 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Perform v1 % v2.
        let v = self.fb.ins().call(self.mod_f, &[n1, n2]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn divk(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let v1 = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let v2 = self.get_const(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));

        // Load type of first opererand.
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load value of first operand.
        let n1 = self.fb.ins().load(
            F64,
            MemFlags::trusted().with_can_move(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if float.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let check_v2 = self.fb.create_block();
        let check_int = self.fb.create_block();
        let join = self.fb.create_block();

        self.fb.append_block_param(check_v2, F64);

        self.fb
            .ins()
            .brif(v, check_v2, &[BlockArg::Value(n1)], check_int, []);

        self.fb.switch_to_block(check_int);
        self.fb.seal_block(check_int);

        // Check if integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let load_int = self.fb.create_block();

        self.fb.ins().brif(v, load_int, [], join, []);

        self.fb.switch_to_block(load_int);
        self.fb.seal_block(load_int);

        // Load integer.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Convert to float.
        let v = self.fb.ins().fcvt_from_sint(F64, v);

        self.fb.ins().jump(check_v2, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_v2);
        self.fb.seal_block(check_v2);

        // Load type of second opererand.
        let n1 = self.fb.block_params(check_v2)[0];
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v2,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Load value of second operand.
        let n2 = self.fb.ins().load(
            F64,
            MemFlags::trusted().with_can_move(),
            v2,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if float.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let div = self.fb.create_block();
        let check_int = self.fb.create_block();

        self.fb.append_block_param(div, F64);
        self.fb.append_block_param(div, F64);

        self.fb.ins().brif(
            v,
            div,
            &[BlockArg::Value(n1), BlockArg::Value(n2)],
            check_int,
            [],
        );

        self.fb.switch_to_block(check_int);
        self.fb.seal_block(check_int);

        // Check if integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let load_int = self.fb.create_block();

        self.fb.ins().brif(v, load_int, [], join, []);

        self.fb.switch_to_block(load_int);
        self.fb.seal_block(load_int);

        // Load integer.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v2,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Convert to float.
        let v = self.fb.ins().fcvt_from_sint(F64, v);

        self.fb
            .ins()
            .jump(div, &[BlockArg::Value(n1), BlockArg::Value(v)]);

        self.fb.switch_to_block(div);
        self.fb.seal_block(div);

        // Set type.
        let &[n1, n2] = self.fb.block_params(div).as_array().unwrap();
        let v = self.fb.ins().iconst(I8, 3 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Perform div.
        let v = self.fb.ins().fdiv(n1, n2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Skip metamethod call.
        let label = self.fb.create_block();

        self.fb.ins().jump(label, []);

        assert!(self.labels.insert(pc + 1, label).is_none());

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn add(&mut self, i: u32, pc: usize) -> usize {
        let v1 = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let v2 = self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        // Load type of v1.
        let t1 = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Get metamethod skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if v1 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t1, 3 | 0 << 4);
        let check_v2 = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, check_v2, [], check_float, []);

        self.fb.switch_to_block(check_v2);
        self.fb.seal_block(check_v2);

        // Load type of v2.
        let t2 = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v2,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if v2 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t2, 3 | 0 << 4);
        let add_int = self.fb.create_block();

        self.fb.ins().brif(v, add_int, [], check_float, []);

        self.fb.switch_to_block(add_int);
        self.fb.seal_block(add_int);

        // Load integer from v1.
        let i1 = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load integer from v2.
        let i2 = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v2,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Set type.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set value.
        let v = self.fb.ins().iadd(i1, i2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Load floats.
        let join = self.fb.create_block();
        let n1 = self.load_num_as_float(v1, join, &[]);
        let n2 = self.load_num_as_float(v2, join, &[]);
        let v = self.fb.ins().iconst(I8, 3 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Store float.
        let v = self.fb.ins().fadd(n1, n2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn mul(&mut self, i: u32, pc: usize) -> usize {
        let v1 = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let v2 = self.get_reg(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8));
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        // Load type of v1.
        let t1 = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Get metamethod skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if v1 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t1, 3 | 0 << 4);
        let check_v2 = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, check_v2, [], check_float, []);

        self.fb.switch_to_block(check_v2);
        self.fb.seal_block(check_v2);

        // Load type of v2.
        let t2 = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            v2,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if v2 integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, t2, 3 | 0 << 4);
        let mul_int = self.fb.create_block();

        self.fb.ins().brif(v, mul_int, [], check_float, []);

        self.fb.switch_to_block(mul_int);
        self.fb.seal_block(mul_int);

        // Load integer from v1.
        let i1 = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v1,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load integer from v2.
        let i2 = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            v2,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Set output type.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set output value.
        let v = self.fb.ins().imul(i1, i2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Load floats.
        let join = self.fb.create_block();
        let n1 = self.load_num_as_float(v1, join, &[]);
        let n2 = self.load_num_as_float(v2, join, &[]);
        let v = self.fb.ins().iconst(I8, 3 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Store float.
        let v = self.fb.ins().fmul(n1, n2);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().jump(skip, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn mmbin(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let tm = self
            .fb
            .ins()
            .iconst(I32, i64::from(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)));

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaT_trybinTM.
        let td = self.fb.use_var(self.td);
        let out = self.fb.ins().stack_addr(self.ptr, val, 0);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.trybinTM, &[td, ra, rb, tm, out, ret]);

        self.return_on_err();
        self.update_base_stack();

        // Set output type.
        let pi = self.code[pc - 2];
        let out = self.get_reg(pi >> 7 & !(!(0u32) << 8));
        let v = self
            .fb
            .ins()
            .stack_load(I8, val, offset_of!(UnsafeValue<A>, tt_) as i32);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            out,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set output value.
        let v = self
            .fb
            .ins()
            .stack_load(I64, val, offset_of!(UnsafeValue<A>, value_) as i32);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            out,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn mmbini(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let imm = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32 - ((1 << 8) - 1 >> 1);
        let tm = i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8);
        let flip = i >> 7 + 8 & !(!(0u32) << 1);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        // Invoke luaT_trybiniTM.
        let td = self.fb.use_var(self.td);
        let imm = self.fb.ins().iconst(I64, i64::from(imm));
        let flip = self.fb.ins().iconst(I32, i64::from(flip));
        let tm = self.fb.ins().iconst(I32, i64::from(tm));
        let out = self.fb.ins().stack_addr(self.ptr, val, 0);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.trybiniTM, &[td, ra, imm, flip, tm, out, ret]);

        self.return_on_err();
        self.update_base_stack();

        // Set output type.
        let pi = self.code[pc - 2];
        let out = self.get_reg(pi >> 7 & !(!(0u32) << 8));
        let v = self
            .fb
            .ins()
            .stack_load(I8, val, offset_of!(UnsafeValue<A>, tt_) as i32);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            out,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set output value.
        let v = self
            .fb
            .ins()
            .stack_load(I64, val, offset_of!(UnsafeValue<A>, value_) as i32);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            out,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn mmbink(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let pi = self.code[pc - 2];
        let imm = self.get_const(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let tm = self
            .fb
            .ins()
            .iconst(I32, i64::from(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)));
        let flip = self
            .fb
            .ins()
            .iconst(I32, i64::from(i >> 7 + 8 & !(!(0u32) << 1)));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        // Invoke luaT_trybinassocTM.
        let td = self.fb.use_var(self.td);
        let out = self.fb.ins().stack_addr(self.ptr, val, 0);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.trybinassocTM, &[td, ra, imm, flip, tm, out, ret]);

        self.return_on_err();
        self.update_base_stack();

        // Set output register.
        let out = self.get_reg(pi >> 7 & !(!(0u32) << 8));
        let tt = self
            .fb
            .ins()
            .stack_load(I8, val, offset_of!(UnsafeValue<A>, tt_) as i32);
        let val = self
            .fb
            .ins()
            .stack_load(I64, val, offset_of!(UnsafeValue<A>, value_) as i32);

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            out,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            val,
            out,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn not(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            rb,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if false.
        let join = self.fb.create_block();
        let check_nil = self.fb.create_block();
        let set_true = self.fb.create_block();
        let set_false = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 1 | 0 << 4);

        self.fb.ins().brif(v, set_true, [], check_nil, []);
        self.fb.switch_to_block(check_nil);
        self.fb.seal_block(check_nil);

        // Check if nil.
        let v = self.fb.ins().band_imm(tt, 0xf);

        self.fb.ins().brif(v, set_false, [], set_true, []);
        self.fb.switch_to_block(set_true);
        self.fb.seal_block(set_true);

        // Set true.
        let v = self.fb.ins().iconst(I8, 1 | 1 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().jump(join, []);
        self.fb.switch_to_block(set_false);
        self.fb.seal_block(set_false);

        // Set false.
        let v = self.fb.ins().iconst(I8, 1 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().jump(join, []);
        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn len(&mut self, i: u32, pc: usize) -> usize {
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        // Invoke luaV_objlen.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);
        let val = self.fb.ins().stack_addr(self.ptr, val, 0);

        self.fb.ins().call(self.objlen, &[td, rb, val, ret]);

        self.return_on_err();
        self.update_base_stack();

        // Set output type.
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            val,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Set output value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            val,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        pc
    }

    pub unsafe fn close(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaF_close.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.close, &[td, ra, ret]);

        self.return_on_err();
        self.update_base_stack();

        pc
    }

    pub unsafe fn tbc(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaF_newtbcupval.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.newtbcupval, &[td, ra, ret]);

        self.return_on_err();

        pc
    }

    pub unsafe fn jmp(&mut self, i: u32, pc: usize) -> usize {
        let dest = pc.wrapping_add_signed(
            ((i >> 7 & !(!(0u32) << 17 + 8)) as i32 - ((1 << 17 + 8) - 1 >> 1)) as isize,
        );

        // Get destination label.
        let label = match self.labels.entry(dest) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        self.fb.ins().jump(label, []);

        // There will be more instructions when OP_JMP was generated by "break" statement like the
        // following code:
        //
        // for i = 1, 1 do
        //   print(i)
        //   break
        //   print('abc')
        // end
        //
        // The instructions for "print('abc')" was not eliminated by Lua compiler so we need to
        // create a block for it. This block will be eliminated by Cranelift because it is
        // unreachable.
        if !self.labels.contains_key(&pc) {
            let b = self.fb.create_block();

            self.fb.switch_to_block(b);
            self.fb.seal_block(b);
        }

        pc
    }

    pub unsafe fn lt(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));

        // Load type of RA.
        let ta = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load type of RB.
        let tb = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            rb,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if RA integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ta, 3 | 0 << 4);
        let check_rb = self.fb.create_block();
        let check_ra = self.fb.create_block();

        self.fb.ins().brif(v, check_rb, [], check_ra, []);

        self.fb.switch_to_block(check_rb);
        self.fb.seal_block(check_rb);

        // Check if RB integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tb, 3 | 0 << 4);
        let cmp_int = self.fb.create_block();

        self.fb.ins().brif(v, cmp_int, [], check_ra, []);

        self.fb.switch_to_block(cmp_int);
        self.fb.seal_block(cmp_int);

        // Load integer from RA.
        let ia = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load integer from RB.
        let ib = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            rb,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare.
        let v = self.fb.ins().icmp(IntCC::SignedLessThan, ia, ib);
        let check_result = self.fb.create_block();

        self.fb.append_block_param(check_result, I8);

        self.fb.ins().jump(check_result, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_ra);
        self.fb.seal_block(check_ra);

        // Check if RA number.
        let v = self.fb.ins().band_imm(ta, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 3);
        let check_rb = self.fb.create_block();
        let not_num = self.fb.create_block();

        self.fb.ins().brif(v, check_rb, [], not_num, []);

        self.fb.switch_to_block(check_rb);
        self.fb.seal_block(check_rb);

        // Check if RB number.
        let v = self.fb.ins().band_imm(tb, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 3);
        let cmp_num = self.fb.create_block();

        self.fb.ins().brif(v, cmp_num, [], not_num, []);

        self.fb.switch_to_block(cmp_num);
        self.fb.seal_block(cmp_num);

        // Invoke LTnum.
        let v = self.fb.ins().call(self.LTnum, &[ra, rb]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().jump(check_result, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(not_num);
        self.fb.seal_block(not_num);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke lessthanothers.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);
        let v = self.fb.ins().call(self.lessthanothers, &[td, ra, rb, ret]);
        let v = self.fb.inst_results(v)[0];

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(check_result, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_result);
        self.fb.seal_block(check_result);

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let cond = self.fb.block_params(check_result)[0];
        let jump = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], jump, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(jump);
        self.fb.seal_block(jump);

        pc
    }

    pub unsafe fn le(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));

        // Load type of RA.
        let ta = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Load type of RB.
        let tb = self.fb.ins().load(
            I8,
            MemFlags::trusted().with_can_move(),
            rb,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if RA integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, ta, 3 | 0 << 4);
        let check_rb = self.fb.create_block();
        let check_num = self.fb.create_block();

        self.fb.ins().brif(v, check_rb, [], check_num, []);

        self.fb.switch_to_block(check_rb);
        self.fb.seal_block(check_rb);

        // Check if RB integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tb, 3 | 0 << 4);
        let cmp_int = self.fb.create_block();

        self.fb.ins().brif(v, cmp_int, [], check_num, []);

        self.fb.switch_to_block(cmp_int);
        self.fb.seal_block(cmp_int);

        // Load integer from RA.
        let lhs = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load integer from RB.
        let rhs = self.fb.ins().load(
            I64,
            MemFlags::trusted().with_can_move(),
            rb,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare integer.
        let v = self.fb.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
        let check_res = self.fb.create_block();

        self.fb.append_block_param(check_res, I8);

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_num);
        self.fb.seal_block(check_num);

        // Check if RA number.
        let v = self.fb.ins().band_imm(ta, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 3);
        let check_rb = self.fb.create_block();
        let not_num = self.fb.create_block();

        self.fb.ins().brif(v, check_rb, [], not_num, []);

        self.fb.switch_to_block(check_rb);
        self.fb.seal_block(check_rb);

        // Check if RB number.
        let v = self.fb.ins().band_imm(tb, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 3);
        let cmp_num = self.fb.create_block();

        self.fb.ins().brif(v, cmp_num, [], not_num, []);

        self.fb.switch_to_block(cmp_num);
        self.fb.seal_block(cmp_num);

        // Invoke LEnum.
        let v = self.fb.ins().call(self.LEnum, &[ra, rb]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(not_num);
        self.fb.seal_block(not_num);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke lessequalothers.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);
        let v = self.fb.ins().call(self.lessequalothers, &[td, ra, rb, ret]);
        let v = self.fb.inst_results(v)[0];

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_res);
        self.fb.seal_block(check_res);

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let cond = self.fb.block_params(check_res)[0];
        let join = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], join, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn eq(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_reg(i >> 7 + 8 + 1 & !(!(0u32) << 8));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaV_equalobj.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);
        let cond = self.fb.ins().call(self.equalobj, &[td, ra, rb, ret]);
        let cond = self.fb.inst_results(cond)[0];

        self.return_on_err();
        self.update_base_stack();

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let jump = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], jump, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(jump);
        self.fb.seal_block(jump);

        pc
    }

    pub unsafe fn eqk(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let rb = self.get_const(i >> 7 + 8 + 1 & !(!(0u32) << 8));

        // Invoke luaV_equalobj.
        let td = self.fb.ins().iconst(self.ptr, 0);
        let ret = self.fb.use_var(self.ret);
        let cond = self.fb.ins().call(self.equalobj, &[td, ra, rb, ret]);
        let cond = self.fb.inst_results(cond)[0];

        self.return_on_err();
        self.update_base_stack();

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let jump = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], jump, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(jump);
        self.fb.seal_block(jump);

        pc
    }

    pub unsafe fn eqi(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let im = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32 - ((1 << 8) - 1 >> 1);
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if integer.
        let cmp_int = self.fb.create_block();
        let check_float = self.fb.create_block();
        let check_res = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);

        self.fb.append_block_param(check_res, I8);

        self.fb.ins().brif(v, cmp_int, [], check_float, []);
        self.fb.switch_to_block(cmp_int);
        self.fb.seal_block(cmp_int);

        // Load integer.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, im as i64);

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);
        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Check if float.
        let z = self.fb.ins().iconst(I8, 0);
        let cmp_float = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);

        self.fb
            .ins()
            .brif(v, cmp_float, [], check_res, &[BlockArg::Value(z)]);
        self.fb.switch_to_block(cmp_float);
        self.fb.seal_block(cmp_float);

        // Load float.
        let v = self.fb.ins().load(
            F64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare float.
        let im = self.fb.ins().f64const(im as f64);
        let v = self.fb.ins().fcmp(FloatCC::Equal, v, im);

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);
        self.fb.switch_to_block(check_res);
        self.fb.seal_block(check_res);

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let jump = self.fb.create_block();
        let cond = self.fb.block_params(check_res)[0];
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], jump, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(jump);
        self.fb.seal_block(jump);

        pc
    }

    pub unsafe fn gti(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let im = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32 - ((1 << 8) - 1 >> 1);
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let cmp_int = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, cmp_int, [], check_float, []);

        self.fb.switch_to_block(cmp_int);
        self.fb.seal_block(cmp_int);

        // Load integer.
        let lhs = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if greater.
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::UnsignedGreaterThan, lhs, i64::from(im));
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::NotEqual, v, i64::from(i >> 7 + 8 & !(!(0u32) << 1)));
        let join = self.fb.create_block();

        self.fb.ins().brif(v, skip, [], join, []);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Check if float.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let cmp_float = self.fb.create_block();
        let invoke_mt = self.fb.create_block();

        self.fb.ins().brif(v, cmp_float, [], invoke_mt, []);

        self.fb.switch_to_block(cmp_float);
        self.fb.seal_block(cmp_float);

        // Load float.
        let lhs = self.fb.ins().load(
            F64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if greater.
        let rhs = self.fb.ins().f64const(im as f64);
        let v = self.fb.ins().fcmp(FloatCC::GreaterThan, lhs, rhs);
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::NotEqual, v, i64::from(i >> 7 + 8 & !(!(0u32) << 1)));

        self.fb.ins().brif(v, skip, [], join, []);

        self.fb.switch_to_block(invoke_mt);
        self.fb.seal_block(invoke_mt);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaT_callorderiTM.
        let td = self.fb.use_var(self.td);
        let im = self.fb.ins().iconst(I32, i64::from(im));
        let one = self.fb.ins().iconst(I32, 1);
        let float = self
            .fb
            .ins()
            .iconst(I32, i64::from(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)));
        let event = self.fb.ins().iconst(I32, i64::from(TM_LT));
        let ret = self.fb.use_var(self.ret);
        let v = self
            .fb
            .ins()
            .call(self.callorderiTM, &[td, ra, im, one, float, event, ret]);
        let v = self.fb.inst_results(v)[0];

        self.return_on_err();
        self.update_base_stack();

        // Check result.
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::NotEqual, v, i64::from(i >> 7 + 8 & !(!(0u32) << 1)));

        self.fb.ins().brif(v, skip, [], join, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn gei(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let im = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32 - ((1 << 8) - 1 >> 1);
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Check if integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let cmp_int = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(v, cmp_int, [], check_float, []);

        self.fb.switch_to_block(cmp_int);
        self.fb.seal_block(cmp_int);

        // Load integer.
        let lhs = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare integer.
        let v = self
            .fb
            .ins()
            .icmp_imm(IntCC::SignedGreaterThanOrEqual, lhs, i64::from(im));
        let check_res = self.fb.create_block();

        self.fb.append_block_param(check_res, I8);

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Check if float.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let cmp_float = self.fb.create_block();
        let invoke_mt = self.fb.create_block();

        self.fb.ins().brif(v, cmp_float, [], invoke_mt, []);

        self.fb.switch_to_block(cmp_float);
        self.fb.seal_block(cmp_float);

        // Load float.
        let lhs = self.fb.ins().load(
            F64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Compare float.
        let v = self.fb.ins().f64const(im as f64);
        let v = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, lhs, v);

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(invoke_mt);
        self.fb.seal_block(invoke_mt);

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaT_callorderiTM.
        let td = self.fb.use_var(self.td);
        let im = self.fb.ins().iconst(I32, i64::from(im));
        let flip = self.fb.ins().iconst(I32, 1);
        let isf = self
            .fb
            .ins()
            .iconst(I32, i64::from(i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)));
        let event = self.fb.ins().iconst(I32, i64::from(TM_LE));
        let ret = self.fb.use_var(self.ret);
        let v = self
            .fb
            .ins()
            .call(self.callorderiTM, &[td, ra, im, flip, isf, event, ret]);
        let v = self.fb.inst_results(v)[0];

        self.return_on_err();
        self.update_base_stack();

        self.fb.ins().jump(check_res, &[BlockArg::Value(v)]);

        self.fb.switch_to_block(check_res);
        self.fb.seal_block(check_res);

        // Skip jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check result.
        let cond = self.fb.block_params(check_res)[0];
        let join = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], join, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn test(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Test for false.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, tt, 1 | 0 << 4);
        let cond = self.fb.ins().iconst(I8, 0);
        let cond = self.fb.ins().bor(cond, v);

        // Test for nil.
        let v = self.fb.ins().band_imm(tt, 0xf);
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 0);
        let cond = self.fb.ins().bor(cond, v);
        let cond = self.fb.ins().bxor_imm(cond, 1);

        // Get jump skip.
        let skip = match self.labels.entry(pc + 1) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if jump skip.
        let next = self.fb.create_block();
        let v = self.fb.ins().icmp_imm(
            IntCC::NotEqual,
            cond,
            i64::from(i >> 7 + 8 & !(!(0u32) << 1)),
        );

        self.fb.ins().brif(v, skip, [], next, []);

        // Next instruction is OP_JMP.
        self.fb.switch_to_block(next);
        self.fb.seal_block(next);

        pc
    }

    pub unsafe fn call(&mut self, i: u32, pc: usize) -> usize {
        // Update top and PC.
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let args = (i >> 7 + 8 + 1 & 0xFF) as u8;
        let nresults = (i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)) as i32 - 1;

        if args != 0 {
            let top = self
                .fb
                .ins()
                .iadd_imm(ra, (usize::from(args) * size_of::<StackValue<A>>()) as i64);

            self.set_top(top);
        }

        self.update_pc(pc);

        // Invoke luaD_precall.
        let nresults = self.fb.ins().iconst(I32, i64::from(nresults));
        let td = self.fb.use_var(self.td);
        let ci = self.fb.use_var(self.ci);
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);
        let precall = self
            .fb
            .ins()
            .call(self.precall, &[td, ci, ra, nresults, cx, ret]);

        self.return_on_err();

        // Prepare to check Lua function.
        let check_lua = self.fb.create_block();

        self.fb.append_block_param(check_lua, self.ptr);

        // Check if pending.
        let v = self.fb.inst_results(precall)[0];
        let resume_precall = self.return_on_pending(check_lua, [&BlockArg::Value(v)]);

        self.fb.switch_to_block(check_lua);

        // Check if Lua function.
        let newci = self.fb.block_params(check_lua)[0];
        let run_lua = self.fb.create_block();
        let finished = self.fb.create_block();

        self.fb.ins().brif(newci, run_lua, [], finished, []);

        self.fb.switch_to_block(run_lua);
        self.fb.seal_block(run_lua);

        // Invoke run.
        let td = self.fb.use_var(self.td);
        let ci = self.fb.use_var(self.ci);
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.run_lua, &[td, ci, newci, cx, ret]);

        self.return_on_err();

        // Jump to finished.
        let resume_lua = self.return_on_pending(finished, []);

        self.fb.switch_to_block(resume_precall);

        // Resume luaD_precall.
        let ci = self.fb.use_var(self.ci);
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);
        let resume_precall = self.fb.ins().call(self.resume_precall, &[ci, cx, ret]);

        self.return_on_err();

        // Check if ready.
        let v = self.fb.inst_results(resume_precall)[0];

        self.join_on_ready(check_lua, [&BlockArg::Value(v)]);

        self.fb.seal_block(check_lua);
        self.fb.switch_to_block(resume_lua);

        // Resume run_lua.
        let ci = self.fb.use_var(self.ci);
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.resume_run_lua, &[ci, cx, ret]);

        self.return_on_err();
        self.join_on_ready(finished, []);

        // Update base stack.
        self.fb.switch_to_block(finished);
        self.fb.seal_block(finished);

        self.update_base_stack();

        pc
    }

    pub unsafe fn tailcall(&mut self, i: u32, mut pc: usize) -> usize {
        let td = self.fb.use_var(self.td);
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let b = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32;
        let b = if b != 0 {
            let top = b as usize * size_of::<StackValue<A>>();
            let top = self.fb.ins().iadd_imm(ra, top as i64);

            self.set_top(top);

            self.fb.ins().iconst(I32, i64::from(b))
        } else {
            // Load top.
            let top = self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                td,
                offset_of!(Thread<A>, top) as i32,
            );

            // Get distance from RA.
            let b = self.fb.ins().isub(top, ra);
            let b = self.fb.ins().udiv_imm(b, size_of::<StackValue<A>>() as i64);

            self.fb.ins().ireduce(I32, b)
        };

        // Get delta.
        let ci = self.fb.use_var(self.ci);
        let nparams1 = (i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8)) as i32;
        let delta = if nparams1 != 0 {
            let v = self.fb.ins().load(
                I32,
                MemFlags::trusted(),
                ci,
                offset_of!(CallInfo, nextraargs) as i32,
            );

            self.fb.ins().iadd_imm(v, i64::from(nparams1))
        } else {
            self.fb.ins().iconst(I32, 0)
        };

        // Invoke luaF_closeupval.
        if (i & 1 << 7 + 8) != 0 {
            let base = self.fb.use_var(self.base);

            self.fb.ins().call(self.closeupval, &[td, base]);
        }

        self.update_pc(pc);

        // Invoke luaD_pretailcall.
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);
        let ret = self
            .fb
            .ins()
            .call(self.pretailcall, &[td, ci, ra, b, delta, cx, ret]);
        let ready = self.fb.create_block();

        self.fb.append_block_param(ready, I32);
        self.fb.append_block_param(ready, I32);

        self.return_on_err();

        // Check if ready.
        let ret = self.fb.inst_results(ret)[0];
        let resume = self.return_on_pending(ready, &[BlockArg::Value(delta), BlockArg::Value(ret)]);

        self.fb.switch_to_block(resume);

        // Resume luaD_pretailcall.
        let ci = self.fb.use_var(self.ci);
        let cx = self.fb.use_var(self.cx);
        let ret = self.fb.use_var(self.ret);
        let ret = self.fb.ins().call(self.resume_pretailcall, &[ci, cx, ret]);
        let ret = self.fb.inst_results(ret)[0];

        self.return_on_err();

        // Check if ready.
        let delta = if nparams1 != 0 {
            let v = self.fb.ins().load(
                I32,
                MemFlags::trusted().with_can_move(),
                ci,
                offset_of!(CallInfo, nextraargs) as i32,
            );

            self.fb.ins().iadd_imm(v, i64::from(nparams1))
        } else {
            self.fb.ins().iconst(I32, 0)
        };

        self.join_on_ready(ready, &[BlockArg::Value(delta), BlockArg::Value(ret)]);

        self.fb.switch_to_block(ready);
        self.fb.seal_block(ready);

        // Check if Lua function.
        let finished = self.fb.create_block();
        let exit = self.fb.create_block();
        let r = self.fb.ins().iconst(I8, i64::from(Status::Replaced));
        let &[delta, n] = self.fb.block_params(ready).as_array().unwrap();
        let v = self.fb.ins().icmp_imm(IntCC::SignedLessThan, n, 0);

        self.fb.append_block_param(exit, I8);

        self.fb
            .ins()
            .brif(v, exit, &[BlockArg::Value(r)], finished, []);
        self.fb.switch_to_block(finished);
        self.fb.seal_block(finished);

        // Load CallInfo::func.
        let ci = self.fb.use_var(self.ci);
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ci,
            offset_of!(CallInfo, func) as i32,
        );

        // Update CallInfo::func.
        let delta = self.fb.ins().sextend(self.ptr, delta);
        let v = self.fb.ins().isub(v, delta);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ci,
            offset_of!(CallInfo, func) as i32,
        );

        // Invoke luaD_poscall.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.poscall, &[td, ci, n, ret]);
        self.return_on_err();

        // Jump to exit.
        let v = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.fb.ins().jump(exit, &[BlockArg::Value(v)]);
        self.fb.switch_to_block(exit);
        self.fb.seal_block(exit);

        // Exit.
        let ret = self.fb.block_params(exit)[0];

        self.fb.ins().return_(&[ret]);

        // Create block for remaining instructions.
        pc += 1; // Skip unused OP_RETURN.

        if let Entry::Vacant(e) = self.labels.entry(pc) {
            e.insert(self.fb.create_block());
        }

        pc
    }

    pub unsafe fn return_(&mut self, i: u32, pc: usize) -> usize {
        let ra = i >> 7 & !(!(0u32) << 8);
        let n = (i >> 7 + 8 + 1 & !(!(0u32) << 8)) as i32 - 1;
        let n = if n < 0 {
            // Load top.
            let td = self.fb.use_var(self.td);
            let top = self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                td,
                offset_of!(Thread<A>, top) as i32,
            );

            // Get number of values to return.
            let ra = unsafe { self.get_reg(ra) };
            let n = self.fb.ins().isub(top, ra);

            self.fb.ins().imul_imm(n, size_of::<StackValue<A>>() as i64)
        } else {
            self.fb.ins().iconst(I32, i64::from(n))
        };

        self.update_pc(pc);

        if (i & 1 << 7 + 8) != 0 {
            // Set CallInfo::u2::nres.
            let ci = self.fb.use_var(self.ci);

            self.fb.ins().store(
                MemFlags::trusted(),
                n,
                ci,
                offset_of!(CallInfo, u2.nres) as i32,
            );

            // Load stack pointers to compare.
            let new = self.get_ci_top();
            let td = self.fb.use_var(self.td);
            let current = self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                td,
                offset_of!(Thread<A>, top) as i32,
            );

            // Check if we need to update top.
            let cond = self.fb.ins().icmp(IntCC::UnsignedLessThan, current, new);
            let update = self.fb.create_block();
            let join = self.fb.create_block();

            self.fb.ins().brif(cond, update, [], join, []);

            self.fb.switch_to_block(update);
            self.fb.seal_block(update);

            // Update top.
            self.fb.ins().store(
                MemFlags::trusted(),
                new,
                td,
                offset_of!(Thread<A>, top) as i32,
            );

            self.fb.ins().jump(join, []);

            self.fb.switch_to_block(join);
            self.fb.seal_block(join);

            // Invoke luaF_close.
            let base = self.fb.use_var(self.base);
            let ret = self.fb.use_var(self.ret);

            self.fb.ins().call(self.close, &[td, base, ret]);

            self.return_on_err();
            self.update_base_stack();
        }

        // Update
        let nparams1 = i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8);

        if nparams1 != 0 {
            // Load CallInfo::nextraargs.
            let ci = self.fb.use_var(self.ci);
            let nextraargs = self.fb.ins().sload32(
                MemFlags::trusted(),
                ci,
                offset_of!(CallInfo, nextraargs) as i32,
            );

            // Load CallInfo::func.
            let func = self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                ci,
                offset_of!(CallInfo, func) as i32,
            );

            // Update CallInfo::func.
            let args = self.fb.ins().iadd_imm(nextraargs, i64::from(nparams1));
            let func = self.fb.ins().isub(func, args);

            self.fb.ins().store(
                MemFlags::trusted(),
                func,
                ci,
                offset_of!(CallInfo, func) as i32,
            );
        }

        // Update top.
        let ra = unsafe { self.get_reg(ra) };
        let top = self.fb.ins().imul_imm(n, size_of::<StackValue<A>>() as i64);
        let top = self.fb.ins().uextend(self.ptr, top);
        let top = self.fb.ins().iadd(ra, top);
        let td = self.fb.use_var(self.td);

        self.fb.ins().store(
            MemFlags::trusted(),
            top,
            td,
            offset_of!(Thread<A>, top) as i32,
        );

        // Invoke luaD_poscall.
        let ci = self.fb.use_var(self.ci);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.poscall, &[td, ci, n, ret]);

        // Emit return.
        let v = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.fb.ins().return_(&[v]);

        // Create block for remaining instructions.
        if let Entry::Vacant(e) = self.labels.entry(pc) {
            e.insert(self.fb.create_block());
        }

        pc
    }

    pub unsafe fn return0(&mut self, _: u32, pc: usize) -> usize {
        let ci = self.fb.use_var(self.ci);

        // Get top.
        let v = self
            .fb
            .ins()
            .iconst(self.ptr, size_of::<StackValue<A>>() as i64);
        let top = self.fb.use_var(self.base);
        let top = self.fb.ins().isub(top, v);

        // Load CallInfo::nresults.
        let nres = self.fb.ins().load(
            I16,
            MemFlags::trusted(),
            ci,
            offset_of!(CallInfo, nresults) as i32,
        );

        // Check if no more slot to fill.
        let v = self.fb.ins().icmp_imm(IntCC::SignedGreaterThan, nres, 0);
        let fill = self.fb.create_block();
        let exit = self.fb.create_block();

        self.fb.append_block_param(fill, I16);
        self.fb.append_block_param(fill, self.ptr);
        self.fb.append_block_param(exit, self.ptr);

        self.fb.ins().brif(
            v,
            fill,
            &[BlockArg::Value(nres), BlockArg::Value(top)],
            exit,
            &[BlockArg::Value(top)],
        );

        self.fb.switch_to_block(fill);

        // Set nil.
        let &[nres, top] = self.fb.block_params(fill).as_array().unwrap();
        let nil = self.fb.ins().iconst(I8, 0 | 0 << 4);

        self.fb.ins().store(
            MemFlags::trusted(),
            nil,
            top,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        // Increase top.
        let top = self
            .fb
            .ins()
            .iadd_imm(top, size_of::<StackValue<A>>() as i64);

        // Decrease nres.
        let one = self.fb.ins().iconst(I16, 1);
        let nres = self.fb.ins().isub(nres, one);

        self.fb.ins().brif(
            nres,
            fill,
            &[BlockArg::Value(nres), BlockArg::Value(top)],
            exit,
            &[BlockArg::Value(top)],
        );

        self.fb.seal_block(fill);
        self.fb.switch_to_block(exit);
        self.fb.seal_block(exit);

        // Set new top.
        let top = self.fb.block_params(exit)[0];

        self.set_top(top);

        // Set Thread::ci.
        let td = self.fb.use_var(self.td);
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ci,
            offset_of!(CallInfo, previous) as i32,
        );

        self.fb
            .ins()
            .store(MemFlags::trusted(), v, td, offset_of!(Thread<A>, ci) as i32);

        // Emit return.
        let v = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.fb.ins().return_(&[v]);

        // Create block for remaining instructions.
        if let Entry::Vacant(e) = self.labels.entry(pc) {
            e.insert(self.fb.create_block());
        }

        pc
    }

    pub unsafe fn forloop(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ra,
            (size_of::<StackValue<A>>() * 2 + offset_of!(StackValue<A>, tt_)) as i32,
        );

        // Check if step is integer.
        let v = self.fb.ins().icmp_imm(IntCC::Equal, v, 3 | 0 << 4);
        let int_step = self.fb.create_block();
        let float_step = self.fb.create_block();

        self.fb.ins().brif(v, int_step, [], float_step, []);

        self.fb.switch_to_block(int_step);
        self.fb.seal_block(int_step);

        // Load count.
        let count = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            (size_of::<StackValue<A>>() + offset_of!(StackValue<A>, value_)) as i32,
        );

        // Check if count > 0.
        let v = self.fb.ins().icmp_imm(IntCC::UnsignedGreaterThan, count, 0);
        let do_int = self.fb.create_block();
        let join = self.fb.create_block();

        self.fb.ins().brif(v, do_int, [], join, []);

        self.fb.switch_to_block(do_int);
        self.fb.seal_block(do_int);

        // Decrease count.
        let one = self.fb.ins().iconst(I64, 1);
        let v = self.fb.ins().isub(count, one);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ra,
            (size_of::<StackValue<A>>() + offset_of!(StackValue<A>, value_)) as i32,
        );

        // Load internal index.
        let idx = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Load step.
        let step = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ra,
            (size_of::<StackValue<A>>() * 2 + offset_of!(StackValue<A>, value_)) as i32,
        );

        // Increase internal index.
        let idx = self.fb.ins().iadd(idx, step);

        self.fb.ins().store(
            MemFlags::trusted(),
            idx,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Update constrol variable.
        let v = self.fb.ins().iconst(I8, 3 | 0 << 4);
        let ctrl = self
            .fb
            .ins()
            .iadd_imm(ra, (size_of::<StackValue<A>>() * 3) as i64);

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            ctrl,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            idx,
            ctrl,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Jump to body.
        let next = pc.wrapping_add_signed(-((i >> 7 + 8 & !(!(0u32) << 8 + 8 + 1)) as isize));
        let jump = self.labels.get(&next).copied().unwrap();

        self.fb.ins().jump(jump, []);

        self.fb.switch_to_block(float_step);
        self.fb.seal_block(float_step);

        // Invoke floatforloop.
        let v = self.fb.ins().call(self.floatforloop, &[ra]);
        let v = self.fb.inst_results(v)[0];

        self.fb.ins().brif(v, jump, [], join, []);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        pc
    }

    pub unsafe fn forprep(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke forprep.
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);
        let v = self.fb.ins().call(self.forprep, &[td, ra, ret]);
        let v = self.fb.inst_results(v)[0];

        self.return_on_err();

        // Get body skip.
        let skip = pc + ((i >> 7 + 8 & !(!(0u32) << 8 + 8 + 1)) + 1) as usize;
        let skip = match self.labels.entry(skip) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => *e.insert(self.fb.create_block()),
        };

        // Check if skip.
        let body = self.fb.create_block();

        assert!(self.labels.insert(pc, body).is_none());

        self.fb.ins().brif(v, skip, [], body, []);

        pc
    }

    pub unsafe fn setlist(&mut self, i: u32, mut pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let last = i >> 7 + 8 + 1 + 8 & !(!(0u32) << 8);
        let n = i >> 7 + 8 + 1 & !(!(0u32) << 8);
        let n = if n == 0 {
            // Get top of the stack.
            let td = self.fb.use_var(self.td);
            let top = self.fb.ins().load(
                self.ptr,
                MemFlags::trusted(),
                td,
                offset_of!(Thread<A>, top) as i32,
            );

            // Get distance from RA.
            let one = self.fb.ins().iconst(self.ptr, 1);
            let n = self.fb.ins().isub(top, ra);
            let n = self.fb.ins().udiv_imm(n, size_of::<StackValue<A>>() as i64);
            let n = self.fb.ins().isub(n, one);

            self.fb.ins().ireduce(I32, n)
        } else {
            self.update_top_from_ci();
            self.fb.ins().iconst(I32, i64::from(n))
        };

        // Get number of items.
        let last = self.fb.ins().iconst(I32, i64::from(last));
        let mut last = self.fb.ins().iadd(last, n);

        if (i & 1 << 7 + 8) != 0 {
            let i = self.code[pc];

            pc += 1;

            last = self.fb.ins().iadd_imm(
                last,
                i64::from((i >> 7 & !(!(0u32) << 8 + 8 + 1 + 8)) * ((1 << 8) - 1 + 1)),
            );
        }

        // Load target table.
        let h = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );

        // Check if number of items larger than array size.
        let v = self.fb.ins().call(self.realasize, &[h]);
        let v = self.fb.inst_results(v)[0];
        let v = self.fb.ins().icmp(IntCC::UnsignedGreaterThan, last, v);
        let resize = self.fb.create_block();
        let copy_loop = self.fb.create_block();

        self.fb.append_block_param(copy_loop, I32);
        self.fb.append_block_param(copy_loop, I32);

        self.fb.ins().brif(
            v,
            resize,
            [],
            copy_loop,
            &[BlockArg::Value(n), BlockArg::Value(last)],
        );

        self.fb.switch_to_block(resize);
        self.fb.seal_block(resize);

        // Invoke luaH_resizearray.
        self.fb.ins().call(self.resizearray, &[h, last]);
        self.fb
            .ins()
            .jump(copy_loop, &[BlockArg::Value(n), BlockArg::Value(last)]);

        self.fb.switch_to_block(copy_loop);

        // Get array.
        let &[n, last] = self.fb.block_params(copy_loop).as_array().unwrap();
        let array = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            h,
            offset_of!(Table<A>, array) as i32,
        );

        // Check remaining.
        let v = self.fb.ins().icmp_imm(IntCC::UnsignedGreaterThan, n, 0);
        let copy = self.fb.create_block();
        let end = self.fb.create_block();

        self.fb.ins().brif(v, copy, [], end, []);

        self.fb.switch_to_block(copy);
        self.fb.seal_block(copy);

        // Get source.
        let one = self.fb.ins().iconst(I32, 1);
        let last = self.fb.ins().isub(last, one);
        let v = self.fb.ins().uextend(self.ptr, n);
        let v = self.fb.ins().imul_imm(v, size_of::<StackValue<A>>() as i64);
        let src = self.fb.ins().iadd(ra, v);

        // Get destination.
        let v = self.fb.ins().uextend(self.ptr, last);
        let v = self
            .fb
            .ins()
            .imul_imm(v, size_of::<UnsafeValue<A>>() as i64);
        let dst = self.fb.ins().iadd(array, v);

        // Copy type.
        let v = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            src,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            dst,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Copy value.
        let v = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            src,
            offset_of!(StackValue<A>, value_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            v,
            dst,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb.ins().call(self.barrier_back, &[ra, src]);

        // Decrease remaining.
        let n = self.fb.ins().isub(n, one);

        self.fb
            .ins()
            .jump(copy_loop, &[BlockArg::Value(n), BlockArg::Value(last)]);

        self.fb.seal_block(copy_loop);
        self.fb.switch_to_block(end);
        self.fb.seal_block(end);

        pc
    }

    pub unsafe fn closure(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));

        // Load Proto::p.
        let p = self.fb.use_var(self.p);
        let p = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            p,
            offset_of!(Proto<A>, p) as i32,
        );

        // Load Proto for closure.
        let p = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            p,
            ((i >> 7 + 8 & !(!(0u32) << 8 + 8 + 1)) as usize * size_of::<*mut Proto<A>>()) as i32,
        );

        self.update_top_from_ci();

        // Invoke pushclosure.
        let td = self.fb.use_var(self.td);
        let f = self.fb.use_var(self.f);
        let base = self.fb.use_var(self.base);

        self.fb.ins().call(self.pushclosure, &[td, p, f, base, ra]);

        // Update top.
        let top = self
            .fb
            .ins()
            .iadd_imm(ra, size_of::<StackValue<A>>() as i64);

        self.set_top(top);

        // Trigger GC.
        self.fb.ins().call(self.gc, &[td]);

        self.update_base_stack();

        pc
    }

    pub unsafe fn vararg(&mut self, i: u32, pc: usize) -> usize {
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let n = (i >> 0 + 7 + 8 + 1 + 8 & !(!(0u32) << 8)) as i32 - 1;
        let n = self.fb.ins().iconst(I32, i64::from(n));

        self.update_top_from_ci();
        self.update_pc(pc);

        // Invoke luaT_getvarargs.
        let td = self.fb.use_var(self.td);
        let ci = self.fb.use_var(self.ci);
        let ret = self.fb.use_var(self.ret);

        self.fb.ins().call(self.getvarargs, &[td, ci, ra, n, ret]);

        self.return_on_err();
        self.update_base_stack();

        pc
    }

    pub unsafe fn varargprep(&mut self, i: u32, pc: usize) -> usize {
        // Set CallInfo::pc.
        self.update_pc(pc);

        // Get LuaFn::p.
        let func = self.fb.use_var(self.f);
        let proto = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            func,
            offset_of!(LuaFn<A>, p) as i32,
        );

        // Invoke luaT_adjustvarargs.
        let nfixparams = self
            .fb
            .ins()
            .iconst(I32, (i >> 0 + 7 & !(!(0u32) << 8) << 0) as i64);
        let td = self.fb.use_var(self.td);
        let ci = self.fb.use_var(self.ci);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.adjustvarargs, &[td, nfixparams, ci, proto, ret]);

        self.return_on_err();
        self.update_base_stack();

        pc
    }

    pub unsafe fn label(&mut self, _: u32, pc: usize) -> usize {
        if let Entry::Vacant(e) = self.labels.entry(pc) {
            e.insert(self.fb.create_block());
        }

        pc
    }

    fn finishget_with_key_parts(&mut self, i: u32, pc: usize, tab: Value, kt: Value, kv: Value) {
        // Set key.
        let key = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        self.fb
            .ins()
            .stack_store(kt, key, offset_of!(UnsafeValue<A>, tt_) as i32);
        self.fb
            .ins()
            .stack_store(kv, key, offset_of!(UnsafeValue<A>, value_) as i32);

        // Invoke luaV_finishget.
        let key = self.fb.ins().stack_addr(self.ptr, key, 0);

        self.finishget(i, pc, tab, key);
    }

    fn finishget(&mut self, i: u32, pc: usize, tab: Value, key: Value) {
        self.update_top_from_ci();
        self.update_pc(pc);

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        // Call luaV_finishget.
        let props_tried = self.fb.ins().iconst(I8, 1);
        let out = self.fb.ins().stack_addr(self.ptr, val, 0);
        let td = self.fb.use_var(self.td);
        let ret = self.fb.use_var(self.ret);

        self.fb
            .ins()
            .call(self.finishget, &[td, tab, key, props_tried, out, ret]);

        self.return_on_err();
        self.update_base_stack();

        // Write output register.
        let tt = self
            .fb
            .ins()
            .stack_load(I8, val, offset_of!(UnsafeValue<A>, tt_) as i32);
        let val = self
            .fb
            .ins()
            .stack_load(I64, val, offset_of!(UnsafeValue<A>, value_) as i32);
        let base = self.fb.use_var(self.base);
        let ra = self.fb.ins().iadd_imm(
            base,
            ((i >> 7 & !(!(0u32) << 8)) * size_of::<StackValue<A>>() as u32) as i64,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            tt,
            ra,
            offset_of!(StackValue<A>, tt_) as i32,
        );

        self.fb.ins().store(
            MemFlags::trusted(),
            val,
            ra,
            offset_of!(StackValue<A>, value_) as i32,
        );
    }

    fn return_on_err(&mut self) {
        // Emit branching.
        let tb = self.fb.create_block();
        let eb = self.fb.create_block();
        let ret = self.fb.use_var(self.ret);
        let obj = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ret,
            offset_of!(Error, obj) as i32,
        );

        self.fb.ins().brif(obj, tb, [], eb, []);
        self.fb.switch_to_block(tb);
        self.fb.seal_block(tb);

        // Emit return.
        let v = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.fb.ins().return_(&[v]);

        // Switch to else block.
        self.fb.switch_to_block(eb);
        self.fb.seal_block(eb);
    }

    /// Returns a block to resume the future. The caller must not seal this block.
    ///
    /// This can only be called after [Self::return_on_err()].
    #[must_use]
    unsafe fn return_on_pending<'c>(
        &mut self,
        ready: Block,
        args: impl IntoIterator<Item = &'c BlockArg>,
    ) -> Block {
        // Check Error::vtb.
        let pending = self.fb.create_block();
        let ret = self.fb.use_var(self.ret);
        let vtb = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ret,
            offset_of!(Error, vtb) as i32,
        );

        self.fb.ins().brif(vtb, pending, [], ready, args);

        self.fb.switch_to_block(pending);
        self.fb.seal_block(pending);

        // Emit return.
        let ret = self.fb.ins().iconst(I8, i64::from(Status::Finished));
        let resume = self.fb.ins().iconst(I32, self.resumes.len() as i64);
        let st = self.fb.use_var(self.st);

        self.fb.ins().store(
            MemFlags::trusted(),
            resume,
            st,
            offset_of!(State::<A>, next_block) as i32,
        );

        self.fb.ins().return_(&[ret]);

        // Create resume block.
        let resume = self.fb.create_block();

        self.resumes.push(self.fb.func.dfg.block_call(resume, []));

        resume
    }

    /// This can only be called after [Self::return_on_err()].
    fn join_on_ready<'c>(&mut self, ready: Block, args: impl IntoIterator<Item = &'c BlockArg>) {
        // Check Error::vtb.
        let pending = self.fb.create_block();
        let ret = self.fb.use_var(self.ret);
        let vtb = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ret,
            offset_of!(Error, vtb) as i32,
        );

        self.fb.ins().brif(vtb, pending, [], ready, args);
        self.fb.switch_to_block(pending);
        self.fb.seal_block(pending);

        // Emit return.
        let ret = self.fb.ins().iconst(I8, i64::from(Status::Finished));

        self.fb.ins().return_(&[ret]);
    }

    /// `(*th).top.set(th.stack.get().add((*ci).top.get()))`.
    fn update_top_from_ci(&mut self) {
        let top = self.get_ci_top();

        unsafe { self.set_top(top) };
    }

    unsafe fn set_top(&mut self, top: Value) {
        let td = self.fb.use_var(self.td);

        self.fb.ins().store(
            MemFlags::trusted(),
            top,
            td,
            offset_of!(Thread<A>, top) as i32,
        );
    }

    fn get_ci_top(&mut self) -> Value {
        // Load CallInfo::top.
        let ci = self.fb.use_var(self.ci);
        let top = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            ci,
            offset_of!(CallInfo, top) as i32,
        );

        // Load Thread::stack.
        let td = self.fb.use_var(self.td);
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            td,
            offset_of!(Thread<A>, stack) as i32,
        );

        // Get top.
        let top = self
            .fb
            .ins()
            .imul_imm(top, size_of::<StackValue<A>>() as i64);

        self.fb.ins().iadd(v, top)
    }

    /// `(*ci).pc = pc`.
    fn update_pc(&mut self, pc: usize) {
        let v = self.fb.ins().iconst(self.ptr, pc as i64);
        let ci = self.fb.use_var(self.ci);

        self.fb
            .ins()
            .store(MemFlags::trusted(), v, ci, offset_of!(CallInfo, pc) as i32);
    }

    /// `base = th.stack.get().add((*ci).func + 1)`.
    fn update_base_stack(&mut self) {
        // Get CallInfo::func.
        let v = self.fb.use_var(self.ci);
        let f = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(CallInfo, func) as i32,
        );

        // Get Thread::stack.
        let f = self.fb.ins().iadd_imm(f, 1);
        let f = self.fb.ins().imul_imm(f, size_of::<StackValue<A>>() as i64);
        let v = self.fb.use_var(self.td);
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted(),
            v,
            offset_of!(Thread<A>, stack) as i32,
        );

        // Update base stack.
        let v = self.fb.ins().iadd(v, f);

        self.fb.def_var(self.base, v);
    }

    unsafe fn load_num_as_float(&mut self, ptr: Value, not_num: Block, args: &[BlockArg]) -> Value {
        // Load type.
        let tt = self.fb.ins().load(
            I8,
            MemFlags::trusted(),
            ptr,
            offset_of!(UnsafeValue<A>, tt_) as i32,
        );

        // Check if integer.
        let c = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 0 << 4);
        let convert = self.fb.create_block();
        let check_float = self.fb.create_block();

        self.fb.ins().brif(c, convert, [], check_float, []);

        self.fb.switch_to_block(convert);
        self.fb.seal_block(convert);

        // Load integer.
        let i = self.fb.ins().load(
            I64,
            MemFlags::trusted(),
            ptr,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        // Convert integer to float.
        let f = self.fb.ins().fcvt_from_sint(F64, i);
        let join = self.fb.create_block();

        self.fb.append_block_param(join, F64);

        self.fb.ins().jump(join, &[BlockArg::Value(f)]);

        self.fb.switch_to_block(check_float);
        self.fb.seal_block(check_float);

        // Check if float.
        let c = self.fb.ins().icmp_imm(IntCC::Equal, tt, 3 | 1 << 4);
        let f = self.fb.ins().load(
            F64,
            MemFlags::trusted().with_can_move(),
            ptr,
            offset_of!(UnsafeValue<A>, value_) as i32,
        );

        self.fb
            .ins()
            .brif(c, join, &[BlockArg::Value(f)], not_num, args);

        self.fb.switch_to_block(join);
        self.fb.seal_block(join);

        self.fb.block_params(join)[0]
    }

    /// Returns a pointer to target constant, which is a pointer to [UnsafeValue].
    fn get_const(&mut self, idx: u32) -> Value {
        let k = self.fb.use_var(self.k);

        self.fb
            .ins()
            .iadd_imm(k, (idx * size_of::<UnsafeValue<A>>() as u32) as i64)
    }

    /// Load constant value.
    unsafe fn load_const(&mut self, idx: u32, off: usize) -> Value {
        let k = self.fb.use_var(self.k);

        self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            k,
            (idx as usize * size_of::<UnsafeValue<A>>() + off) as i32,
        )
    }

    /// Returns a pointer to [UpVal].
    unsafe fn get_uv(&mut self, idx: u32) -> Value {
        // Load LuaFn::upvals.
        let f = self.fb.use_var(self.f);
        let v = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            f,
            offset_of!(LuaFn<A>, upvals) as i32,
        );

        // Load UpVal.
        self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            (idx as usize * size_of::<*mut UpVal<A>>()) as i32,
        )
    }

    /// Load [UpVal::v], which is a pointer to [UnsafeValue].
    unsafe fn load_uv(&mut self, idx: u32) -> Value {
        let v = self.get_uv(idx);

        self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            v,
            offset_of!(UpVal<A>, v) as i32,
        )
    }

    /// Returns a pointer to target register.
    unsafe fn get_reg(&mut self, idx: u32) -> Value {
        let base = self.fb.use_var(self.base);

        self.fb
            .ins()
            .iadd_imm(base, (idx * size_of::<StackValue<A>>() as u32) as i64)
    }
}

impl<'a, 'b, A> Drop for Emitter<'a, 'b, A> {
    fn drop(&mut self) {
        for l in self.labels.values().copied() {
            self.fb.seal_block(l);
        }
    }
}
