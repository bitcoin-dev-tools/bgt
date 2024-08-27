//! # bgt
//!
//! `bgt` is a command-line tool for automated Guix builds of Bitcoin Core.
//!
//! This binary provides functionality to build, attest, and codesign Bitcoin Core releases.
//! It can also watch for new tags and automatically process them.
//!
//! For detailed usage instructions, please refer to the README.md file in the repository.

use anyhow::{Context, Result};
use clap::Parser;
use env_logger::Env;
use log::info;

mod builder;
mod commands;
mod config;
mod daemon;
mod fetcher;
mod version;
mod watcher;
mod wizard;
mod xor;

use builder::{BuildAction, BuildArgs};
use clap::Subcommand;
use config::Config;

use crate::commands::{create_builder, run_watcher};
use crate::config::{get_config_file, read_config};
use crate::daemon::{start_daemon, stop_daemon};
use crate::fetcher::fetch_all_tags;
use crate::wizard::init_wizard;

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
        /// Attempt to automatically sign using gpg and automatically open a PR on GitHub
        #[arg(long)]
        auto: bool,
    },
    /// Attach codesignatures to existing non-codesigned outputs and attest
    Codesign {
        /// The tag to codesign
        tag: String,
        /// Attempt to automatically sign using gpg and automatically open a PR on GitHub
        #[arg(long)]
        auto: bool,
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
    /// Guix build current master to populate Guix caches
    Warmup,
}

#[derive(Subcommand)]
enum WatchAction {
    /// Start the watcher daemon
    Start {
        /// Daemonize to background process
        #[arg(long)]
        daemon: bool,
        /// Attempt to automatically attest using gpg and automatically open a PR on GitHub
        #[arg(long)]
        auto: bool,
    },
    /// Stop the watcher daemon
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting BGT Builder");

    let mut config = match &cli.command {
        Commands::Setup => Config::default(),
        _ => read_config().context("Failed to read config")?,
    };
    if cli.multi_package {
        config.multi_package = true;
    }

    match cli.command {
        Commands::Setup => setup().await?,
        Commands::Build { tag } => build(&config, &tag).await?,
        Commands::Attest { tag, auto } => attest(&config, &tag, auto).await?,
        Commands::Codesign { tag, auto } => codesign(&config, &tag, auto).await?,
        Commands::Watch { action } => watch(&config, action).await?,
        Commands::Clean => clean(&config).await?,
        Commands::ShowConfig => show_config(&config),
        Commands::Warmup => warmup(&config).await?,
    }

    Ok(())
}

/// Run the setup wizard and initialize the builder
async fn setup() -> Result<()> {
    init_wizard().await.context("Failed to run setup wizard")?;
    let updated_config = read_config().context("Failed to read updated config")?;
    let _ = create_builder(&updated_config, BuildArgs::default())
        .await
        .context("Failed to initialize builder")?;
    info!("Builder successfully initialised. bgt ready.");
    Ok(())
}

/// Build a specific tag
async fn build(config: &Config, tag: &str) -> Result<()> {
    let args = BuildArgs {
        action: BuildAction::Build,
        tag: Some(tag.to_string()),
        ..Default::default()
    };
    let builder = create_builder(config, args)
        .await
        .context("Failed to initialize builder")?;
    builder
        .run()
        .await
        .with_context(|| format!("Build process for tag {} failed", tag))
}

/// Attest to non-codesigned build outputs
async fn attest(config: &Config, tag: &str, auto: bool) -> Result<()> {
    let args = BuildArgs {
        action: BuildAction::NonCodeSigned,
        tag: Some(tag.to_string()),
        auto,
        ..Default::default()
    };
    let builder = create_builder(config, args)
        .await
        .context("Failed to initialize builder")?;
    builder
        .run()
        .await
        .with_context(|| format!("Noncodesigned attestation process for tag {} failed", tag))
}

/// Attach codesignatures to existing non-codesigned outputs and attest
async fn codesign(config: &Config, tag: &str, auto: bool) -> Result<()> {
    let args = BuildArgs {
        action: BuildAction::CodeSigned,
        tag: Some(tag.to_string()),
        auto,
        ..Default::default()
    };
    let builder = create_builder(config, args)
        .await
        .context("Failed to initialize builder")?;
    builder
        .run()
        .await
        .with_context(|| format!("Codesigned attestation process for tag {} failed", tag))
}

/// Run a continuous watcher to monitor for new tags and automatically build them
async fn watch(config: &Config, action: WatchAction) -> Result<()> {
    let pid_file = get_config_file("watch.pid");
    let log_file = get_config_file("watch.log");

    match action {
        WatchAction::Start { daemon, auto } => {
            if auto {
                info!("Checking for automatic GPG signing capability when using --auto flag...");
                check_gpg_signing(&config.gpg_key_id)
                    .context("Failed to verify GPG signing capability")?;
                info!("GPG signing check passed.");
            }
            if daemon {
                info!("Starting BGT watcher as a daemon...");
                info!("View logs at: {}.", log_file.display());
                start_daemon(&pid_file, &log_file).context("Failed to start daemon")?;
            } else {
                info!("Starting BGT watcher in the foreground...");
            }
            let (mut seen_tags_bitcoin, mut seen_tags_sigs) = fetch_all_tags(config)
                .await
                .context("Failed to fetch initial tags")?;
            let args = BuildArgs {
                auto,
                ..Default::default()
            };
            create_builder(config, args)
                .await
                .context("Failed to initialize builder")?;
            run_watcher(config, &mut seen_tags_bitcoin, &mut seen_tags_sigs)
                .await
                .context("Watcher encountered an error")
        }
        WatchAction::Stop => {
            info!("Stopping BGT watcher daemon...");
            stop_daemon(&pid_file).context("Failed to stop daemon")
        }
    }
}

/// Clean up guix build directories leaving caches intact
async fn clean(config: &Config) -> Result<()> {
    let args = BuildArgs {
        action: BuildAction::Clean,
        ..Default::default()
    };
    let builder = create_builder(config, args)
        .await
        .context("Failed to initialize builder")?;
    builder.run().await.context("Failed to run clean action")
}

/// View the current configuration settings
fn show_config(config: &Config) {
    println!("{}", config);
}

/// Guix build current master to populate Guix caches
async fn warmup(config: &Config) -> Result<()> {
    let args = BuildArgs {
        action: BuildAction::Warmup,
        ..Default::default()
    };
    let builder = create_builder(config, args)
        .await
        .context("Failed to initialize builder")?;
    builder
        .run()
        .await
        .context("Build process for tag warmup failed")
}

/// Check if GPG signing is possible with the given key short ID
fn check_gpg_signing(key_id: &str) -> Result<()> {
    use anyhow::bail;
    use std::process::Command;

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
        bail!("GPG signing check failed: {}", stderr)
    }
}
