//! # bgt-builder
//!
//! `bgt-builder` is a command-line tool for automated Guix builds of Bitcoin Core.
//!
//! This binary provides functionality to build, attest, and codesign Bitcoin Core releases.
//! It can also watch for new tags and automatically process them.
//!
//! For detailed usage instructions, please refer to the README.md file in the repository.
//!
mod builder;
mod config;
mod fetcher;
mod version;
mod wizard;
mod xor;

use std::collections::HashSet;

use anyhow::{Context, Result};
use builder::{BuildAction, Builder};
use clap::{Parser, Subcommand};
use config::Config;
use env_logger::Env;
use fetcher::{check_for_new_tags, fetch_all_tags};
use log::{debug, error, info, warn};
use tokio::signal;
use tokio::time::sleep;
use wizard::init_wizard;

// #![deny(unused_crate_dependencies)]

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Use `JOBS=1 ADDITIONAL_GUIX_COMMON_FLAGS='--max-jobs=8'`
    #[arg(long)]
    multi_package: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the setup wizard
    Setup,
    /// Build a specific tag
    Build {
        /// The tag to build
        tag: String,
    },
    /// Attest to non-codesigned build outputs
    Attest {
        /// The tag to attest to
        tag: String,
    },
    /// Attach codesignatures to existing non-codesigned outputs and attest
    Codesign {
        /// The tag to codesign
        tag: String,
    },
    /// Run a continuous watcher to monitor for new tags and automatically build them
    Watch,
    /// Clean up guix build directories leaving caches intact
    Clean,
    /// View the current configuration settings
    ShowConfig,
}

#[derive(Subcommand)]
enum AttestType {
    /// Attest without code signing
    Noncodesigned,
    /// Attest with code signing
    Codesigned,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting BGT Builder");
    let cli = Cli::parse();

    // Try to load the config file
    let mut config = match &cli.command {
        Commands::Setup => Config::default(),
        _ => read_config().context("Failed to read config")?,
    };
    if cli.multi_package {
        config.multi_package = true;
    }

    match &cli.command {
        Commands::Setup => {
            init_wizard().await?;
            // Re-read the config here, as we may have updated it
            config = read_config().context("Failed to read updated config")?;
            initialize_builder(&config).await?;
            println!("Initialization complete. You can now use bgt builder!");
        }
        Commands::Build { tag } => {
            initialize_builder(&config).await?;
            build_tag(tag.as_str(), &config).await;
        }
        Commands::Attest { tag } => {
            non_codesigned(tag, &config).await;
        }
        Commands::Codesign { tag } => {
            codesigned(tag, &config).await;
        }
        Commands::Watch => {
            let (mut seen_tags_bitcoin, mut seen_tags_sigs) = fetch_all_tags(&config).await?;
            initialize_builder(&config).await?;
            run_watcher(&config, &mut seen_tags_bitcoin, &mut seen_tags_sigs).await?;
        }
        Commands::Clean => {
            let builder = Builder::new(String::new(), BuildAction::Clean, config.clone())?;
            builder.run().await?;
        }
        Commands::ShowConfig => {
            println!("{}", config);
        }
    }

    Ok(())
}

fn read_config() -> Result<Config> {
    Config::load().map_err(|e| {
        error!(
            "Failed to load config: {}. Please run 'bgt setup' to set up your configuration.",
            e
        );
        anyhow::anyhow!("Config not properly set up")
    })
}

async fn run_watcher(
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
                info!("Shutting down...");
                break;
            }
        }
    }

    Ok(())
}

async fn build_tag(tag: &str, config: &Config) {
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

async fn non_codesigned(tag: &str, config: &Config) {
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

async fn codesigned(tag: &str, config: &Config) {
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

async fn initialize_builder(config: &Config) -> Result<Builder> {
    let builder = Builder::new(String::new(), BuildAction::Setup, config.clone())?;
    builder.init().await?;
    Ok(builder)
}
