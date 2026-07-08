use std::collections::HashSet;

use crate::builder::{BuildAction, BuildArgs};
use crate::commands::create_builder;
use crate::config::Config;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use octocrab::Octocrab;
use tokio::signal;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::sleep;

use crate::fetcher::check_for_new_tags;

/// Check if build outputs exist on disk for a given tag.
fn build_exists_on_disk(config: &Config, tag: &str) -> bool {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let output_dir = config
        .bitcoin_dir
        .join(format!("guix-build-{}/output", version));

    if !output_dir.exists() {
        debug!("Build output directory does not exist: {:?}", output_dir);
        return false;
    }

    if let Ok(entries) = std::fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let sums_file = path.join("SHA256SUMS.part");
                if sums_file.exists() {
                    debug!("Found build output: {:?}", sums_file);
                    return true;
                }
            }
        }
    }

    debug!("No SHA256SUMS.part files found in {:?}", output_dir);
    false
}

pub(crate) async fn run_watcher(
    config: &Config,
    octocrab: &Octocrab,
    seen_tags_bitcoin: &mut HashSet<String>,
    seen_tags_sigs: &mut HashSet<String>,
    auto: bool,
    dry_run: bool,
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
                loop {
                    let mut processed_tags = false;
                    match check_and_process_bitcoin_tags(config, octocrab, seen_tags_bitcoin, &mut in_progress, dry_run, auto).await {
                        Ok(processed) => processed_tags |= processed,
                        Err(e) => error!("Error processing Bitcoin tags: {:?}", e),
                    }
                    match check_and_process_sigs_tags(config, octocrab, seen_tags_sigs, &mut in_progress, dry_run, auto).await {
                        Ok(processed) => processed_tags |= processed,
                        Err(e) => error!("Error processing sigs tags: {:?}", e),
                    }
                    if !processed_tags {
                        break;
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

async fn check_and_process_bitcoin_tags(
    config: &Config,
    octocrab: &Octocrab,
    seen_tags_bitcoin: &mut HashSet<String>,
    in_progress: &mut HashSet<String>,
    dry_run: bool,
    auto: bool,
) -> Result<bool> {
    debug!("Checking for new bitcoin tags...");
    match check_for_new_tags(
        seen_tags_bitcoin,
        &config.source_repo_owner,
        &config.source_repo_name,
        octocrab,
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
                    if dry_run {
                        info!("Skipping build for tag {tag} because --dry-run is enabled");
                        seen_tags_bitcoin.insert(tag);
                        continue;
                    }
                    info!("Processing bitcoin tag {tag}");
                    let mut args = BuildArgs {
                        action: BuildAction::Build,
                        tag: Some(tag.clone()),
                        auto,
                    };
                    let builder = create_builder(config, args.clone())
                        .await
                        .context("Failed to initialize first guix builder in watcher")?;
                    builder
                        .run()
                        .await
                        .with_context(|| format!("Build process for tag {} failed", tag))?;

                    args.action = BuildAction::NonCodeSigned;
                    let builder = create_builder(config, args)
                        .await
                        .context("Failed to initialize non-codesigned builder in watcher")?;
                    builder.run().await.with_context(|| {
                        format!("Noncodesigned attestation process for tag {} failed", tag)
                    })?;
                    in_progress.insert(tag.clone());
                    seen_tags_bitcoin.insert(tag);
                }
                return Ok(true);
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
    Ok(false)
}

async fn check_and_process_sigs_tags(
    config: &Config,
    octocrab: &Octocrab,
    seen_tags_sigs: &mut HashSet<String>,
    in_progress: &mut HashSet<String>,
    dry_run: bool,
    auto: bool,
) -> Result<bool> {
    debug!("Checking for new detached sigs tags...");
    match check_for_new_tags(
        seen_tags_sigs,
        &config.detached_repo_owner,
        &config.detached_repo_name,
        octocrab,
    )
    .await
    {
        Ok(new_tags) => {
            if !new_tags.is_empty() {
                let mut processed_tags = false;
                info!(
                    "Detected {} new tags for {}/{}",
                    new_tags.len(),
                    &config.detached_repo_owner,
                    &config.detached_repo_name
                );
                for tag in new_tags {
                    if in_progress.contains(&tag) || build_exists_on_disk(config, &tag) {
                        if dry_run {
                            info!("Skipping build for sigs tag {tag} because --dry-run is enabled");
                            seen_tags_sigs.insert(tag);
                            processed_tags = true;
                            continue;
                        }
                        info!("Processing detached sigs tag {tag}");
                        let args = BuildArgs {
                            action: BuildAction::CodeSigned,
                            tag: Some(tag.clone()),
                            auto,
                        };
                        let builder = create_builder(config, args)
                            .await
                            .context("Failed to initialize builder")?;
                        builder.run().await.with_context(|| {
                            format!("Codesigned attestation process for tag {} failed", tag)
                        })?;
                        in_progress.remove(&tag);
                        seen_tags_sigs.insert(tag);
                        processed_tags = true;
                    } else {
                        warn!(
                            "Detached sigs tag {} detected but no corresponding build found. \
                             Run 'bgt build {}' and 'bgt attest {}' first, or build outputs are missing from {:?}",
                            tag, tag, tag,
                            config.bitcoin_dir.join(format!("guix-build-{}/output", tag.strip_prefix('v').unwrap_or(&tag)))
                        );
                    }
                }
                return Ok(processed_tags);
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
    Ok(false)
}
