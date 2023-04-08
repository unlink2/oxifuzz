use log::debug;

use super::{config::Config, error::FResult, rand::Rand};

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
    rand: Rand,

    cmd: Option<String>,
    cmd_args: Vec<String>,

    expect: Option<String>,
    expect_len: Option<usize>,

    n_run: u32,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Ok(Self {
            input: cfg.input()?,
            output: cfg.output()?,

            words: cfg.words()?,
            target: Target::Word(cfg.target.to_owned().into_bytes()),
            rand: cfg.rand(),
            cmd: cfg.cmd(),
            cmd_args: cfg.cmd_args().unwrap_or(vec![]),
            expect: cfg.expect.to_owned(),
            expect_len: cfg.expect_len,
            n_run: cfg.n_run,
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

    fn output(&mut self, input: &[u8]) -> FResult<()> {
        self.output.write(input)?;
        Ok(())
    }

    fn apply_next(&mut self, input: &[u8]) -> FResult<usize> {
        if input.is_empty() {
            Ok(0)
        } else if self.target.should_replace(input) {
            // FIXME to not clone word...
            let word = &self.select_word().to_owned();
            self.output(&word)?;
            Ok(self.target.len())
        } else {
            self.output(&input[0..1])?;
            Ok(1)
        }
    }

    pub fn apply(&mut self) -> FResult<()> {
        let input = self.read_all()?;

        debug!("Input: {:?}", input);

        for _ in 0..self.n_run {
            let mut data = &input[0..];
            while !data.is_empty() {
                let read = self.apply_next(data)?;
                if read == 0 {
                    break;
                }

                data = &data[read..];
            }
        }

        Ok(())
    }
}
