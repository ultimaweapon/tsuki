use crate::Context;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

pub fn error(cx: &Context) -> Result<(), Box<dyn core::error::Error>> {
    let msg = cx.get_str(0, true)?;

    if cx.len() > 1 {
        return Err("second argument of 'error' is not supported".into());
    }

    Err(String::from_utf8_lossy(msg.as_bytes()).into())
}

#[cfg(feature = "std")]
pub fn print(cx: &Context) -> Result<(), Box<dyn core::error::Error>> {
    use std::io::Write;

    // We can't print while converting the arguments to string since it can call into arbitrary
    // function, which may lock stdout.
    let mut args = Vec::with_capacity(cx.len());

    for i in 0..cx.len() {
        args.push(cx.to_str(i)?);
    }

    // Print.
    let mut stdout = std::io::stdout().lock();

    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            stdout.write_all(b"\t")?;
        }

        stdout.write_all(arg.as_bytes())?;
    }

    writeln!(stdout)?;

    Ok(())
}
