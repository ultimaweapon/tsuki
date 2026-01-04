use crate::value::UntaggedValue;
use core::ffi::c_int;

#[repr(C)]
pub(crate) struct Node<A> {
    pub tt_: u8,
    pub key_tt: u8,
    pub next: c_int,
    pub value_: UntaggedValue<A>,
    pub key_val: UntaggedValue<A>,
}

impl<A> Clone for Node<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for Node<A> {}
