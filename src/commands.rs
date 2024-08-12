use crate::builder::{BuildAction, Builder};
use crate::config::Config;
pub(crate) use crate::watcher::run_watcher;
use anyhow::{Context, Result};
use log::info;

pub(crate) async fn build_tag(tag: &str, config: &Config) -> Result<()> {
    info!("Building tag {}", tag);
    let action = BuildAction::Build;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone())
        .with_context(|| format!("Failed to create new Builder instance for tag {}", tag))?;

    info!("Using builder for tag {}:\n{}", tag, tag_builder);
    tag_builder
        .run()
        .await
        .with_context(|| format!("Build process for tag {} failed", tag))?;

    Ok(())
}

pub(crate) async fn non_codesigned(tag: &str, config: &Config) -> Result<()> {
    info!("Attesting to non-codesigned tag {}", tag);
    let action = BuildAction::NonCodeSigned;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone()).with_context(|| {
        format!(
            "Failed to create new Builder instance for non-codesigned tag {}",
            tag
        )
    })?;

    tag_builder
        .run()
        .await
        .with_context(|| format!("Non-codesigned attestation process for tag {} failed", tag))?;

    Ok(())
}

pub(crate) async fn codesigned(tag: &str, config: &Config) -> Result<()> {
    info!("Codesigning tag {}", tag);
    let action = BuildAction::CodeSigned;
    info!(
        "Creating a builder for tag {} and BuildAction {:?}",
        tag, action
    );

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), action, config.clone()).with_context(|| {
        format!(
            "Failed to create new Builder instance for codesigned tag {}",
            tag
        )
    })?;

    tag_builder
        .run()
        .await
        .with_context(|| format!("Codesigning process for tag {} failed", tag))?;

    Ok(())
}

pub(crate) async fn initialize_builder(config: &Config) -> Result<Builder> {
    let builder = Builder::new(String::new(), BuildAction::Setup, config.clone())
        .context("Failed to create new Builder instance for initialization")?;
    builder
        .init()
        .await
        .context("Failed to initialize builder")?;
    Ok(builder)
}
