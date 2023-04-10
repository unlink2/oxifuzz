use std::{
    io::{BufReader, LineWriter, Read, Write},
    path::PathBuf,
};

#[cfg(feature = "cli")]
use clap::{CommandFactory, Parser};
#[cfg(feature = "cli")]
use clap_complete::{generate, Generator, Shell};
use lazy_static::lazy_static;
use log::debug;

use super::{error::FResult, rand::Rand, transform::Word};

lazy_static! {
    pub static ref CFG: Config = Config::new();
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "cli", derive(Parser))]
#[cfg_attr(feature = "cli", command(author, version, about, long_about = None))]
pub struct Config {
    pub input: Option<PathBuf>,

    pub output: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long, help = "Run command for each output"))]
    pub exec: Option<String>,

    #[cfg_attr(
        feature = "cli",
        clap(
            long,
            help = "Look for this specific output and notify the user when found"
        )
    )]
    pub expect: Option<String>,

    #[cfg_attr(feature = "cli", clap(long, help = "Expected command lenght"))]
    pub expect_len: Option<usize>,

    #[cfg_attr(feature = "cli", clap(long, help = "Replace target for command args",
        default_value =crate::core::transform::DEFAULT_TARGET_WORD))]
    pub exec_target: String,

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

    #[cfg_attr(feature = "cli", clap(long, default_value = "\n"))]
    pub word_list_term: String,

    #[cfg_attr(feature = "cli", clap(long))]
    pub random_file: Option<PathBuf>,

    #[cfg_attr(feature = "cli", clap(long))]
    pub seed: Option<u64>,

    #[cfg_attr(feature = "cli", arg(short, long, action = clap::ArgAction::Count))]
    pub verbose: u8,

    #[cfg_attr(feature = "cli", arg(long, help = "Output as bytes instead of chars"))]
    pub raw: bool,

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

    pub fn rand(&self) -> Rand {
        if let Some(seed) = self.seed {
            Rand::from_seed(seed)
        } else if let Some(path) = &self.random_file {
            Rand::File(path.to_owned())
        } else {
            Rand::default()
        }
    }

    pub fn words(&self) -> FResult<Vec<Word>> {
        let mut res: Vec<Word> = self.word.iter().map(|x| x.clone().into_bytes()).collect();

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
    pub fn cmd(&self) -> Option<String> {
        if let Some(exec) = &self.exec {
            let mut split = exec.split_whitespace();
            let command = split.next().unwrap_or("").to_owned();

            debug!("Command {:?}", command);

            Some(command)
        } else {
            None
        }
    }

    pub fn cmd_args(&self) -> Option<Vec<String>> {
        if let Some(exec) = &self.exec {
            let split = exec.split_whitespace().skip(1);
            let args = split.map(|x| x.to_owned()).collect();

            debug!("Args {:?}", args);

            Some(args)
        } else {
            None
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
