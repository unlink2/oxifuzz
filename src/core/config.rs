use std::path::PathBuf;

#[cfg(feature = "cli")]
use clap::{CommandFactory, Parser};
#[cfg(feature = "cli")]
use clap_complete::{generate, Generator, Shell};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CFG: Config = Config::new();
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "cli", derive(Parser))]
#[cfg_attr(feature = "cli", command(author, version, about, long_about = None))]
pub struct Config {
    input: Option<PathBuf>,

    output: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long, short, default_value = "OXIFUZZ"))]
    replace_word: String,

    #[cfg_attr(feature = "cli", clap(long, short))]
    word_lists: Vec<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    word: Vec<String>,

    #[cfg_attr(feature = "cli", clap(long, short, default_value_t = 1))]
    n_run: u32,

    #[cfg_attr(feature = "cli", clap(long, default_value = "\n"))]
    word_list_term: String,

    #[cfg_attr(feature = "cli", clap(long))]
    random_file: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    seed: u32,

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
