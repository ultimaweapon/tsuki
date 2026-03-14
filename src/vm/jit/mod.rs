use self::funcs::RustFuncs;
use super::OP_VARARGPREP;
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::ltm::luaT_adjustvarargs;
use crate::{Lua, LuaFn, StackValue, Thread};
use alloc::boxed::Box;
use core::mem::{offset_of, transmute};
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::{Context, Poll};
use cranelift_codegen::ir::types::I32;
use cranelift_codegen::ir::{AbiParam, FuncRef, Function, InstBuilder, MemFlags, Signature, Type};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, Variable};
use target_lexicon::Triple;

mod funcs;

pub async unsafe fn run<A>(
    td: &Thread<A>,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    // Check if already jitted.
    let f = (*td.stack.get().add((*ci).func))
        .value_
        .gc
        .cast::<LuaFn<A>>();
    let p = (*f).p.get();

    if (*p).jitted.is_null() {
        compile(td.hdr.global(), p);
    }

    // Invoke jitted function.
    let jitted = transmute((*p).jitted);
    let state = State { td, ci };

    Invoker { state, jitted }.await
}

#[inline(never)]
unsafe fn compile<A>(g: &Lua<A>, p: *mut Proto<A>) {
    // https://users.rust-lang.org/t/calling-a-rust-function-from-cranelift/103948/5.
    let mut sig = Signature::new(CallConv::triple_default(&HOST));
    let ptr = Type::triple_pointer_type(&HOST);

    sig.params.push(AbiParam::new(ptr)); // *mut State
    sig.params.push(AbiParam::new(ptr)); // *mut Context
    sig.params.push(AbiParam::new(ptr)); // *mut Error

    // Setup builder.
    let mut ctx = g.jit.builder_context.borrow_mut();
    let mut fun = Function::with_name_signature(Default::default(), sig);
    let mut fb = FunctionBuilder::new(&mut fun, &mut ctx);

    // Create entry block.
    let code = unsafe { core::slice::from_raw_parts((*p).code, (*p).sizecode as usize) };
    let entry = fb.create_block();

    fb.append_block_params_for_function_params(entry);
    fb.switch_to_block(entry);
    fb.seal_block(entry);

    // Load arguments.
    let st = fb.declare_var(ptr);
    let cx = fb.declare_var(ptr);
    let ret = fb.declare_var(ptr);

    fb.def_var(st, fb.block_params(entry)[0]);
    fb.def_var(cx, fb.block_params(entry)[1]);
    fb.def_var(ret, fb.block_params(entry)[2]);

    // Load td.
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

    // Compile instructions.
    let mut pc = 0;
    let mut funcs = RustFuncs::default();
    let base = fb.declare_var(ptr);
    let mut com = Compiler {
        ret,
        td,
        ci,
        func: f,
        base,
        adjustvarargs: funcs.import(
            &mut fb,
            &[ptr, I32, ptr, ptr, ptr],
            None,
            adjustvarargs::<A> as *const u8,
        ),
        fb,
        ptr,
    };

    com.update_base_stack::<A>();

    loop {
        let i = match code.get(pc).copied() {
            Some(v) => v,
            None => break,
        };

        pc += 1;

        pc = match i & 0x7F {
            OP_VARARGPREP => com.emit_varargprep::<A>(i, pc),
            v => todo!("{v}"),
        };
    }

    com.fb.finalize();

    drop(ctx);

    // Prepare to generate machine code.
    let mut ctx = g.jit.codegen_context.borrow_mut();

    ctx.func = fun;

    // Generate machine code.
    let code = ctx
        .compile(g.jit.isa.deref(), &mut Default::default())
        .unwrap();

    ctx.clear();

    todo!()
}

unsafe extern "C-unwind" fn adjustvarargs<A>(
    td: *const Thread<A>,
    nfixparams: i32,
    ci: *mut CallInfo,
    p: *const Proto<A>,
    ret: *mut Error,
) {
    if let Err(e) = luaT_adjustvarargs(td, nfixparams, ci, p) {
        (*ret).set_error(e);
    };
}

/// Contains state to compile a Lua function.
struct Compiler<'a> {
    ret: Variable,
    td: Variable,
    ci: Variable,
    func: Variable,
    base: Variable,
    adjustvarargs: FuncRef,
    fb: FunctionBuilder<'a>,
    ptr: Type,
}

impl<'a> Compiler<'a> {
    fn emit_varargprep<A>(&mut self, i: u32, pc: usize) -> usize {
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

/// Implementation of [Future] to invoke jitted function.
struct Invoker<A> {
    state: State<A>,
    jitted: unsafe extern "C-unwind" fn(*mut State<A>, *mut Context, *mut Error),
}

impl<A> Future for Invoker<A> {
    type Output = Result<(), Box<dyn core::error::Error>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = Error {
            obj: null_mut(),
            vtb: null_mut(),
        };

        unsafe { (self.jitted)(&mut self.state, cx, &mut r) };

        if !r.obj.is_null() {
            Poll::Ready(Err(unsafe { Box::from_raw(transmute(r)) }))
        } else if r.vtb.is_null() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

/// State of a call to jitted function.
#[repr(C)]
struct State<A> {
    td: *const Thread<A>,
    ci: *mut CallInfo,
}

/// Contains error from jitted function.
///
/// This struct must have the same layout as a pointer to trait object.
#[repr(C)]
#[derive(Clone, Copy)]
struct Error {
    obj: *mut (),
    vtb: *const (),
}

impl Error {
    fn set_error(&mut self, e: Box<dyn core::error::Error>) {
        *self = unsafe { transmute(Box::into_raw(e)) };
    }
}

const HOST: Triple = Triple::host();
