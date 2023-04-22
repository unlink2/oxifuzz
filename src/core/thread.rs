use super::{config::Config, transform::Context};

// Runs a context as a thread
pub struct Thread {
    n_threads: u32,
    src_ctx: Context,
}

impl Thread {
    pub fn from_cfg(cfg: &Config) -> FResult<Self> {
        Ok(Self {
            n_threads: 0,
            src_ctx: Context::from_cfg(cfg)?,
        })
    }

    pub fn run(&self) -> FResult<()> {
        if self.n_threads <= 1 {
            self.src_ctx.clone().apply(input, output)?;
        }
        Ok(())
    }
}
