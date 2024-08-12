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
mod commands;
mod config;
mod daemon;
mod fetcher;
mod version;
mod watcher;
mod wizard;
mod xor;

use std::process::Command;

use anyhow::{Context, Result};
use builder::{BuildAction, Builder};
use clap::{Parser, Subcommand};
use config::Config;
use env_logger::Env;
use log::info;

use crate::commands::{build_tag, codesigned, initialize_builder, non_codesigned, run_watcher};
use crate::config::{get_config_file, read_config};
use crate::daemon::{start_daemon, stop_daemon};
use crate::fetcher::fetch_all_tags;
use crate::wizard::init_wizard;

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
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },
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

#[derive(Subcommand)]
enum WatchAction {
    /// Start the watcher daemon
    Start {
        /// Daemonize to background process
        #[arg(long)]
        daemon: bool,
        /// Attempt to automatically attest using gpg
        #[arg(long)]
        auto: bool,
    },
    /// Stop the watcher daemon
    Stop,
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
            info!("Initialization complete. You can now use bgt builder!");
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
        Commands::Watch { action } => {
            let pid_file = get_config_file("watch.pid");
            let log_file = get_config_file("watch.log");

            match action {
                WatchAction::Start { daemon, auto } => {
                    if *auto {
                        info!("Checking for automatic GPG signing capability when using --auto flag...");
                        check_gpg_signing(&config.gpg_key_id)
                            .context("Failed to verify GPG signing capability")?;
                        info!("GPG signing check passed.");
                    }
                    if *daemon {
                        info!("Starting BGT watcher as a daemon...");
                        info!("View logs at: {}.", log_file.display());
                        start_daemon(&pid_file, &log_file)?;
                    } else {
                        info!("Starting BGT watcher in the foreground...");
                    }
                    let (mut seen_tags_bitcoin, mut seen_tags_sigs) =
                        fetch_all_tags(&config).await?;
                    initialize_builder(&config).await?;
                    run_watcher(&config, &mut seen_tags_bitcoin, &mut seen_tags_sigs).await?;
                }
                WatchAction::Stop => {
                    info!("Stopping BGT watcher daemon...");
                    stop_daemon(&pid_file)?;
                }
            }
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

pub fn check_gpg_signing(key_id: &str) -> Result<()> {
    let output = Command::new("gpg")
        .args([
            "--batch",
            "--dry-run",
            "--local-user",
            key_id,
            "--armor",
            "--sign",
            "--output",
            "/dev/null",
            "/dev/null",
        ])
        .output()
        .context("Failed to execute GPG command")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("GPG signing check failed: {}", stderr)
    }
}
