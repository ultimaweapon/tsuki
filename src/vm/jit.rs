use crate::lobject::Jitted;
use crate::{Lua, LuaFn};
use alloc::vec::Vec;
use cranelift_codegen::ir::{AbiParam, Function, Signature, Type};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use target_lexicon::Triple;

pub unsafe fn compile<A>(g: &Lua<A>, lf: *const LuaFn<A>) -> Vec<Jitted<A>> {
    // https://users.rust-lang.org/t/calling-a-rust-function-from-cranelift/103948/5.
    let mut sig = Signature::new(CallConv::triple_default(&HOST));
    let ptr = Type::triple_pointer_type(&HOST);

    sig.params.push(AbiParam::new(ptr)); // *const Thread<A>
    sig.params.push(AbiParam::new(ptr)); // *const *mut UpVal<A>

    // Setup builder.
    let mut ctx = g.jit.borrow_mut();
    let mut fun = Function::with_name_signature(Default::default(), sig);
    let mut fb = FunctionBuilder::new(&mut fun, &mut ctx);

    // Compile.
    let mut jitted = Vec::new();
    let p = (*lf).p.get();
    let code = unsafe { core::slice::from_raw_parts((*p).code, (*p).sizecode as usize) };
    let mut pc = 0;
    let entry = fb.create_block();

    fb.append_block_params_for_function_params(entry);
    fb.switch_to_block(entry);
    fb.seal_block(entry);

    loop {
        let i = match code.get(pc).copied() {
            Some(v) => v,
            None => break,
        };

        pc += 1;

        match i & 0x7F {
            _ => jitted.push(Jitted::Inst(i)),
        }
    }

    jitted
}

static HOST: Triple = Triple::host();
