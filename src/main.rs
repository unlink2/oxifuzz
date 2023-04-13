#[cfg(not(any(feature = "cli")))]
fn main() {}

#[cfg(feature = "cli")]
fn main() -> oxifuzz::prelude::FResult<std::process::ExitCode> {
    use oxifuzz::prelude::CFG;
    let res = oxifuzz::cli::init(&CFG);
    if let Err(err) = &res {
        println!("{err}");
    }
    let exit_code: i32 = res?.into();
    Ok((exit_code as u8).into())
}

#[cfg(test)]
mod test {}
