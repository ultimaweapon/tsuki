use super::HOST;
use core::marker::PhantomData;
use cranelift_codegen::ir::{
    AbiParam, ExtFuncData, ExternalName, FuncRef, Signature, Type, UserExternalName,
    UserExternalNameRef,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use std::collections::HashMap;

/// Contains Rust functions that can be called from jitted function.
pub struct RustFuncs<A> {
    list: HashMap<UserExternalNameRef, *const u8>,
    phantom: PhantomData<A>,
}

impl<A> RustFuncs<A> {
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
        let name = UserExternalName::new(0, self.list.len().try_into().unwrap());
        let name = fb.func.declare_imported_user_function(name);

        assert!(self.list.insert(name, f).is_none());

        fb.func.import_function(ExtFuncData {
            name: ExternalName::User(name),
            signature: sig,
            colocated: false,
            patchable: false,
        })
    }

    /// # Panics
    /// If `name` does not exists.
    pub fn get(&self, name: UserExternalNameRef) -> *const u8 {
        self.list.get(&name).copied().unwrap()
    }
}

impl<A> Default for RustFuncs<A> {
    fn default() -> Self {
        Self {
            list: Default::default(),
            phantom: Default::default(),
        }
    }
}
