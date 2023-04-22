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

#[derive(Clone)]
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
    fn(ctx: &Context, runner: &CommandRunnerKind, data: &Word) -> FResult<(Option<i32>, Word)>;

pub type CommandExpectFn =
    fn(ctx: &Context, exit_code: Option<i32>, data: &Word) -> FResult<ExecRes>;

#[derive(Clone)]
pub enum CommandRunnerKind {
    Shell {
        cmd: String,
        cmd_args: Vec<String>,
        cmd_arg_target: String,
    },
    Output,
    None,
}

#[derive(Clone)]
pub struct CommandRunner {
    kind: CommandRunnerKind,
    on_run: CommandRunnerFn,
    on_expect: CommandExpectFn,
}

impl CommandRunner {
    pub fn shell_runner(cfg: &Config) -> FResult<Option<Self>> {
        if let Some(cmd) = cfg.cmd()? {
            Ok(Some(Self {
                kind: CommandRunnerKind::Shell {
                    cmd,
                    cmd_args: cfg.cmd_args()?.unwrap_or(vec![]),
                    cmd_arg_target: cfg.exec_target.to_owned(),
                },
                on_run: shell_command_runner,
                on_expect: default_command_expect,
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
            on_expect: default_command_expect,
        }))
    }

    pub fn from_cfg(cfg: &Config) -> FResult<Option<Self>> {
        match cfg.runner {
            super::config::RunnerKindConfig::Shell => Self::shell_runner(cfg),
            super::config::RunnerKindConfig::None => Ok(None),
            super::config::RunnerKindConfig::Output => Self::output_runner(cfg),
        }
    }

    pub fn run(&self, ctx: &Context, data: &Word) -> FResult<(Option<i32>, Word)> {
        (self.on_run)(ctx, &self.kind, data)
    }

    pub fn expect(&self, ctx: &Context, exit_code: Option<i32>, data: &Word) -> FResult<ExecRes> {
        (self.on_expect)(ctx, exit_code, data)
    }

    pub fn run_and_expect(&self, ctx: &Context, data: &Word) -> FResult<ExecRes> {
        let (exit_code, data) = self.run(ctx, data)?;
        self.expect(ctx, exit_code, &data)
    }
}

pub fn output_command_runner(
    _ctx: &Context,
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
    runner: &CommandRunnerKind,
    data: &Word,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Shell {
        cmd,
        cmd_args,
        cmd_arg_target,
    } = runner
    {
        let args: Vec<String> = cmd_args
            .iter()
            .map(|x| x.replace(cmd_arg_target, &String::from_utf8_lossy(data)))
            .collect();

        if ctx.dry_run {
            let mut output = Vec::new();
            output.write_all(cmd.as_bytes())?;
            for arg in args {
                output.write_all(b" ")?;
                output.write_all(arg.as_bytes())?;
            }
            Ok((None, output))
        } else {
            let args: Vec<&str> = args.iter().map(|x| x.as_ref()).collect();

            let mut child = Command::new(cmd)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            if !ctx.no_stdin {
                let mut child_in = BufWriter::new(child.stdin.as_mut().unwrap());
                child_in.write_all(data)?;
            }
            let exit_code = child.wait()?;
            let mut child_out = BufReader::new(child.stdout.as_mut().unwrap());
            let output = std::io::read_to_string(&mut child_out)?;

            Ok((exit_code.code(), output.trim_end().into()))
        }
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

pub fn default_command_expect(
    ctx: &Context,
    exit_code: Option<i32>,
    data: &Word,
) -> FResult<ExecRes> {
    let success_code = if exit_code == Some(0) || exit_code.is_none() {
        ExitCodes::Success
    } else {
        ExitCodes::RunnerFailed
    };

    if ctx.expect.is_empty() {
        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
            fmt: OutputFmt::None,
        })
    } else if ctx.compare_expected(data, exit_code) {
        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
            fmt: OutputFmt::Expected,
        })
    } else {
        Ok(ExecRes {
            exit_code: ExitCodes::Failure,
            out: data.to_owned(),
            fmt: OutputFmt::NotExpected,
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub enum ExitCodes {
    #[default]
    Success,
    Failure,
    RunnerFailed,
    Unknown,
}

impl ExitCodes {
    pub fn is_failure(&self) -> bool {
        *self != ExitCodes::Success
    }
}

impl From<ExitCodes> for i32 {
    fn from(value: ExitCodes) -> Self {
        match value {
            ExitCodes::Success => 0,
            ExitCodes::Failure => 1,
            ExitCodes::RunnerFailed => 2,
            ExitCodes::Unknown => -1,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct ExecRes {
    pub exit_code: ExitCodes,
    pub out: Word,
    pub fmt: OutputFmt,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum OutputFmt {
    #[default]
    None,
    Expected,
    NotExpected,
}

#[derive(Clone, Default)]
pub struct ContextIter {
    count: u32,
    n_run: u32,
    pub ctx: Context,
    input: Vec<u8>,
}

impl ContextIter {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Ok(ContextIter {
            n_run: cfg.n_run,
            count: 0,
            ctx: Context::from_cfg(cfg)?,
            input: Context::read_all(&mut cfg.input()?)?,
        })
    }
}

impl std::iter::Iterator for ContextIter {
    type Item = FResult<ExecRes>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count < self.n_run {
            self.count += 1;
            Some(self.ctx.apply(&self.input))
        } else {
            None
        }
    }
}

#[derive(Clone, Default)]
pub struct Context {
    words: Vec<Word>,
    target: Target,
    rand: Rand,

    expect: Vec<Expect>,

    no_stdin: bool,

    runner: Option<CommandRunner>,

    dry_run: bool,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Self::from_cfg_with_runner(cfg, CommandRunner::from_cfg(cfg)?)
    }

    pub fn from_cfg_with_runner(cfg: &Config, runner: Option<CommandRunner>) -> FResult<Self> {
        Ok(Self {
            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),
            rand: cfg.rand(),

            expect: Expect::from_cfg(cfg)?,

            no_stdin: cfg.no_stdin,

            runner,
            dry_run: cfg.dry_run,
        })
    }

    fn select_word(&mut self) -> FResult<&Word> {
        let index = self.rand.next_range(0, self.words.len() as u64)?;

        Ok(&self.words[(index as usize).min(self.words.len() - 1)])
    }

    pub fn read_all(input: &mut dyn std::io::Read) -> FResult<Vec<u8>> {
        let mut buf = Vec::new();
        input.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn compare_expected(&self, data: &Word, exit_code: Option<i32>) -> bool {
        for e in self.expect.iter() {
            if e.expect(data, exit_code) {
                return true;
            }
        }
        false
    }

    /// helper for formatted output to any stream
    pub fn output(
        cfg: &Config,
        output: &mut dyn std::io::Write,
        data: &Word,
        fmt: &OutputFmt,
    ) -> FResult<()> {
        let str_output = String::from_utf8_lossy(data);
        if cfg.raw {
            match fmt {
                OutputFmt::NotExpected => {}
                _ => output.write_all(data)?,
            }
        } else {
            // if colors are enabled then override the value according to cfg
            if console::colors_enabled() {
                console::set_colors_enabled(!cfg.no_color);
            }
            match fmt {
                OutputFmt::None => writeln!(output, "{}", style(str_output).white())?,
                OutputFmt::Expected => writeln!(
                    output,
                    "{} {}",
                    style("+").green(),
                    style(str_output).green()
                )?,
                OutputFmt::NotExpected => {
                    writeln!(output, "{} {}", style("-").red(), style(str_output).red())?
                }
            }
        }
        Ok(())
    }

    fn maybe_exec(&mut self, data: &Word) -> FResult<ExecRes> {
        if let Some(runner) = &self.runner {
            runner.run_and_expect(self, data)
        } else {
            Ok(ExecRes {
                exit_code: ExitCodes::Success,
                out: data.to_owned(),
                fmt: OutputFmt::None,
            })
        }
    }

    fn apply_next(&mut self, input: &[u8], result: &mut Word) -> FResult<usize> {
        if input.is_empty() {
            Ok(0)
        } else if self.target.should_replace(input) {
            let word = &self.select_word()?;
            result.extend_from_slice(word);
            Ok(self.target.len())
        } else {
            let d = &input[0..1];
            result.extend_from_slice(d);
            Ok(d.len())
        }
    }

    /// This function converts the input data
    /// into an output which is collected into a single Word
    /// (this can be disabled in Context's settings)
    /// It will also streams results into output if it is provided
    pub fn apply(&mut self, input: &[u8]) -> FResult<ExecRes> {
        debug!("Input: {:?}", input);

        let mut data = &input[0..];
        let mut result = Vec::new();
        while !data.is_empty() {
            let read = self.apply_next(data, &mut result)?;
            if read == 0 {
                break;
            }

            data = &data[read..];
        }

        let exec_res = self.maybe_exec(&result)?;

        debug!("Res: {:?}", exec_res);
        Ok(exec_res)
    }
}

#[derive(Clone)]
pub enum Expect {
    Contains(Word),
    Regex(regex::Regex),
    Equals(Word),
    ExitCode(Option<i32>),
    Len(usize),
}

impl Expect {
    pub fn from_cfg(cfg: &Config) -> FResult<Vec<Self>> {
        let mut expects = Vec::default();

        for expect in cfg.expect.iter() {
            expects.push(Self::Equals(expect.to_owned()));
        }
        for len in cfg.expect_len.iter() {
            expects.push(Self::Len(*len));
        }
        for ex in cfg.expect_exit_code.iter() {
            expects.push(Self::ExitCode(Some(*ex)));
        }
        for re in cfg.expect_regex.iter() {
            expects.push(Self::Regex(
                regex::Regex::new(re).map_err(|_| Error::InvalidRegex)?,
            ));
        }
        for contains in cfg.contains.iter() {
            expects.push(Self::Contains(contains.to_owned()));
        }
        Ok(expects)
    }

    pub fn expect(&self, data: &Word, exit_code: Option<i32>) -> bool {
        match self {
            Expect::Contains(contains) => {
                for window in data.windows(contains.len()) {
                    if contains == window {
                        return true;
                    }
                }
                false
            }
            Expect::Regex(re) => {
                let utf8 = String::from_utf8_lossy(data);
                re.is_match(&utf8)
            }
            Expect::Equals(expected) => expected == data,
            Expect::ExitCode(expected) => &exit_code == expected,
            Expect::Len(len) => data.len() == *len,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::core::{
        rand::Rand,
        transform::{default_command_expect, ContextIter, Expect},
    };

    use super::{output_command_runner, Context, ExecRes, Word};

    fn assert_apply(input: &str, expected: Vec<ExecRes>, n_run: u32, expect: Option<Word>) {
        let mut ctx = ContextIter {
            input: input.bytes().collect(),
            count: 0,
            n_run,
            ctx: Context {
                words: vec![b"123".to_vec(), b"45".to_vec(), b"abc".to_vec()],
                target: Default::default(),
                rand: Rand::from_seed(1),
                expect: if let Some(expect) = expect {
                    vec![Expect::Equals(expect.to_owned())]
                } else {
                    vec![]
                },
                no_stdin: false,
                runner: Some(super::CommandRunner {
                    kind: super::CommandRunnerKind::Output,
                    on_run: output_command_runner,
                    on_expect: default_command_expect,
                }),
                dry_run: false,
            },
        };
        let res: Vec<ExecRes> = ctx.try_collect().unwrap();

        assert_eq!(expected, res);
    }

    #[test]
    fn success() {
        assert_apply(
            "{12: OXIFUZZ}",
            vec![ExecRes {
                exit_code: super::ExitCodes::Success,
                out: b"{12: abc}".to_vec(),
                fmt: super::OutputFmt::None,
            }],
            1,
            None,
        );

        assert_apply(
            "{12: OXIFUZZ}",
            vec![
                ExecRes {
                    exit_code: super::ExitCodes::Success,
                    out: b"{12: abc}".to_vec(),
                    fmt: super::OutputFmt::Expected,
                },
                ExecRes {
                    exit_code: super::ExitCodes::Failure,
                    out: b"{12: 45}".to_vec(),
                    fmt: super::OutputFmt::NotExpected,
                },
            ],
            2,
            Some("{12: abc}".into()),
        );
    }
}
