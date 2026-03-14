use super::{Error, HOST, RustFuncs, State};
use crate::lstate::CallInfo;
use crate::{LuaFn, StackValue, Thread};
use core::mem::{ManuallyDrop, offset_of};
use cranelift_codegen::ir::types::I32;
use cranelift_codegen::ir::{FuncRef, InstBuilder, MemFlags, Type};
use cranelift_frontend::{FunctionBuilder, Variable};

/// Contains state to compile a Lua function.
pub struct Compiler<'a> {
    fb: ManuallyDrop<FunctionBuilder<'a>>,
    ret: Variable,
    td: Variable,
    ci: Variable,
    func: Variable,
    base: Variable,
    adjustvarargs: FuncRef,
    ptr: Type,
}

impl<'a> Compiler<'a> {
    pub unsafe fn new<A>(
        mut fb: FunctionBuilder<'a>,
        st: Variable,
        cx: Variable,
        ret: Variable,
        funcs: &mut RustFuncs,
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

        // Get base stack.
        let mut c = Self {
            ret,
            td,
            ci,
            func: f,
            base: fb.declare_var(ptr),
            adjustvarargs: funcs.import(
                &mut fb,
                &[ptr, I32, ptr, ptr, ptr],
                None,
                super::adjustvarargs::<A> as *const u8,
            ),
            ptr,
            fb: ManuallyDrop::new(fb),
        };

        c.update_base_stack::<A>();
        c
    }

    pub fn emit_varargprep<A>(&mut self, i: u32, pc: usize) -> usize {
        // Set CallInfo::pc.
        let v = self.fb.ins().iconst(self.ptr, pc as i64);
        let ci = self.fb.use_var(self.ci);

        self.fb
            .ins()
            .store(MemFlags::trusted(), v, ci, offset_of!(CallInfo, pc) as i32);

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

        self.emit_return_on_err();
        self.update_base_stack::<A>();

        pc
    }

    fn emit_return_on_err(&mut self) {
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

    fn update_base_stack<A>(&mut self) {
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

impl<'a> Drop for Compiler<'a> {
    fn drop(&mut self) {
        // SAFETY: We don't touch fb after this.
        unsafe { ManuallyDrop::take(&mut self.fb).finalize() };
    }
}
