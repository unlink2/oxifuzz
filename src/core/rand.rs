use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use log::debug;
use rand::prelude::*;

use super::error::FResult;

/// Generic file reader with a buffer
pub struct FileReader {
    path: PathBuf,
    reader: Option<Box<dyn std::io::Read>>,
    buffer: [u8; std::mem::size_of::<u64>()],
}

/// Rng wrapper that can either use the rand crate
/// or read random data from a file which is intended to be used with
/// files such as /dev/urandom
pub enum Rand {
    Random(Box<StdRng>),
    File(FileReader),
}

impl Clone for Rand {
    fn clone(&self) -> Self {
        match self {
            Self::Random(val) => Self::Random(val.clone()),
            Self::File(r) => Self::from_path(&r.path),
        }
    }
}

impl Default for Rand {
    fn default() -> Self {
        Self::Random(Box::new(StdRng::from_entropy()))
    }
}

impl Rand {
    /// Create a new random file from seed using the rand crate
    pub fn from_seed(seed: u64) -> Self {
        Self::Random(Box::new(StdRng::seed_from_u64(seed)))
    }

    /// Create a new file-based rand
    pub fn from_path(path: &Path) -> Self {
        Self::File(FileReader {
            path: path.into(),
            reader: None,
            buffer: [0; std::mem::size_of::<u64>()],
        })
    }

    /// Read u64 form the file-based rand
    fn read_u64_from(r: &mut FileReader) -> FResult<u64> {
        if r.reader.is_none() {
            debug!("Opening file {:?}", r.path);
            r.reader = Some(Box::new(BufReader::new(File::open(&r.path)?)));
        }
        // this should always be ok
        let reader = r.reader.as_mut().unwrap();
        reader.read_exact(&mut r.buffer)?;
        Ok(u64::from_ne_bytes(r.buffer))
    }

    /// generate a random u64
    pub fn next_gen(&mut self) -> FResult<u64> {
        match self {
            Rand::Random(rng) => Ok(rng.gen()),
            Rand::File(r) => Self::read_u64_from(r),
        }
    }

    /// generate a random u64 within a certain range
    pub fn next_range(&mut self, from: u64, to: u64) -> FResult<u64> {
        match self {
            Rand::Random(rng) => Ok(rng.gen_range(from..to)),
            Rand::File(r) => Ok((Self::read_u64_from(r)? & to).wrapping_add(from)),
        }
    }
}
