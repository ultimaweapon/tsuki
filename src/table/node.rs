use crate::UnsafeValue;
use crate::value::UntaggedValue;
use core::ffi::c_int;

#[repr(C)]
pub(crate) union Node<D> {
    pub u: NodeKey<D>,
    pub i_val: UnsafeValue<D>,
}

impl<D> Clone for Node<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Node<D> {}

#[repr(C)]
pub(crate) struct NodeKey<D> {
    pub tt_: u8,
    pub key_tt: u8,
    pub value_: UntaggedValue<D>,
    pub next: c_int,
    pub key_val: UntaggedValue<D>,
}

impl<D> Clone for NodeKey<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for NodeKey<D> {}
