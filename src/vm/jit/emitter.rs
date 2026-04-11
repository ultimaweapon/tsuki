use super::{Error, HOST, RustFuncs, State};
use crate::lobject::{Proto, UpVal};
use crate::lstate::CallInfo;
use crate::value::UnsafeValue;
use crate::{LuaFn, StackValue, Thread, UserData, luaH_resize};
use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;
use core::mem::offset_of;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types::{I8, I32, I64};
use cranelift_codegen::ir::{
    Block, BlockArg, BlockCall, FuncRef, InstBuilder, MemFlags, StackSlotData, StackSlotKind, Type,
    Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};

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
    getshortstr: FuncRef,
    precall: FuncRef,
    resume_precall: FuncRef,
    run_lua: FuncRef,
    resume_run_lua: FuncRef,
    close: FuncRef,
    poscall: FuncRef,
    pushclosure: FuncRef,
    gc: FuncRef,
    create_table: FuncRef,
    resize_table: FuncRef,
    ptr: Type,
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
        funcs: &'a mut RustFuncs<A>,
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
            adjustvarargs: funcs.import(
                fb,
                &[ptr, I32, ptr, ptr, ptr],
                None,
                super::adjustvarargs::<A> as *const u8,
            ),
            getvarargs: funcs.import(
                fb,
                &[ptr, ptr, ptr, I32, ptr],
                None,
                super::getvarargs::<A> as *const u8,
            ),
            finishget: funcs.import(
                fb,
                &[ptr, ptr, ptr, I8, ptr, ptr],
                None,
                super::finishget::<A> as *const u8,
            ),
            getshortstr: funcs.import(
                fb,
                &[ptr, ptr],
                Some(ptr),
                super::getshortstr::<A> as *const u8,
            ),
            precall: funcs.import(
                fb,
                &[ptr, ptr, ptr, I32, ptr, ptr],
                Some(ptr),
                super::precall::<A> as *const u8,
            ),
            resume_precall: funcs.import(
                fb,
                &[ptr, ptr, ptr],
                Some(ptr),
                super::resume_precall::<A> as *const u8,
            ),
            run_lua: funcs.import(
                fb,
                &[ptr, ptr, ptr, ptr],
                None,
                super::run_lua::<A> as *const u8,
            ),
            resume_run_lua: funcs.import(
                fb,
                &[ptr, ptr, ptr],
                None,
                super::resume_run_lua::<A> as *const u8,
            ),
            close: funcs.import(fb, &[ptr, ptr, ptr], None, super::close::<A> as *const u8),
            poscall: funcs.import(
                fb,
                &[ptr, ptr, I32, ptr],
                None,
                super::poscall::<A> as *const u8,
            ),
            pushclosure: funcs.import(
                fb,
                &[ptr, ptr, ptr, ptr, ptr],
                None,
                super::pushclosure::<A> as *const u8,
            ),
            gc: funcs.import(fb, &[ptr], None, super::step_gc::<A> as *const u8),
            create_table: funcs.import(
                fb,
                &[ptr],
                Some(ptr),
                super::create_table::<A> as *const u8,
            ),
            resize_table: funcs.import(fb, &[ptr, I32, I32], None, luaH_resize::<A> as *const u8),
            ptr,
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

    pub unsafe fn loadi(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    pub unsafe fn loadk(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    pub unsafe fn gettabup(&mut self, i: u32, pc: usize) -> Option<usize> {
        // Get output register and key.
        let ra = self.get_reg(i >> 7 & !(!(0u32) << 8));
        let k = self.load_const(
            i >> 24 & !(!(0u32) << 8),
            offset_of!(UnsafeValue<A>, value_),
        );

        // Load LuaFn::upvals.
        let func = self.fb.use_var(self.f);
        let vals = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            func,
            offset_of!(LuaFn<A>, upvals) as i32,
        );

        // Load UpVal.
        let uv = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            vals,
            ((i >> 7 + 8 + 1 & !(!(0u32) << 8) << 0) * size_of::<usize>() as u32) as i32,
        );

        // Load UpVal::v.
        let tab = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move(),
            uv,
            offset_of!(UpVal<A>, v) as i32,
        );

        // Load UnsafeValue::tt_.
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

        self.finishget(i, pc, tab, v, k);

        self.fb.ins().jump(next_inst, []);

        self.fb.switch_to_block(next_inst);
        self.fb.seal_block(next_inst);

        Some(pc)
    }

    pub unsafe fn newtable(&mut self, i: u32, mut pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    pub unsafe fn call(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        self.fb.ins().call(self.run_lua, &[td, ci, cx, ret]);

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

        Some(pc)
    }

    pub unsafe fn r#return(&mut self, i: u32, pc: usize) -> Option<usize> {
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

            self.fb.ins().call(self.close, &[td, base]);

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
        self.fb.ins().return_(&[]);

        None
    }

    pub unsafe fn closure(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    pub unsafe fn vararg(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    pub unsafe fn varargprep(&mut self, i: u32, pc: usize) -> Option<usize> {
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

        Some(pc)
    }

    fn finishget(&mut self, i: u32, pc: usize, tab: Value, kt: Value, kv: Value) {
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

        self.update_top_from_ci();
        self.update_pc(pc);

        // Allocate buffer for result.
        let val = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size_of::<UnsafeValue<A>>() as u32,
            align_of::<UnsafeValue<A>>() as u8,
        ));

        // Call luaV_finishget.
        let key = self.fb.ins().stack_addr(self.ptr, key, 0);
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

        // Emit return.
        self.fb.switch_to_block(tb);
        self.fb.seal_block(tb);

        self.fb.ins().return_(&[]);

        // Switch to else block.
        self.fb.switch_to_block(eb);
        self.fb.seal_block(eb);
    }

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
        let resume = self.fb.ins().iconst(I32, self.resumes.len() as i64);
        let st = self.fb.use_var(self.st);

        self.fb.ins().store(
            MemFlags::trusted(),
            resume,
            st,
            offset_of!(State::<A>, next_block) as i32,
        );

        self.fb.ins().return_(&[]);

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

        // Emit return.
        self.fb.switch_to_block(pending);
        self.fb.seal_block(pending);

        self.fb.ins().return_(&[]);
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

    /// Returns a pointer to target constant.
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

    /// Returns a pointer to target register.
    unsafe fn get_reg(&mut self, idx: u32) -> Value {
        let base = self.fb.use_var(self.base);

        self.fb
            .ins()
            .iadd_imm(base, (idx * size_of::<StackValue<A>>() as u32) as i64)
    }
}
