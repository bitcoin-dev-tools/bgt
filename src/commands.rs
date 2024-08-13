use crate::builder::{BuildArgs, Builder};
use crate::config::Config;
pub(crate) use crate::watcher::run_watcher;
use anyhow::{Context, Result};

pub(crate) async fn create_builder(config: &Config, args: BuildArgs) -> Result<Builder> {
    let builder = Builder::new(config.clone(), args)
        .context("Failed to create new Builder instance for initialization")?;
    builder
        .init()
        .await
        .context("Failed to initialize builder")?;
    Ok(builder)
}
