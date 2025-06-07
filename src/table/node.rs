use crate::TValue;
use crate::lobject::UntaggedValue;
use std::ffi::c_int;

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union Node {
    pub u: NodeKey,
    pub i_val: TValue,
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
