use super::object::Object;

/// List of Lua objects optimized for CPU cache to be operate by GC.
#[derive(Default)]
pub(super) struct ObjectList {
    list: Vec<Object>,
    holes: Vec<usize>,
}
