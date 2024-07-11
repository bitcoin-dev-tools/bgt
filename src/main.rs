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
}

#[derive(Subcommand)]
enum Commands {
    /// Run the watcher to monitor for new tags
    Watcher,
    /// Build a specific tag
    Tag {
        /// The tag to build
        tag: String,
    },
    /// Configure a few settings and write them to a config file
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting BGT Builder");
    let cli = Cli::parse();

    // Try to load the config file
    let config = match Config::load() {
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

    match &cli.command {
        Commands::Watcher => {
            // Initialize seen_tags with all existing tags
            let mut seen_tags = fetch_all_tags(&config).await?;
            info!("Initialized with {} existing tags", seen_tags.len());
            initialize_builder(&config).await?;
            run_watcher(&config, &mut seen_tags).await?;
        }
        Commands::Tag { tag } => {
            initialize_builder(&config).await?;
            build_tag(tag.as_str(), &config).await;
        }
        Commands::Init => {
            init_wizard().await?;
        }
    }

    Ok(())
}

async fn run_watcher(config: &Config, seen_tags: &mut HashSet<String>) -> Result<()> {
    loop {
        info!(
            "Polling https://github.com/{}/{} for new tags...",
            config.repo_owner, config.repo_name
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
    info!("New tag detected and being built: {}", tag);

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(
        tag.to_string(),
        BuildAction::Build,
        config.signer.clone(),
        config.guix_sigs_fork.clone(),
    )
    .expect("Failed to create new Builder instance");

    info!("Using builder for tag {}:\n{}", tag, tag_builder);
    if let Err(e) = tag_builder.run().await {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

async fn initialize_builder(config: &Config) -> Result<Builder> {
    let builder = Builder::new(
        String::new(),
        BuildAction::Setup,
        config.signer.clone(),
        config.guix_sigs_fork.clone(),
    )?;
    builder.init().await?;
    Ok(builder)
}
