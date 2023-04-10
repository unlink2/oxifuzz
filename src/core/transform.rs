use std::process::Command;

use super::{config::Config, error::FResult, rand::Rand};
use console::style;
use log::debug;

pub type Word = Vec<u8>;

pub const DEFAULT_TARGET_WORD: &str = "OXIFUZZ";

pub enum Target {
    Word(Word),
}

impl Default for Target {
    fn default() -> Self {
        Target::Word(DEFAULT_TARGET_WORD.bytes().collect())
    }
}

impl Target {
    pub fn should_replace(&self, input: &[u8]) -> bool {
        match self {
            Target::Word(word) => input.starts_with(word),
        }
    }

    fn len(&self) -> usize {
        match self {
            Target::Word(word) => word.len(),
        }
    }
}

pub struct Context {
    input: Box<dyn std::io::Read>,
    output: Box<dyn std::io::Write>,

    words: Vec<Word>,
    target: Target,
    cmd_arg_target: String,
    rand: Rand,

    cmd: Option<String>,
    cmd_args: Vec<String>,

    expect: Option<String>,
    expect_len: Option<usize>,

    n_run: u32,
    raw: bool,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Ok(Self {
            input: cfg.input()?,
            output: cfg.output()?,

            cmd_arg_target: cfg.exec_target.to_owned(),

            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),
            rand: cfg.rand(),
            cmd: cfg.cmd(),
            cmd_args: cfg.cmd_args().unwrap_or(vec![]),
            expect: cfg.expect.to_owned(),
            expect_len: cfg.expect_len,
            n_run: cfg.n_run,
            raw: cfg.raw,
        })
    }

    fn select_word(&mut self) -> &Word {
        let index = self.rand.next_range(0, self.words.len() as u64);

        &self.words[index as usize]
    }

    fn read_all(&mut self) -> FResult<Vec<u8>> {
        let mut buf = Vec::new();
        self.input.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn output(&mut self, input: &[u8], hit: bool) -> FResult<()> {
        if self.cmd.is_some() {
            return Ok(());
        }
        if hit && !self.raw {
            write!(
                self.output,
                "{}",
                style(String::from_utf8_lossy(input)).red()
            )?
        } else {
            self.output.write(input)?;
        }
        Ok(())
    }

    fn maybe_compare_expected(&self, cmd_output: &[u8]) -> bool {
        if let Some(expect) = &self.expect {
            cmd_output == expect.as_bytes()
        } else {
            false
        }
    }

    fn maybe_compare_expected_len(&self, cmd_output: &[u8]) -> bool {
        if let Some(len) = self.expect_len {
            len == cmd_output.len()
        } else {
            false
        }
    }

    fn did_compare_expected(&self) -> bool {
        self.expect_len.is_none() || self.expect.is_none()
    }

    fn maybe_exec(&mut self, data: &Word) -> FResult<()> {
        if let Some(cmd) = &self.cmd {
            let args: Vec<String> = self
                .cmd_args
                .iter()
                .map(|x| x.replace(&self.cmd_arg_target, &String::from_utf8_lossy(data)))
                .collect();

            let output = Command::new(cmd).args(&args).output()?;
            let output = String::from_utf8_lossy(&output.stdout);

            if self.maybe_compare_expected(output.as_bytes())
                || self.maybe_compare_expected_len(output.as_bytes())
            {
                write!(self.output, "{}", style(output).green())?;
            } else if !self.did_compare_expected() {
                write!(self.output, "{}", style(output).white())?;
            } else {
                write!(self.output, "{}", style(output).red())?;
            }
        }

        Ok(())
    }

    fn apply_next(&mut self, input: &[u8], result: &mut Word) -> FResult<usize> {
        if input.is_empty() {
            Ok(0)
        } else if self.target.should_replace(input) {
            // FIXME do not clone word...
            let word = &self.select_word().to_owned();
            self.output(&word, true)?;
            result.extend_from_slice(&word);
            Ok(self.target.len())
        } else {
            let d = &input[0..1];
            self.output(d, false)?;
            result.extend_from_slice(d);
            Ok(d.len())
        }
    }

    pub fn apply(&mut self) -> FResult<()> {
        let input = self.read_all()?;

        debug!("Input: {:?}", input);

        for _ in 0..self.n_run {
            let mut data = &input[0..];
            let mut result = Vec::new();
            while !data.is_empty() {
                let read = self.apply_next(data, &mut result)?;
                if read == 0 {
                    break;
                }

                data = &data[read..];
            }

            self.maybe_exec(&result)?;
        }

        Ok(())
    }
}
