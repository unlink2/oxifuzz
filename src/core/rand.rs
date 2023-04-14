use std::path::PathBuf;

use rand::prelude::*;

#[derive(Clone)]
pub enum Rand {
    Random(StdRng),
    File(PathBuf),
}

impl Default for Rand {
    fn default() -> Self {
        Self::Random(StdRng::from_entropy())
    }
}

impl Rand {
    pub fn from_seed(seed: u64) -> Self {
        Self::Random(StdRng::seed_from_u64(seed))
    }

    pub fn next(&mut self) -> u64 {
        match self {
            Rand::Random(rng) => rng.gen(),
            Rand::File(_path) => todo!(),
        }
    }

    pub fn next_range(&mut self, from: u64, to: u64) -> u64 {
        match self {
            Rand::Random(rng) => rng.gen_range(from..to),
            Rand::File(_) => todo!(),
        }
    }
}
