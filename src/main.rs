mod builder;
mod config;
mod fetcher;
mod version;

use anyhow::{Context, Result};
use builder::{BuildAction, Builder};
use config::Config;
use fetcher::{check_for_new_tags, fetch_all_tags};
use log::{debug, error, info};
use octocrab::Octocrab;
use std::env;
use tokio::signal;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting BGT Builder");
    let config = Config::default();

    info!("Creating Octocrab instance");
    let octocrab = Octocrab::builder().build()?;

    // Initialize the builder early to catch configuration errors
    let builder = match initialize_builder().await {
        Ok(b) => {
            info!("Builder initialized successfully:\n{}", b);
            b
        }
        Err(e) => {
            error!("Failed to initialize builder: {:?}", e);
            return Err(e);
        }
    };

    // Initialize seen_tags with all existing tags
    let mut seen_tags = fetch_all_tags(&octocrab, &config).await?;
    info!("Initialized with {} existing tags", seen_tags.len());

    loop {
        info!(
            "Polling https://github.com/{}/{} for new tags...",
            config.repo_owner, config.repo_name
        );
        tokio::select! {
            _ = sleep(config.poll_interval) => {
                match check_for_new_tags(&octocrab, &mut seen_tags, &config).await {
                    Ok(new_tags) => {
                        if !new_tags.is_empty() {
                            info!("Detected {} new tags", new_tags.len());
                            for tag in new_tags {
                                info!("Building tag {}", tag);
                                build_tag(&tag, &builder);
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

fn build_tag(tag: &str, builder: &Builder) {
    info!("New tag detected: {}", tag);

    // Create a new Builder instance with the same signer, and new tag
    let tag_builder = Builder::new(builder.signer.clone(), tag.to_string(), BuildAction::Build)
        .expect("Failed to create new Builder instance");

    info!("Using builder for tag {}:\n{}", tag, tag_builder);
    if let Err(e) = tag_builder.run() {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

async fn initialize_builder() -> Result<Builder> {
    let signer = env::var("SIGNER").context("SIGNER environment variable not set")?;
    let builder = Builder::new(signer, String::new(), BuildAction::Setup)?;
    builder.init().await?;
    Ok(builder)
}
