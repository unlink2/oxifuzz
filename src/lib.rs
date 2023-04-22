#![feature(exit_status_error)]
#![feature(iterator_try_collect)]

#[cfg(feature = "cli")]
pub mod cli;
pub mod core;
pub mod prelude;
