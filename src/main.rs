mod builder;
mod config;
mod fetcher;
mod version;
mod xor;

use anyhow::Result;
use builder::{BuildAction, Builder};
use config::Config;
use env_logger::Env;
use fetcher::{check_for_new_tags, fetch_all_tags};
use log::{debug, error, info};
use octocrab::Octocrab;
use tokio::signal;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting BGT Builder");
    let config = Config::default();

    info!("Creating Octocrab instance");
    let octocrab = Octocrab::builder().build()?;

    // TODO: Should do a tool check here, we need:
    // - guix
    // - git
    // - ??
    // check_tools_installed();

    // Test init a dummy builder early to catch configuration errors
    let _ = match initialize_builder().await {
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
    // build_tag("v26.2").await;

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
                                build_tag(&tag).await;
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

async fn build_tag(tag: &str) {
    info!("New tag detected and being built: {}", tag);

    // Create a new Builder instance with the tag to operate on
    let tag_builder = Builder::new(tag.to_string(), BuildAction::Build)
        .expect("Failed to create new Builder instance");

    info!("Using builder for tag {}:\n{}", tag, tag_builder);
    if let Err(e) = tag_builder.run().await {
        error!("Build process for tag {} failed: {:?}", tag, e);
    }
}

async fn initialize_builder() -> Result<Builder> {
    let builder = Builder::new(String::new(), BuildAction::Setup)?;
    builder.init().await?;
    Ok(builder)
}
