pub use libc::*;

#[cfg(windows)]
unsafe extern "C-unwind" {
    #[link_name = "tsuki_snprintf"]
    fn snprintf(buffer: *mut c_char, count: usize, format: *const c_char, ...) -> c_int;
}
