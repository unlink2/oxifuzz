#[cfg(not(any(feature = "cli")))]
fn main() {}

#[cfg(feature = "cli")]
fn main() {
    use oxifuzz::{core::config::generate_completion, core::config::CFG};
    if let Some(shell) = CFG.completions {
        generate_completion(shell);
        std::process::exit(0);
    }
}

#[cfg(test)]
mod test {}
