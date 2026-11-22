#![no_std] // Prevent accidentally use unimplemented functions.
use alloc::format;
use alloc::string::String;
use erdp::ErrorDisplay;
use tsuki::Lua;
use tsuki::builtin::{BaseLib, CoroLib, MathLib, StrLib, TableLib, Utf8Lib};
use wasm_bindgen::prelude::*;

extern crate alloc;

#[wasm_bindgen]
pub fn run(code: &str) -> Result<String, JsError> {
    // Set up VM.
    let vm = Lua::with_seed((), 0);

    vm.use_module(None, true, BaseLib).unwrap();
    vm.use_module(None, true, CoroLib).unwrap();
    vm.use_module(None, true, MathLib).unwrap();
    vm.use_module(None, true, StrLib).unwrap();
    vm.use_module(None, true, TableLib).unwrap();
    vm.use_module(None, true, Utf8Lib).unwrap();

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
