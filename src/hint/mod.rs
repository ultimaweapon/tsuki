//! Taken from
//! https://github.com/rust-lang/hashbrown/commit/64bd7db1d1b148594edfde112cdb6d6260e2cfc3.

#[inline(always)]
pub fn likely(b: bool) -> bool {
    if b {
        true
    } else {
        cold_path();
        false
    }
}

#[inline(always)]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
        true
    } else {
        false
    }
}

#[cold]
#[inline(always)]
pub fn cold_path() {}
