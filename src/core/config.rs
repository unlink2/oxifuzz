use std::{
    io::{BufReader, LineWriter, Read, Write},
    path::PathBuf,
};

#[cfg(feature = "cli")]
use clap::{CommandFactory, Parser};
#[cfg(feature = "cli")]
use clap_complete::{generate, Generator, Shell};
use lazy_static::lazy_static;

use super::error::FResult;

lazy_static! {
    pub static ref CFG: Config = Config::new();
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "cli", derive(Parser))]
#[cfg_attr(feature = "cli", command(author, version, about, long_about = None))]
pub struct Config {
    pub input: Option<PathBuf>,

    pub output: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long, short, default_value = crate::core::transform::DEFAULT_TARGET_WORD))]
    pub target: String,

    #[cfg_attr(feature = "cli", clap(long, short))]
    pub word_lists: Vec<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub word: Vec<String>,

    #[cfg_attr(feature = "cli", clap(long, short, default_value_t = 1))]
    pub n_run: u32,

    #[cfg_attr(feature = "cli", clap(long, default_value = "\n"))]
    pub word_list_term: String,

    #[cfg_attr(feature = "cli", clap(long))]
    pub random_file: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub seed: Option<u64>,

    #[cfg_attr(feature = "cli", clap(long, default_value = "0"))]
    pub verbose: u8,

    #[cfg_attr(feature = "cli", clap(long, value_name = "SHELL"))]
    #[cfg(feature = "cli")]
    pub completions: Option<Shell>,
}

impl Config {
    #[cfg(feature = "cli")]
    pub fn new() -> Self {
        Self::parse()
    }

    #[cfg(not(feature = "cli"))]
    pub fn new() -> Self {
        Default::default()
    }

    pub fn input(&self) -> FResult<Box<dyn Read>> {
        Ok(if let Some(path) = &self.input {
            Box::new(BufReader::new(std::fs::File::open(path)?))
        } else {
            Box::new(BufReader::new(std::io::stdin()))
        })
    }

    pub fn output(&self) -> FResult<Box<dyn Write>> {
        Ok(if let Some(path) = &self.output {
            Box::new(LineWriter::new(std::fs::File::create(path)?))
        } else {
            Box::new(LineWriter::new(std::io::stdout().lock()))
        })
    }
}

#[cfg(feature = "cli")]
pub fn generate_completion<G: Generator>(gen: G) {
    generate(
        gen,
        &mut Config::command(),
        Config::command().get_name(),
        &mut std::io::stdout(),
    );
}
