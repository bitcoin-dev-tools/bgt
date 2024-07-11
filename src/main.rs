mod builder;
mod config;
mod fetcher;
mod version;
mod wizard;
mod xor;

use std::collections::HashSet;

use anyhow::Result;
use builder::{BuildAction, Builder};
use clap::{Parser, Subcommand};
use config::Config;
use env_logger::Env;
use fetcher::{check_for_new_tags, fetch_all_tags};
use log::{debug, error, info};
use tokio::signal;
use tokio::time::sleep;
use wizard::init_wizard;

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
    /// Configure settings and write them to a config file
    Init,
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
    let mut config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            if let Commands::Init = cli.command {
                // If the command is Init, we don't need the config yet
                Config::default()
            } else {
                error!("Failed to load config: {}. Please run 'bgt init' to set up your configuration.", e);
                return Err(anyhow::anyhow!("Config not properly set up"));
            }
        }
    };
    if cli.multi_package {
        config.multi_package = true;
    }

    match &cli.command {
        Commands::Init => {
            init_wizard().await?;
        }
        Commands::Build { tag } => {
            initialize_builder(&config).await?;
            build_tag(tag.as_str(), &config).await;
        }
        Commands::Attest { tag } => {
            info!("Performing non code-signed attestation for tag: {}", tag);
            non_codesigned(tag, &config).await;
        }
        Commands::Codesign { tag } => {
            info!("Performing code-signed attestation for tag: {}", tag);
            codesigned(tag, &config).await;
        }
        Commands::Watch => {
            // Initialize seen_tags with all existing tags
            let mut seen_tags = fetch_all_tags(&config).await?;
            info!("Initialized with {} existing tags", seen_tags.len());
            initialize_builder(&config).await?;
            run_watcher(&config, &mut seen_tags).await?;
        }
        Commands::Clean => {
            unimplemented!();
        }
    }

    Ok(())
}

async fn run_watcher(config: &Config, seen_tags: &mut HashSet<String>) -> Result<()> {
    loop {
        info!(
            "Polling https://github.com/{}/{} for new tags every {:?}s...",
            config.repo_owner, config.repo_name, config.poll_interval
        );
        tokio::select! {
            _ = sleep(config.poll_interval) => {
                match check_for_new_tags(seen_tags, config).await {
                    Ok(new_tags) => {
                        if !new_tags.is_empty() {
                            info!("Detected {} new tags", new_tags.len());
                            for tag in new_tags {
                                info!("Building tag {}", tag);
                                build_tag(&tag, config).await;
                            }
                        } else {
                            debug!("No new tags detected.");
                        }
                    }
                    Err(e) => {
                        error!("Error checking for new tags: {:?}", e);
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
