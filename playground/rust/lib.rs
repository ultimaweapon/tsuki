#![no_std] // Prevent accidentally use unimplemented functions.
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use erdp::ErrorDisplay;
use tsuki::builtin::{BaseLib, CoroLib, MathLib, StrLib, TableLib, Utf8Lib};
use tsuki::context::{Args, Ret};
use tsuki::{Lua, fp};
use wasm_bindgen::prelude::*;

extern crate alloc;

#[wasm_bindgen]
pub fn run(cx: Context, code: &str) -> Result<String, JsError> {
    // Set up VM.
    let vm = Lua::with_seed(cx, 0);

    vm.use_module(None, true, BaseLib).unwrap();
    vm.use_module(None, true, CoroLib).unwrap();
    vm.use_module(None, true, MathLib).unwrap();
    vm.use_module(None, true, StrLib).unwrap();
    vm.use_module(None, true, TableLib).unwrap();
    vm.use_module(None, true, Utf8Lib).unwrap();

    vm.global().set_str_key("print", fp!(print));

    // Load.
    let chunk = match vm.load("playground.lua", code) {
        Ok(v) => v,
        Err(e) => return Ok(format!("Failed to load Lua code: {}.", e.display())),
    };

    // Run.
    let td = vm.create_thread();

    if let Err(e) = td.call::<()>(chunk, ()) {
        return Ok(format!("Failed to run Lua code: {}.", e.display()));
    }

    Ok(String::new())
}

fn print(
    cx: tsuki::context::Context<Context, Args>,
) -> Result<tsuki::context::Context<Context, Ret>, Box<dyn core::error::Error>> {
    // Fast path for zero or one argument.
    let mut args = match cx.args() {
        0 => {
            cx.associated_data().writeln(&[]);

            return Ok(cx.into());
        }
        1 => {
            let v = cx.arg(1).display()?;

            cx.associated_data().writeln(v.as_bytes());

            return Ok(cx.into());
        }
        n => alloc::vec::Vec::with_capacity(n),
    };

    // Get all args before printing to prevent interleave since it can call into arbitrary function.
    for i in 1..=cx.args() {
        args.push(cx.arg(i).display()?);
    }

    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            cx.associated_data().write(b"\t");
        }

        cx.associated_data().write(arg.as_bytes());
    }

    cx.associated_data().writeln(&[]);

    Ok(cx.into())
}

#[wasm_bindgen(module = "@/context")]
extern "C" {
    #[wasm_bindgen(typescript_type = "Context")]
    pub type Context;

    #[wasm_bindgen(method)]
    fn write(this: &Context, data: &[u8]);

    #[wasm_bindgen(method)]
    fn writeln(this: &Context, data: &[u8]);
}
