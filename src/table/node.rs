use crate::UnsafeValue;
use crate::value::UntaggedValue;
use core::ffi::c_int;

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union Node {
    pub u: NodeKey,
    pub i_val: UnsafeValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) struct NodeKey {
    pub value_: UntaggedValue,
    pub tt_: u8,
    pub key_tt: u8,
    pub next: c_int,
    pub key_val: UntaggedValue,
}
