use std::collections::HashSet;

use crate::builder::{BuildAction, BuildArgs};
use crate::commands::create_builder;
use crate::config::Config;
use anyhow::{Context, Result};
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
        config.source_repo_owner,
        config.source_repo_name,
        config.detached_repo_owner,
        config.detached_repo_name,
        config.poll_interval
    );
    let mut sigterm =
        signal(SignalKind::terminate()).context("Failed to register SIGTERM handler")?;

    loop {
        tokio::select! {
            _ = sleep(config.poll_interval) => {
                if let Err(e) = check_and_process_bitcoin_tags(config, seen_tags_bitcoin, &mut in_progress).await {
                    error!("Error processing Bitcoin tags: {:?}", e);
                }
            }
            _ = sleep(config.poll_interval) => {
                if let Err(e) = check_and_process_sigs_tags(config, seen_tags_sigs, &mut in_progress).await {
                    error!("Error processing sigs tags: {:?}", e);
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

async fn check_and_process_bitcoin_tags(
    config: &Config,
    seen_tags_bitcoin: &mut HashSet<String>,
    in_progress: &mut HashSet<String>,
) -> Result<()> {
    match check_for_new_tags(
        seen_tags_bitcoin,
        &config.source_repo_owner,
        &config.source_repo_name,
    )
    .await
    {
        Ok(new_tags) => {
            if !new_tags.is_empty() {
                info!(
                    "Detected {} new tags for {}/{}",
                    new_tags.len(),
                    &config.source_repo_owner,
                    &config.source_repo_name
                );
                for tag in new_tags {
                    // TODO: check for auto here
                    // args.auto = true;

                    // Build first

                    let mut args = BuildArgs {
                        action: BuildAction::Build,
                        tag: Some(tag.clone()),
                        ..Default::default()
                    };
                    let builder = create_builder(config, args.clone())
                        .await
                        .context("Failed to initialize first builder in watcher")?;
                    in_progress.insert(tag.clone());
                    builder
                        .run()
                        .await
                        .with_context(|| format!("Build process for tag {} failed", tag))?;

                    // Then attest to noncodesigned
                    args.action = BuildAction::NonCodeSigned;
                    let builder = create_builder(config, args)
                        .await
                        .context("Failed to initialize second builder in watcher")?;
                    builder.run().await.with_context(|| {
                        format!("Noncodesigned attestation process for tag {} failed", tag)
                    })?;
                }
            } else {
                debug!(
                    "No new tags for {}/{} found",
                    &config.source_repo_owner, &config.source_repo_name
                );
            }
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "Error checking for new tags in {}",
                    &config.source_repo_name
                )
            });
        }
    }
    Ok(())
}

async fn check_and_process_sigs_tags(
    config: &Config,
    seen_tags_sigs: &mut HashSet<String>,
    in_progress: &mut HashSet<String>,
) -> Result<()> {
    match check_for_new_tags(
        seen_tags_sigs,
        &config.detached_repo_owner,
        &config.detached_repo_name,
    )
    .await
    {
        Ok(new_tags) => {
            if !new_tags.is_empty() {
                info!(
                    "Detected {} new tags for {}/{}",
                    new_tags.len(),
                    &config.detached_repo_owner,
                    &config.detached_repo_name
                );
                for tag in new_tags {
                    if in_progress.contains(&tag) {
                        let args = BuildArgs {
                            action: BuildAction::CodeSigned,
                            tag: Some(tag.clone()),
                            ..Default::default()
                        };
                        let builder = create_builder(config, args)
                            .await
                            .context("Failed to initialize builder")?;
                        builder.run().await.with_context(|| {
                            format!("Codesigned attestation process for tag {} failed", tag)
                        })?;
                        in_progress.remove(&tag);
                    } else {
                        // TODO: Consider implementing the codesigning attempt here
                        warn!("New tag detected in {}/{} was not in-progress (already built and non-codesigned) and so cannot be automatically codesigned", &config.detached_repo_owner, &config.detached_repo_name);
                    }
                }
            } else {
                debug!(
                    "No new tags for {}/{} found",
                    &config.detached_repo_owner, &config.detached_repo_name
                );
            }
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "Error checking for new tags in {}",
                    &config.detached_repo_name
                )
            });
        }
    }
    Ok(())
}
