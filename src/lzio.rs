use core::ffi::c_char;

pub type ZIO = Zio;

#[repr(C)]
pub struct Zio {
    pub n: usize,
    pub p: *const c_char,
}
