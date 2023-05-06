use std::{
    io::{BufReader, LineWriter, Read, Write},
    path::PathBuf,
};

#[cfg(feature = "cli")]
use clap::{CommandFactory, Parser, ValueEnum};
#[cfg(feature = "cli")]
use clap_complete::{generate, Generator, Shell};
use lazy_static::lazy_static;
use log::debug;

use super::{error::FResult, rand::Rand, transform::Word};

lazy_static! {
    pub static ref CFG: Config = Config::new();
}

/// Runner kind without data attached
#[cfg_attr(feature = "cli", derive(ValueEnum))]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum RunnerKindConfig {
    Shell,
    Output,
    Http,
    Jwt,
    #[default]
    None,
}

#[cfg_attr(feature = "cli", derive(ValueEnum))]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum SignatureConfig {
    HmacSha256,
    Rs256,
    #[default]
    None,
}

// Http method
// TODO implement more methods in the future, use curl as --exec for now if needed
#[cfg_attr(feature = "cli", derive(ValueEnum))]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum HttpMethod {
    #[default]
    Get,
    Head,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "cli", derive(Parser))]
#[cfg_attr(feature = "cli", command(author, version, about, long_about = None))]
pub struct Config {
    pub input: Option<PathBuf>,

    pub output: Option<PathBuf>,

    #[arg(last = true)]
    pub escaped_words: Vec<Word>,

    #[cfg_attr(feature = "cli", clap(long, help = "Run command for each output"))]
    pub exec: Option<String>,

    #[cfg_attr(
        feature = "cli",
        clap(
            long,
            help = "Look for this specific output and notify the user when found"
        )
    )]
    pub expect: Vec<Word>,

    #[cfg_attr(
        feature = "cli",
        clap(long, help = "Check if output contains this sequence")
    )]
    pub contains: Vec<Word>,

    #[cfg_attr(feature = "cli", clap(long, help = "Apply a regex to the result"))]
    pub expect_regex: Vec<String>,

    #[cfg_attr(feature = "cli", clap(long, help = "Expected command lenght"))]
    pub expect_len: Vec<usize>,
    #[cfg_attr(feature = "cli", clap(long, help = "Expected exit code"))]
    pub expect_exit_code: Vec<i32>,

    #[cfg_attr(feature = "cli", clap(long, help = "Replace target for command args",
        default_value =crate::core::transform::DEFAULT_TARGET_WORD))]
    pub exec_target: String,

    #[cfg_attr(feature = "cli", clap(long))]
    pub url: Option<String>,

    #[cfg_attr(
        feature = "cli",
        clap(long, help = "Specify a http header and value (header:value)")
    )]
    pub header: Vec<String>,

    #[cfg_attr(
        feature = "cli",
        clap(long, help = "Do not include headers in http response")
    )]
    pub no_headers: bool,

    #[cfg_attr(feature = "cli", clap(long))]
    pub http_method: Option<HttpMethod>,

    #[cfg_attr(feature = "cli", clap(long, help = "Http request timeout in ms"))]
    pub http_timeout: Option<u32>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub jwt_secret: Option<Word>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub jwt_secret_file: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub jwt_header: Option<String>,

    #[cfg_attr(feature = "cli", clap(long, default_value = "none"))]
    pub jwt_signature: SignatureConfig,

    #[cfg_attr(feature = "cli", clap(long, short, 
        help="The target substring that will be replaced with words. 
        If the target string appears in the cli arguments it will be replaced with an entire iteration's output, otherwise the output will be passed in via stdin.", 
        default_value = crate::core::transform::DEFAULT_TARGET_WORD))]
    pub target: String,

    #[cfg_attr(feature = "cli", clap(long, short, help = "List of words"))]
    pub word_list: Vec<PathBuf>,

    #[cfg_attr(
        feature = "cli",
        clap(long, help = "Add content of an entire file as a word")
    )]
    pub word_file: Vec<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub word: Vec<String>,

    #[cfg_attr(feature = "cli", clap(long, short, default_value_t = 1))]
    pub n_run: u32,

    #[cfg_attr(feature = "cli", clap(long, default_value_t = 1))]
    pub n_thread: u32,

    #[cfg_attr(
        feature = "cli",
        clap(long, default_value_t = 0, help = "Delay between teach run in ms")
    )]
    pub delay: u64,

    #[cfg_attr(feature = "cli", clap(long, default_value = "\n"))]
    pub word_list_term: String,

    #[cfg_attr(feature = "cli", clap(long))]
    pub random_file: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub seed: Option<u64>,

    #[cfg_attr(feature = "cli", arg(short, long, action = clap::ArgAction::Count))]
    pub verbose: u8,

    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            help = "Output as bytes instead of chars. In raw mode only outputs that match any of the --expect options will be output."
        )
    )]
    pub raw: bool,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = false))]
    pub no_color: bool,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = false))]
    pub no_fail_on_err: bool,

    #[cfg_attr(
        feature = "cli",
        arg(long, help = "Disable writing output data to child command's stdin")
    )]
    pub no_stdin: bool,

    #[cfg_attr(feature = "cli", clap(long, short, value_enum, default_value_t = RunnerKindConfig::None))]
    pub runner: RunnerKindConfig,

    #[cfg_attr(feature = "cli", clap(long, help = "Disable runners"))]
    pub dry_run: bool,

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
            if path.to_str().unwrap_or("") == "-" {
                Box::new(BufReader::new(std::io::stdin()))
            } else {
                Box::new(BufReader::new(std::fs::File::open(path)?))
            }
        } else {
            Box::new(BufReader::new(std::io::stdin()))
        })
    }

    pub fn output(&self) -> FResult<Box<dyn Write>> {
        Ok(if let Some(path) = &self.output {
            if path.to_str().unwrap_or("") == "-" {
                Box::new(LineWriter::new(std::io::stdout()))
            } else {
                Box::new(LineWriter::new(std::fs::File::create(path)?))
            }
        } else {
            Box::new(LineWriter::new(std::io::stdout()))
        })
    }

    pub fn rand(&self) -> Rand {
        if let Some(seed) = self.seed {
            Rand::from_seed(seed)
        } else if let Some(path) = &self.random_file {
            Rand::from_path(path)
        } else {
            Rand::default()
        }
    }

    pub fn words(&self) -> FResult<Vec<Word>> {
        let mut res: Vec<Word> = self.word.iter().map(|x| x.clone().into_bytes()).collect();

        for word in &self.escaped_words {
            res.push(word.to_owned());
        }

        for path in &self.word_list {
            let all = std::fs::read_to_string(path)?;
            res.append(
                &mut all
                    .split(&self.word_list_term)
                    .map(|x| x.to_owned().into_bytes())
                    .collect(),
            );
        }

        for path in &self.word_file {
            let mut f = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;

            res.push(buffer);
        }

        debug!("Word list: {:?}", res);

        Ok(res)
    }

    // returns the command as well as args
    pub fn cmd(&self) -> FResult<Option<String>> {
        if let Some(exec) = &self.exec {
            let split = shell_words::split(exec).map_err(|_| crate::prelude::Error::ArgError)?;
            let command = split.first().unwrap_or(&"".into()).to_owned();

            debug!("Command {:?}", command);

            Ok(Some(command))
        } else {
            Ok(None)
        }
    }

    pub fn cmd_args(&self) -> FResult<Option<Vec<String>>> {
        if let Some(exec) = &self.exec {
            let mut split =
                shell_words::split(exec).map_err(|_| crate::prelude::Error::ArgError)?;
            if !split.is_empty() {
                split.remove(0);
            }
            let args = split;

            debug!("Args {:?}", args);

            Ok(Some(args))
        } else {
            Ok(None)
        }
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
