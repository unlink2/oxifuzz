use crate::core::{
    config::{generate_completion, Config},
    error::FResult,
    transform::{Context, ContextIter, ExitCodes},
};

use log::{error, trace, LevelFilter};
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

    let mut output = cfg.output()?;
    let mut overall_exit_code = ExitCodes::Success;
    let mut ctx = ContextIter::from_cfg(cfg)?;
    let delay = std::time::Duration::from_millis(cfg.delay);

    // TODO a new ctx for each thread and run them all
    // and wait for all of them to finish
    for _ in 0..cfg.n_thread {
        ctx.try_for_each(|x| {
            if let Err(x) = &x {
                error!("{:?}", x);
                overall_exit_code = ExitCodes::RunnerFailed;
                if cfg.no_fail_on_err {
                    return Ok(());
                }
            }
            let x = x?;
            if x.exit_code.is_failure() {
                overall_exit_code = x.exit_code;
            }
            let res = Context::output(cfg, &mut output, &x.out, &x.fmt);
            trace!("Sleeping for {} ms", delay.as_millis());
            std::thread::sleep(delay);
            res
        })?;
    }
    Ok(overall_exit_code)
}
