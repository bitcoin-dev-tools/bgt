use std::collections::HashSet;

use crate::builder::{BuildAction, Builder};
use crate::config::Config;
use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::signal;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::sleep;

use crate::fetcher::check_for_new_tags;

pub(crate) async fn run_watcher(
    config: &Config,
    seen_tags_bitcoin: &mut HashSet<String>,
    seen_tags_sigs: &mut HashSet<String>,
) -> Result<()> {
    let mut in_progress: HashSet<String> = HashSet::new();
    info!(
        "Polling {}/{} and {}/{} for new tags every {:?}...",
        config.repo_owner,
        config.repo_name,
        config.repo_owner_detached,
        config.repo_name_detached,
        config.poll_interval
    );
    let mut sigterm = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = sleep(config.poll_interval) => {
                match check_for_new_tags(seen_tags_bitcoin, &config.repo_owner, &config.repo_name).await {
                    Ok(new_tags) => {
                        if !new_tags.is_empty() {
                            info!("Detected {} new tags for {}/{}", new_tags.len(), &config.repo_owner, &config.repo_name);
                            for tag in new_tags {
                                in_progress.insert(tag.clone());
                                build_tag(&tag, config).await;
                                non_codesigned(&tag, config).await;
                            }
                        } else {
                            debug!("No new tags for {}/{} found", &config.repo_owner, &config.repo_name);
                        }
                    }
                    Err(e) => {
                        error!("Error checking for new tags in {}: {:?}", &config.repo_name, e);
                    }
                }
            }
            _ = sleep(config.poll_interval) => {
                match check_for_new_tags(seen_tags_sigs, &config.repo_owner_detached, &config.repo_name_detached).await {
                    Ok(new_tags) => {
                        if !new_tags.is_empty() {
                            info!("Detected {} new tags for {}/{}", new_tags.len(), &config.repo_owner_detached, &config.repo_name_detached);
                            for tag in new_tags {
                                if in_progress.contains(&tag) {
                                    codesigned(&tag, config).await;
                                    in_progress.remove(&tag);
                                } else {
                                    // TODO: I think here we could probably try codesigning first,
                                    // in case we are out of sync with in_progress, and warn if we
                                    // catch an error?
                                    warn!("New tag detected in {}/{} was in-progress (already build and non-codesigned) and so cannot be automatically codesigned", &config.repo_owner_detached, &config.repo_name_detached);
                                }
                            }
                        } else {
                            debug!("No new tags for {}/{} found", &config.repo_owner_detached, &config.repo_name_detached);
                        }
                    }
                    Err(e) => {
                        error!("Error checking for new tags in {}: {:?}", &config.repo_name_detached, e);
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C. Shutting down...");
                break;
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM. Shutting down...");
                break;
            }
        }
    }
    info!("Watcher stopped.");
    Ok(())
}

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
