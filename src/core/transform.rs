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
pub type CommandRunnerFn = fn(
    ctx: &Context,
    output: &mut dyn std::io::Write,
    runner: &CommandRunnerKind,
    data: &Word,
) -> FResult<(Option<i32>, Word)>;

#[derive(Clone)]
pub enum CommandRunnerKind {
    Shell { cmd: String, cmd_args: Vec<String> },
    Output,
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
                on_run: shell_command_runner,
            }))
        } else {
            error!("Command shell runner configured without a command!");
            Err(Error::InsufficientRunnerConfiguration)
        }
    }

    pub fn output_runner(_cfg: &Config) -> FResult<Option<Self>> {
        Ok(Some(Self {
            kind: CommandRunnerKind::Output,
            on_run: output_command_runner,
        }))
    }

    pub fn from_cfg(cfg: &Config) -> FResult<Option<Self>> {
        match cfg.runner {
            super::config::RunnerKindConfig::Shell => Self::shell_runner(cfg),
            super::config::RunnerKindConfig::None => Ok(None),
            super::config::RunnerKindConfig::Output => Self::output_runner(cfg),
        }
    }

    pub fn run(
        &self,
        ctx: &Context,
        output: &mut dyn std::io::Write,
        data: &Word,
    ) -> FResult<(Option<i32>, Word)> {
        (self.on_run)(ctx, output, &self.kind, data)
    }
}

pub fn output_command_runner(
    _ctx: &Context,
    _output: &mut dyn std::io::Write,
    runner: &CommandRunnerKind,
    data: &Word,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Output = runner {
        Ok((None, data.to_owned()))
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

pub fn shell_command_runner(
    ctx: &Context,
    _output: &mut dyn std::io::Write,
    runner: &CommandRunnerKind,
    data: &Word,
) -> FResult<(Option<i32>, Word)> {
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

        Ok((exit_code.code(), output.trim_end().into()))
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub enum ExitCodes {
    #[default]
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

#[derive(Clone, Default)]
pub struct ApplyRes {
    pub exit_code: ExitCodes,
    pub overall_res: Word,
}

pub struct Context {
    words: Vec<Word>,
    target: Target,
    cmd_arg_target: String,
    rand: Rand,

    raw: bool,

    expect: Option<String>,
    expect_len: Option<usize>,
    expect_exit_code: Option<i32>,

    n_run: u32,
    no_stdin: bool,

    runner: Option<CommandRunner>,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Self::from_cfg_with_runner(cfg, CommandRunner::from_cfg(cfg)?)
    }

    pub fn from_cfg_with_runner(cfg: &Config, runner: Option<CommandRunner>) -> FResult<Self> {
        Ok(Self {
            cmd_arg_target: cfg.exec_target.to_owned(),

            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),
            rand: cfg.rand(),

            raw: cfg.raw,

            expect: cfg.expect.to_owned(),
            expect_len: cfg.expect_len,
            n_run: cfg.n_run,
            no_stdin: cfg.no_stdin,
            expect_exit_code: cfg.expect_exit_code,

            runner: runner,
        })
    }

    fn select_word(&mut self) -> &Word {
        let index = self.rand.next_range(0, self.words.len() as u64);

        &self.words[index as usize]
    }

    fn read_all(&self, input: &mut dyn std::io::Read) -> FResult<Vec<u8>> {
        let mut buf = Vec::new();
        input.read_to_end(&mut buf)?;
        Ok(buf)
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

    fn maybe_exec(&mut self, output: &mut dyn std::io::Write, data: &Word) -> FResult<ApplyRes> {
        if let Some(runner) = &self.runner {
            let (exit_code, stdout_output) = runner.run(self, output, data)?;
            let str_output = String::from_utf8_lossy(&stdout_output);

            if self.expect.is_none() && self.expect_len.is_none() && self.expect_exit_code.is_none()
            {
                if !self.raw {
                    writeln!(output, "{}", style(str_output).white())?;
                } else {
                    output.write(&stdout_output)?;
                }
                Ok(ApplyRes {
                    exit_code: ExitCodes::Success,
                    overall_res: stdout_output,
                })
            } else if self.maybe_compare_expected(&stdout_output)
                || self.maybe_compare_expected_len(&stdout_output)
                || (exit_code == self.expect_exit_code && self.expect_exit_code.is_some())
            {
                if !self.raw {
                    writeln!(
                        output,
                        "{} {}",
                        style("+").green(),
                        style(str_output).green()
                    )?;
                } else {
                    output.write(&stdout_output)?;
                }
                Ok(ApplyRes {
                    exit_code: ExitCodes::Success,
                    overall_res: stdout_output,
                })
            } else {
                if !self.raw {
                    writeln!(output, "{} {}", style("-").red(), style(str_output).red())?;
                }
                Ok(ApplyRes {
                    exit_code: ExitCodes::Failure,
                    overall_res: stdout_output,
                })
            }
        } else {
            Ok(ApplyRes {
                exit_code: ExitCodes::Success,
                overall_res: data.to_owned(),
            })
        }
    }

    fn apply_next(&mut self, input: &[u8], result: &mut Word) -> FResult<usize> {
        if input.is_empty() {
            Ok(0)
        } else if self.target.should_replace(input) {
            // FIXME do not clone word...
            let word = &self.select_word().to_owned();
            result.extend_from_slice(&word);
            Ok(self.target.len())
        } else {
            let d = &input[0..1];
            result.extend_from_slice(d);
            Ok(d.len())
        }
    }

    pub fn apply(
        &mut self,
        input: &mut dyn std::io::Read,
        output: &mut dyn std::io::Write,
    ) -> FResult<ApplyRes> {
        let input = self.read_all(input)?;

        debug!("Input: {:?}", input);
        let mut exit_code = ExitCodes::Success;
        let mut overall_res = Vec::new();

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

            let mut exec_res = self.maybe_exec(output, &result)?;
            if exec_res.exit_code.is_failure() {
                exit_code = exec_res.exit_code;
            }

            overall_res.append(&mut exec_res.overall_res);
        }

        Ok(ApplyRes {
            exit_code,
            overall_res,
        })
    }
}
