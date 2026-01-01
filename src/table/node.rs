use crate::value::UntaggedValue;
use core::ffi::c_int;

#[repr(C)]
pub(crate) struct Node<A> {
    pub tt_: u8,
    pub key_tt: u8,
    pub value_: UntaggedValue<A>,
    pub next: c_int,
    pub key_val: UntaggedValue<A>,
}

impl<D> Clone for Node<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Node<D> {}
