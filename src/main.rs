#[cfg(not(any(feature = "cli")))]
fn main() {}

#[cfg(feature = "cli")]
fn main() -> oxifuzz::prelude::FResult<()> {
    use oxifuzz::prelude::CFG;
    oxifuzz::cli::init(&CFG)
}

#[cfg(test)]
mod test {}
