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
    fn should_replace(&self, input: &[u8]) -> bool {
        false
    }
}

pub struct Context {
    input: Box<dyn std::io::Read>,
    output: Box<dyn std::io::Write>,

    words: Vec<Word>,
    target: Target,
    rand: Rand,
}

impl Context {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        todo!()
    }

    fn select_word(&mut self) -> &Word {
        let index = self.rand.next_range(0, self.words.len() as u64);

        &self.words[index as usize]
    }

    fn apply(&mut self) -> FResult<()> {
        Ok(())
    }
}
