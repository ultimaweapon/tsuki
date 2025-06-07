pub type ZIO = Zio;

#[repr(C)]
pub struct Zio {
    pub n: usize,
    pub p: *const libc::c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mbuffer {
    pub buffer: *mut libc::c_char,
    pub n: usize,
    pub buffsize: usize,
}
