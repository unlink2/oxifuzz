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

pub type CommandExpectFn = fn(
    output: &mut Option<&mut dyn std::io::Write>,
    ctx: &Context,
    exit_code: Option<i32>,
    data: &Word,
) -> FResult<ExecRes>;

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
        if let Some(cmd) = cfg.cmd() {
            Ok(Some(Self {
                kind: CommandRunnerKind::Shell {
                    cmd,
                    cmd_args: cfg.cmd_args().unwrap_or(vec![]),
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

    pub fn expect(
        &self,
        output: &mut Option<&mut dyn std::io::Write>,
        ctx: &Context,
        exit_code: Option<i32>,
        data: &Word,
    ) -> FResult<ExecRes> {
        (self.on_expect)(output, ctx, exit_code, data)
    }

    pub fn run_and_expect(
        &self,
        output: &mut Option<&mut dyn std::io::Write>,
        ctx: &Context,
        data: &Word,
    ) -> FResult<ExecRes> {
        let (exit_code, data) = self.run(ctx, data)?;
        self.expect(output, ctx, exit_code, &data)
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

pub fn default_command_expect(
    output: &mut Option<&mut dyn std::io::Write>,
    ctx: &Context,
    exit_code: Option<i32>,
    data: &Word,
) -> FResult<ExecRes> {
    let success_code = if exit_code == Some(0) || exit_code.is_none() {
        ExitCodes::Success
    } else {
        ExitCodes::RunnerFailed
    };

    if ctx.expect.is_none() && ctx.expect_len.is_none() && ctx.expect_exit_code.is_none() {
        ctx.output(output, data, &OutputFmt::None)?;
        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
        })
    } else if ctx.maybe_compare_expected(&data)
        || ctx.maybe_compare_expected_len(&data)
        || (exit_code == ctx.expect_exit_code && ctx.expect_exit_code.is_some())
    {
        ctx.output(output, data, &OutputFmt::Expected)?;

        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
        })
    } else {
        ctx.output(output, data, &OutputFmt::NotExpected)?;
        Ok(ExecRes {
            exit_code: ExitCodes::Failure,
            out: data.to_owned(),
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub enum ExitCodes {
    #[default]
    Success,
    Failure,
    RunnerFailed,
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
            ExitCodes::RunnerFailed => 2,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct ExecRes {
    pub exit_code: ExitCodes,
    pub out: Word,
}

pub enum OutputFmt {
    None,
    Expected,
    NotExpected,
}

#[derive(Clone, Default)]
pub struct Context {
    words: Vec<Word>,
    target: Target,
    rand: Rand,

    raw: bool,
    colors_enabled: bool,

    expect: Option<String>,
    expect_len: Option<usize>,
    expect_exit_code: Option<i32>,

    n_run: u32,
    no_stdin: bool,

    runner: Option<CommandRunner>,

    // when set to false do not collect result into one large output string
    collect_res: bool,
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

            raw: cfg.raw,
            colors_enabled: !cfg.no_color,

            expect: cfg.expect.to_owned(),
            expect_len: cfg.expect_len,
            n_run: cfg.n_run,
            no_stdin: cfg.no_stdin,
            expect_exit_code: cfg.expect_exit_code,

            runner,
            collect_res: false,
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

    fn output(
        &self,
        output: &mut Option<&mut dyn std::io::Write>,
        data: &Word,
        fmt: &OutputFmt,
    ) -> FResult<()> {
        if let Some(output) = output {
            let str_output = String::from_utf8_lossy(&data);
            if self.raw {
                match fmt {
                    OutputFmt::NotExpected => {}
                    _ => output.write_all(data)?,
                }
            } else {
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
        }
        Ok(())
    }

    fn maybe_exec(
        &mut self,
        output: &mut Option<&mut dyn std::io::Write>,
        data: &Word,
    ) -> FResult<ExecRes> {
        if let Some(runner) = &self.runner {
            runner.run_and_expect(output, self, data)
        } else {
            Ok(ExecRes {
                exit_code: ExitCodes::Success,
                out: data.to_owned(),
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

    /// This function converts the input data
    /// into an output which is collected into a single Word
    /// (this can be disabled in Context's settings)
    /// It will also streams results into output if it is provided
    pub fn apply(
        &mut self,
        input: &mut dyn std::io::Read,
        mut output: Option<&mut dyn std::io::Write>,
    ) -> FResult<(ExitCodes, Vec<ExecRes>)> {
        // if colors are enabled then override the value according to cfg
        if console::colors_enabled() {
            console::set_colors_enabled(self.colors_enabled);
        }

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

            let exec_res = self.maybe_exec(&mut output, &result)?;
            if exec_res.exit_code.is_failure() {
                exit_code = exec_res.exit_code;
            }

            if self.collect_res {
                overall_res.push(exec_res);
            }
        }

        debug!("Exit code: {:?}. Overall res: {:?}", exit_code, overall_res);

        Ok((exit_code, overall_res))
    }
}

#[cfg(test)]
mod test {
    use crate::core::{rand::Rand, transform::default_command_expect};

    use super::{output_command_runner, Context, ExecRes, ExitCodes, Word};

    fn assert_apply(
        input: &str,
        expected: (ExitCodes, Vec<ExecRes>),
        expected_output: Word,
        n_run: u32,
        expect: Option<String>,
    ) {
        let mut ctx = Context {
            words: vec![b"123".to_vec(), b"45".to_vec(), b"abc".to_vec()],
            target: Default::default(),
            rand: Rand::from_seed(1),
            raw: false,
            expect,
            expect_len: None,
            expect_exit_code: None,
            n_run,
            no_stdin: false,
            runner: Some(super::CommandRunner {
                kind: super::CommandRunnerKind::Output,
                on_run: output_command_runner,
                on_expect: default_command_expect,
            }),
            collect_res: true,
            colors_enabled: false,
        };
        let mut output = vec![];
        let res = ctx
            .apply(&mut input.as_bytes().to_vec().as_slice(), Some(&mut output))
            .unwrap();

        assert_eq!(expected, res);
        assert_eq!(expected_output, output);
    }

    #[test]
    fn success() {
        assert_apply(
            "{12: OXIFUZZ}",
            (
                ExitCodes::Success,
                vec![ExecRes {
                    exit_code: super::ExitCodes::Success,
                    out: b"{12: abc}".to_vec(),
                }],
            ),
            b"{12: abc}\n".to_vec(),
            1,
            None,
        );

        assert_apply(
            "{12: OXIFUZZ}",
            (
                ExitCodes::Failure,
                vec![
                    ExecRes {
                        exit_code: super::ExitCodes::Success,
                        out: b"{12: abc}".to_vec(),
                    },
                    ExecRes {
                        exit_code: super::ExitCodes::Failure,
                        out: b"{12: 45}".to_vec(),
                    },
                ],
            ),
            b"+ {12: abc}\n- {12: 45}\n".to_vec(),
            2,
            Some("{12: abc}".into()),
        );
    }
}
