use std::collections::HashSet;

use crate::commands::{build_tag, codesigned, non_codesigned};
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
        config.repo_owner,
        config.repo_name,
        config.repo_owner_detached,
        config.repo_name_detached,
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
    match check_for_new_tags(seen_tags_bitcoin, &config.repo_owner, &config.repo_name).await {
        Ok(new_tags) => {
            if !new_tags.is_empty() {
                info!(
                    "Detected {} new tags for {}/{}",
                    new_tags.len(),
                    &config.repo_owner,
                    &config.repo_name
                );
                for tag in new_tags {
                    in_progress.insert(tag.clone());
                    build_tag(&tag, config)
                        .await
                        .with_context(|| format!("Failed to build tag {}", tag))?;
                    non_codesigned(&tag, config)
                        .await
                        .with_context(|| format!("Failed to attest non-codesigned tag {}", tag))?;
                }
            } else {
                debug!(
                    "No new tags for {}/{} found",
                    &config.repo_owner, &config.repo_name
                );
            }
        }
        Err(e) => {
            return Err(e)
                .with_context(|| format!("Error checking for new tags in {}", &config.repo_name));
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
        &config.repo_owner_detached,
        &config.repo_name_detached,
    )
    .await
    {
        Ok(new_tags) => {
            if !new_tags.is_empty() {
                info!(
                    "Detected {} new tags for {}/{}",
                    new_tags.len(),
                    &config.repo_owner_detached,
                    &config.repo_name_detached
                );
                for tag in new_tags {
                    if in_progress.contains(&tag) {
                        codesigned(&tag, config)
                            .await
                            .with_context(|| format!("Failed to codesign tag {}", tag))?;
                        in_progress.remove(&tag);
                    } else {
                        // TODO: Consider implementing the codesigning attempt here
                        warn!("New tag detected in {}/{} was not in-progress (already built and non-codesigned) and so cannot be automatically codesigned", &config.repo_owner_detached, &config.repo_name_detached);
                    }
                }
            } else {
                debug!(
                    "No new tags for {}/{} found",
                    &config.repo_owner_detached, &config.repo_name_detached
                );
            }
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "Error checking for new tags in {}",
                    &config.repo_name_detached
                )
            });
        }
    }
    Ok(())
}
