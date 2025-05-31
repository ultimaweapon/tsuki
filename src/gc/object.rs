use std::cell::Cell;

/// Header of all object managed by Garbage Collector.
///
/// All object must have this struct at the beginning of its memory block.
pub(crate) struct Object {
    pub next: Cell<*mut Object>,
    pub tt: u8,
    pub marked: u8,
    pub refs: usize,
    pub handle: usize,
}
