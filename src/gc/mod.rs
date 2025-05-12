use std::ffi::c_int;

/// Command to control the garbage collector.
pub enum GcCommand {
    Stop,
    Restart,
    Collect,
    Count,
    CountByte,
    Step(c_int),
    SetPause(c_int),
    SetStepMul(c_int),
    GetRunning,
    SetGen(c_int, c_int),
    SetInc(c_int, c_int, c_int),
}
