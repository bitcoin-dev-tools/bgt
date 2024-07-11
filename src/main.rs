mod builder;
mod config;
mod fetcher;
mod version;

use anyhow::Result;
use builder::{BuildAction, Builder};
use config::Config;
use fetcher::{check_for_new_tags, fetch_all_tags};
use log::{debug, error, info};
use octocrab::Octocrab;
use tokio::signal;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting BGT Builder");
    let config = Config::default();

    info!("Creating Octocrab instance");
    let octocrab = Octocrab::builder().build()?;

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
                                new_tag_detected(&tag);
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

fn new_tag_detected(tag: &str) {
    info!("New tag detected: {}", tag);

    // Initialize and run the builder
    match Builder::new(
        "your_signer_name".to_string(),
        tag.to_string(),
        BuildAction::All,
    ) {
        Ok(builder) => {
            if let Err(e) = builder.run() {
                error!("Build process failed: {:?}", e);
            }
        }
        Err(e) => error!("Failed to initialize builder: {:?}", e),
    }
}
