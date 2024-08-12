use crate::builder::{BuildAction, Builder};
use crate::config::Config;
pub(crate) use crate::watcher::run_watcher;
use anyhow::Result;
use log::{error, info};

pub(crate) async fn build_tag(tag: &str, config: &Config) {
    info!("Building tag {}", tag);
    let action = BuildAction::Build;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone())
        .expect("Failed to create new Builder instance");

    info!("Using builder for tag {}:\n{}", tag, tag_builder);
    if let Err(e) = tag_builder.run().await {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

pub(crate) async fn non_codesigned(tag: &str, config: &Config) {
    info!("Attesting to non-codesigned tag {}", tag);
    let action = BuildAction::NonCodeSigned;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone())
        .expect("Failed to create new Builder instance");

    if let Err(e) = tag_builder.run().await {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

pub(crate) async fn codesigned(tag: &str, config: &Config) {
    info!("Codesigning tag {}", tag);
    let action = BuildAction::CodeSigned;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone())
        .expect("Failed to create new Builder instance");

    if let Err(e) = tag_builder.run().await {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

pub(crate) async fn initialize_builder(config: &Config) -> Result<Builder> {
    let builder = Builder::new(String::new(), BuildAction::Setup, config.clone())?;
    builder.init().await?;
    Ok(builder)
}
