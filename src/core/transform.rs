use super::{
    config::Config,
    error::{Error, FResult},
    rand::Rand,
    runner::CommandRunner,
};
use console::style;
use log::debug;

pub type Word = Vec<u8>;

pub const DEFAULT_TARGET_WORD: &str = "OXIFUZZ";
pub const DEFAULT_USER_AGENT: &str = "oxifuzz/0.1";

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
    rand: Rand,
    input: Vec<u8>,
}

impl ContextIter {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Ok(ContextIter {
            n_run: cfg.n_run,
            count: 0,
            rand: cfg.rand(),
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
            Some(self.ctx.apply(&self.input, &mut self.rand))
        } else {
            None
        }
    }
}

#[derive(Clone, Default)]
pub struct Context {
    words: Vec<Word>,
    target: Target,

    pub expect: Vec<Expect>,

    pub runner: Option<CommandRunner>,

    pub dry_run: bool,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Self::from_cfg_with_runner(cfg, CommandRunner::from_cfg(cfg)?)
    }

    pub fn from_cfg_with_runner(cfg: &Config, runner: Option<CommandRunner>) -> FResult<Self> {
        Ok(Self {
            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),

            expect: Expect::from_cfg(cfg)?,

            runner,
            dry_run: cfg.dry_run,
        })
    }

    pub fn select_word(&self, rand: &mut Rand) -> FResult<&Word> {
        let index = rand.next_range(0, self.words.len() as u64)?;

        Ok(&self.words[(index as usize).min(self.words.len() - 1)])
    }

    pub fn read_all(input: &mut dyn std::io::Read) -> FResult<Vec<u8>> {
        let mut buf = Vec::new();
        input.read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub fn compare_expected(&self, data: &Word, exit_code: Option<i32>) -> bool {
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

    fn maybe_exec(&self, data: &Word, rand: &mut Rand) -> FResult<ExecRes> {
        if let Some(runner) = &self.runner {
            runner.run_and_expect(self, data, rand)
        } else {
            Ok(ExecRes {
                exit_code: ExitCodes::Success,
                out: data.to_owned(),
                fmt: OutputFmt::None,
            })
        }
    }

    fn apply_next(&self, input: &[u8], result: &mut Word, rand: &mut Rand) -> FResult<usize> {
        if input.is_empty() {
            Ok(0)
        } else if self.target.should_replace(input) {
            let word = &self.select_word(rand)?;
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
    pub fn apply(&self, input: &[u8], rand: &mut Rand) -> FResult<ExecRes> {
        debug!("Input: {:?}", input);

        let mut data = &input[0..];
        let mut result = Vec::new();
        while !data.is_empty() {
            let read = self.apply_next(data, &mut result, rand)?;
            if read == 0 {
                break;
            }

            data = &data[read..];
        }

        let exec_res = self.maybe_exec(&result, rand)?;

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
        runner::{default_command_expect, output_command_runner},
        transform::{ContextIter, Expect},
    };

    use super::{Context, ExecRes};

    fn assert_apply(input: &str, expected: Vec<ExecRes>, n_run: u32, expect: Option<Expect>) {
        let mut ctx = ContextIter {
            input: input.bytes().collect(),
            count: 0,
            n_run,
            rand: Rand::from_seed(1),
            ctx: Context {
                words: vec![b"123".to_vec(), b"45".to_vec(), b"abc".to_vec()],
                target: Default::default(),
                expect: if let Some(expect) = expect {
                    vec![expect]
                } else {
                    vec![]
                },
                runner: Some(super::CommandRunner {
                    kind: crate::core::runner::CommandRunnerKind::Output,
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
            Some(Expect::Equals("{12: abc}".into())),
        );
    }
}
