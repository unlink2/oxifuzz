use thiserror::Error;

pub type FResult<T> = Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown error")]
    Unknown,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
