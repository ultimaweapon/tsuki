use super::object::Object;
use std::collections::VecDeque;

/// List of Lua objects optimized for CPU cache to be operate by GC.
pub(super) struct ObjectList {
    list: Vec<Object>,
    holes: VecDeque<usize>,
}
