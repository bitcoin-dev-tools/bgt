use anyhow::Result;
use dirs::config_dir;
use log::{debug, info};
use octocrab::{models::repos::Release, Octocrab};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::version::compare_versions;

/// Fetches all tags from the GitHub repository and updates the known tags file.
///
/// # Arguments
///
/// * `octocrab` - An instance of the Octocrab GitHub API client
/// * `config` - The configuration for the repository
///
/// # Returns
///
/// A Result containing a HashSet of all known tags, or an error if the fetch failed.
pub async fn fetch_all_tags(octocrab: &Octocrab, config: &Config) -> Result<HashSet<String>> {
    info!("Reading existing known tags from file...");
    let mut existing_tags = read_known_tags().unwrap_or_else(|_| {
        info!("No existing tags file found, starting fresh.");
        HashSet::new()
    });
    info!("Found {} existing tags", existing_tags.len());

    info!(
        "Fetching all releases from {}/{} repository...",
        config.repo_owner, config.repo_name
    );
    let releases: Vec<Release> = octocrab
        .repos(&config.repo_owner, &config.repo_name)
        .releases()
        .list()
        .per_page(100)
        .page(1u32)
        .send()
        .await?
        .items;

    let mut new_tags = Vec::new();
    for release in releases {
        if existing_tags.insert(release.tag_name.clone()) {
            new_tags.push(release.tag_name);
        }
    }

    if !new_tags.is_empty() {
        info!("tags detected since last startup:");
        for tag in &new_tags {
            info!("historical tag: {}", tag);
        }
    } else {
        info!("No new tags detected since last startup");
    }

    info!("Total known tags: {}", existing_tags.len());
    debug!("All tags: {:?}", existing_tags);

    info!("Writing updated known tags to file...");
    write_known_tags(&existing_tags)?;

    Ok(existing_tags)
}

/// Checks for new tags in the GitHub repository.
///
/// # Arguments
///
/// * `octocrab` - An instance of the Octocrab GitHub API client
/// * `seen_tags` - A mutable reference to the HashSet of known tags
/// * `config` - The configuration for the repository
///
/// # Returns
///
/// A Result containing a Vector of new tags, or an error if the check failed.
pub async fn check_for_new_tags(
    octocrab: &Octocrab,
    seen_tags: &mut HashSet<String>,
    config: &Config,
) -> Result<Vec<String>> {
    let releases: Vec<Release> = octocrab
        .repos(&config.repo_owner, &config.repo_name)
        .releases()
        .list()
        .per_page(100)
        .page(1u32)
        .send()
        .await?
        .items;

    info!("Fetched {} releases", releases.len());

    let mut new_tags = Vec::new();
    for release in releases {
        let tag_name = release.tag_name;
        if !seen_tags.contains(&tag_name) {
            info!("New tag detected: {}", tag_name);
            new_tags.push(tag_name.clone());
            seen_tags.insert(tag_name);
        }
    }

    Ok(new_tags)
}

/// Returns the path to the configuration file.
///
/// # Returns
///
/// A PathBuf representing the path to the known_releases file.
fn get_config_file_path() -> PathBuf {
    let mut path = config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("bgt");
    std::fs::create_dir_all(&path).expect("Failed to create config directory");
    path.push("known_releases");
    path
}

/// Reads known tags from the configuration file.
///
/// # Returns
///
/// A Result containing a HashSet of known tags, or an error if the file couldn't be read.
fn read_known_tags() -> Result<HashSet<String>> {
    let path = get_config_file_path();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let tags: HashSet<String> = reader.lines().map_while(Result::ok).collect();
    Ok(tags)
}

/// Writes known tags to the configuration file in sorted order.
///
/// # Arguments
///
/// * `tags` - A HashSet of tags to write to the file
///
/// # Returns
///
/// A Result indicating success or failure of the write operation.
fn write_known_tags(tags: &HashSet<String>) -> Result<()> {
    let path = get_config_file_path();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    let mut sorted_tags: Vec<_> = tags.iter().collect();
    sorted_tags.sort_by(|a, b| compare_versions(a, b));

    for tag in sorted_tags {
        writeln!(file, "{}", tag)?;
    }
    Ok(())
}
