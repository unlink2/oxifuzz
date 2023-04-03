pub enum Rand {
    GetRandom,
    File(PathBuf),
}

impl Rand {
    pub fn new() -> Self {
        todo!()
    }

    pub fn from_seed(seed: u32) -> Self {
        todo!()
    }

    pub fn next(&self) -> u32 {
        todo!()
    }
}
