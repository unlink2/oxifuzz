use std::{
    io::{BufReader, BufWriter, Write},
    process::{Command, Stdio},
};

use super::{
    config::Config,
    error::{Error, FResult},
    rand::Rand,
};
use console::style;
use log::{debug, error};

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

/// Function that runs a command and returns an exit code and the output of the command
pub type CommandRunnerFn =
    fn(ctx: &Context, runner: &CommandRunnerKind, data: &Word) -> FResult<(Option<i32>, String)>;

#[derive(Clone)]
pub enum CommandRunnerKind {
    Shell { cmd: String, cmd_args: Vec<String> },
    None,
}

#[derive(Clone)]
pub struct CommandRunner {
    kind: CommandRunnerKind,
    on_run: CommandRunnerFn,
}

impl CommandRunner {
    pub fn shell_runner(cfg: &Config) -> FResult<Option<Self>> {
        if let Some(cmd) = cfg.cmd() {
            Ok(Some(Self {
                kind: CommandRunnerKind::Shell {
                    cmd,
                    cmd_args: cfg.cmd_args().unwrap_or(vec![]),
                },
                on_run: default_command_runner,
            }))
        } else {
            error!("Command shell runner configured without a command!");
            Err(Error::InsufficientRunnerConfiguration)
        }
    }

    pub fn from_cfg(cfg: &Config) -> FResult<Option<Self>> {
        match cfg.runner {
            super::config::RunnerKindConfig::Shell => Self::shell_runner(cfg),
            super::config::RunnerKindConfig::None => Ok(None),
        }
    }

    pub fn run(&self, ctx: &Context, data: &Word) -> FResult<(Option<i32>, String)> {
        (self.on_run)(ctx, &self.kind, data)
    }
}

pub fn default_command_runner(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
) -> FResult<(Option<i32>, String)> {
    if let CommandRunnerKind::Shell { cmd, cmd_args } = runner {
        let args: Vec<String> = cmd_args
            .iter()
            .map(|x| x.replace(&ctx.cmd_arg_target, &String::from_utf8_lossy(data)))
            .collect();
        let args: Vec<&str> = args.iter().map(|x| x.as_ref()).collect();

        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if !ctx.no_stdin {
            let mut child_in = BufWriter::new(child.stdin.as_mut().unwrap());
            child_in.write_all(&data)?;
        }
        let exit_code = child.wait()?;
        let mut child_out = BufReader::new(child.stdout.as_mut().unwrap());
        let output = std::io::read_to_string(&mut child_out)?;

        Ok((exit_code.code(), output))
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ExitCodes {
    Success,
    Failure,
}

impl ExitCodes {
    pub fn is_failure(&self) -> bool {
        *self != ExitCodes::Success
    }
}

impl Into<i32> for ExitCodes {
    fn into(self) -> i32 {
        match self {
            ExitCodes::Success => 0,
            ExitCodes::Failure => 1,
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

    expect: Option<String>,
    expect_len: Option<usize>,
    expect_exit_code: Option<i32>,

    n_run: u32,
    raw: bool,
    no_stdin: bool,

    runner: Option<CommandRunner>,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Self::from_cfg_with_runner(cfg, CommandRunner::from_cfg(cfg)?)
    }

    pub fn from_cfg_with_runner(cfg: &Config, runner: Option<CommandRunner>) -> FResult<Self> {
        Ok(Self {
            input: cfg.input()?,
            output: cfg.output()?,

            cmd_arg_target: cfg.exec_target.to_owned(),

            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),
            rand: cfg.rand(),

            expect: cfg.expect.to_owned(),
            expect_len: cfg.expect_len,
            n_run: cfg.n_run,
            raw: cfg.raw,
            no_stdin: cfg.no_stdin,
            expect_exit_code: cfg.expect_exit_code,

            runner: runner,
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
        if self.runner.is_some() {
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

    fn maybe_exec(&mut self, data: &Word) -> FResult<ExitCodes> {
        if let Some(runner) = &self.runner {
            let (exit_code, output) = runner.run(self, data)?;
            let output = output.trim_end();

            if self.expect.is_none() && self.expect_len.is_none() && self.expect_exit_code.is_none()
            {
                writeln!(self.output, "{}", style(output).white())?;
                Ok(ExitCodes::Success)
            } else if self.maybe_compare_expected(output.as_bytes())
                || self.maybe_compare_expected_len(output.as_bytes())
                || (exit_code == self.expect_exit_code && self.expect_exit_code.is_some())
            {
                writeln!(
                    self.output,
                    "{} {}",
                    style("+").green(),
                    style(output).green()
                )?;
                Ok(ExitCodes::Success)
            } else {
                writeln!(self.output, "{} {}", style("-").red(), style(output).red())?;
                Ok(ExitCodes::Failure)
            }
        } else {
            Ok(ExitCodes::Success)
        }
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

    pub fn apply(&mut self) -> FResult<ExitCodes> {
        let input = self.read_all()?;

        debug!("Input: {:?}", input);
        let mut exit_code = ExitCodes::Success;

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

            let code = self.maybe_exec(&result)?;
            if code.is_failure() {
                exit_code = code;
            }
        }

        Ok(exit_code)
    }
}
