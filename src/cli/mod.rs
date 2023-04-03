use crate::core::{
    config::{generate_completion, Config},
    error::FResult,
};

pub fn init(cfg: &Config) -> FResult<()> {
    if let Some(shell) = cfg.completions {
        generate_completion(shell);
        std::process::exit(0);
    }
    Ok(())
}
