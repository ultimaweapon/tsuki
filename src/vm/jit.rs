use crate::Thread;
use crate::lstate::CallInfo;
use alloc::boxed::Box;

pub async unsafe fn run<A>(
    td: &Thread<A>,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    todo!()
}
