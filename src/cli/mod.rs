use crate::core::{
    config::{generate_completion, Config},
    error::FResult,
    transform::{Context, ExitCodes},
};

use log::LevelFilter;
use simple_logger::SimpleLogger;

fn verbose_to_level_filter(v: u8) -> LevelFilter {
    match v {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

pub fn init(cfg: &Config) -> FResult<ExitCodes> {
    SimpleLogger::new()
        .with_level(verbose_to_level_filter(cfg.verbose))
        .init()
        .expect("Failed initializing logger");

    if let Some(shell) = cfg.completions {
        generate_completion(shell);
        std::process::exit(0);
    }

    let mut ctx = Context::from_cfg(cfg)?;
    Ok(ctx
        .apply(cfg.input()?.as_mut(), cfg.output()?.as_mut())?
        .exit_code)
}
