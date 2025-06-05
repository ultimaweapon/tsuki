use std::ffi::c_int;

/// Contains information for Lua chunk.
#[derive(Default, Clone)]
pub struct ChunkInfo {
    pub name: String,
}

/// Represents an error when failed to parse Lua source.
#[derive(Debug)]
pub enum ParseError {
    ItemLimit(&'static str, c_int),
}
