use std::any::Any;
use std::rc::Rc;

/// Object managed by Garbage Collector.
pub(crate) struct Object {
    pub next: *mut Object,
    pub marked: u8,
    pub value: Rc<dyn Any>,
}
