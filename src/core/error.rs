use std::process::ExitStatusError;

use openssl::error::ErrorStack;
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
    #[error("JWT Signature error")]
    JwtSignatureError,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ExitStatus(#[from] ExitStatusError),
    #[error(transparent)]
    IsahcHttp(#[from] isahc::http::Error),
    #[error(transparent)]
    Isahc(#[from] isahc::Error),
    #[error(transparent)]
    OpenSsl(#[from] ErrorStack),
    #[error(transparent)]
    Hmac(#[from] hmac::digest::InvalidLength),
}
