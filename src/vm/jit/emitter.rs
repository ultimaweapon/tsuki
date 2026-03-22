use super::{Error, HOST, RustFuncs, State};
use crate::lobject::{Proto, UpVal};
use crate::lstate::CallInfo;
use crate::value::UnsafeValue;
use crate::{LuaFn, StackValue, Thread, UserData};
use core::any::Any;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, offset_of};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types::{I8, I32, I64};
use cranelift_codegen::ir::{
    BlockArg, FuncRef, InstBuilder, MemFlags, StackSlotData, StackSlotKind, Type, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};

/// Contains state to emit Cranelift instructions for a Lua function.
pub struct Emitter<'a, A> {
    fb: ManuallyDrop<FunctionBuilder<'a>>,
    ret: Variable,
    td: Variable,
    ci: Variable,
    func: Variable,
    k: Variable,
    base: Variable,
    adjustvarargs: FuncRef,
    finishget: FuncRef,
    getshortstr: FuncRef,
    ptr: Type,
    phantom: PhantomData<A>,
}

impl<'a, A> Emitter<'a, A> {
    pub unsafe fn new(
        mut fb: FunctionBuilder<'a>,
        st: Variable,
        cx: Variable,
        ret: Variable,
        funcs: &mut RustFuncs<A>,
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
        let proto = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            v,
            offset_of!(LuaFn<A>, p) as i32,
        );

        // Load constants.
        let k = fb.declare_var(ptr);
        let v = fb.ins().load(
            ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            proto,
            offset_of!(Proto<A>, k) as i32,
        );

        fb.def_var(k, v);

        // Get base stack.
        let mut e = Self {
            ret,
            td,
            ci,
            func: f,
            k,
            base: fb.declare_var(ptr),
            adjustvarargs: funcs.import(
                &mut fb,
                &[ptr, I32, ptr, ptr, ptr],
                None,
                super::adjustvarargs::<A> as *const u8,
            ),
            finishget: funcs.import(
                &mut fb,
                &[ptr, ptr, ptr, I8, ptr, ptr],
                None,
                super::finishget::<A> as *const u8,
            ),
            getshortstr: funcs.import(
                &mut fb,
                &[ptr, ptr],
                Some(ptr),
                super::getshortstr::<A> as *const u8,
            ),
            ptr,
            fb: ManuallyDrop::new(fb),
            phantom: PhantomData,
        };

        e.update_base_stack();
        e
    }

    pub fn gettabup(&mut self, i: u32, pc: usize) -> usize {
        // Get output register.
        let base = self.fb.use_var(self.base);
        let ra = self.fb.ins().iadd_imm(
            base,
            ((i >> 7 & !(!(0u32) << 8)) * size_of::<StackValue<A>>() as u32) as i64,
        );

        // Load key.
        let k = self.fb.use_var(self.k);
        let k = self.fb.ins().load(
            self.ptr,
            MemFlags::trusted().with_can_move().with_readonly(),
            k,
            ((i >> 24 & !(!(0u32) << 8)) as usize * size_of::<UnsafeValue<A>>()
                + offset_of!(UnsafeValue<A>, value_)) as i32,
        );

        // Load LuaFn::upvals.
        let func = self.fb.use_var(self.func);
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

        pc
    }

    pub fn varargprep(&mut self, i: u32, pc: usize) -> usize {
        // Set CallInfo::pc.
        self.update_pc(pc);

        // Get LuaFn::p.
        let func = self.fb.use_var(self.func);
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

        self.update_top();
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

    fn update_top(&mut self) {
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

        // Set Thread::top.
        let top = self
            .fb
            .ins()
            .imul_imm(top, size_of::<StackValue<A>>() as i64);
        let top = self.fb.ins().iadd(v, top);
        let td = self.fb.use_var(self.td);

        self.fb.ins().store(
            MemFlags::trusted(),
            top,
            td,
            offset_of!(Thread<A>, top) as i32,
        );
    }

    fn update_pc(&mut self, pc: usize) {
        let v = self.fb.ins().iconst(self.ptr, pc as i64);
        let ci = self.fb.use_var(self.ci);

        self.fb
            .ins()
            .store(MemFlags::trusted(), v, ci, offset_of!(CallInfo, pc) as i32);
    }

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
}

impl<'a, A> Drop for Emitter<'a, A> {
    fn drop(&mut self) {
        // SAFETY: We don't touch fb after this.
        unsafe { ManuallyDrop::take(&mut self.fb).finalize() };
    }
}
