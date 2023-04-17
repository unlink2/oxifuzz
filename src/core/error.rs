use std::process::ExitStatusError;

use thiserror::Error;

pub type FResult<T> = Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown error")]
    Unknown,
    #[error("Unsupported command runner")]
    UnsupportedCommandRunner,
    #[error("Runner configuration is missing required options")]
    InsufficientRunnerConfiguration,
    #[error("Argument format error")]
    ArgError,
    #[error("Invalid regex")]
    InvalidRegex,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ExitStatus(#[from] ExitStatusError),
}
