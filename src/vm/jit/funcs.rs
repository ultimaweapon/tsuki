use super::HOST;
use alloc::vec::Vec;
use cranelift_codegen::ir::{
    AbiParam, ExtFuncData, ExternalName, FuncRef, Signature, Type, UserExternalName,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;

/// Contains Rust functions that can be called from jitted function.
#[derive(Default)]
pub struct RustFuncs(Vec<*const u8>);

impl RustFuncs {
    /// # Safety
    /// `f` must use C calling convention and its signature must matched with `params` and `output`.
    pub unsafe fn import(
        &mut self,
        fb: &mut FunctionBuilder,
        params: &[Type],
        output: Option<Type>,
        f: *const u8,
    ) -> FuncRef {
        // Build signature.
        let mut sig = Signature::new(CallConv::triple_default(&HOST));

        for p in params {
            sig.params.push(AbiParam::new(*p));
        }

        if let Some(v) = output {
            sig.returns.push(AbiParam::new(v));
        }

        // Import function.
        let sig = fb.func.import_signature(sig);
        let name = UserExternalName::new(0, self.0.len().try_into().unwrap());
        let name = fb.func.declare_imported_user_function(name);

        self.0.push(f);

        fb.func.import_function(ExtFuncData {
            name: ExternalName::User(name),
            signature: sig,
            colocated: false,
        })
    }
}
