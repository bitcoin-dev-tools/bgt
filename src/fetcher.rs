use anyhow::{Context, Result};
use log::{debug, info};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::config::{get_config_file, Config};
use crate::version::compare_versions;

/// Fetches all tags from the GitHub repository and updates the known tags file.
///
/// # Returns
///
/// A Result tuple of HashSets of all known tags for each of the two repos, or an error if the fetch failed.
pub async fn fetch_all_tags(config: &Config) -> Result<(HashSet<String>, HashSet<String>)> {
    let mut bitcoin_tags = HashSet::new();
    let mut sig_tags = HashSet::new();

    for (repo_type, owner, name, tags_file, tag_set) in [
        (
            "bitcoin",
            &config.repo_owner,
            &config.repo_name,
            "known_tags_bitcoin",
            &mut bitcoin_tags,
        ),
        (
            "sigs",
            &config.repo_owner_detached,
            &config.repo_name_detached,
            "known_tags_sigs",
            &mut sig_tags,
        ),
    ] {
        info!("Processing {} repository", repo_type);

        info!("Reading existing known tags from file...");
        let path = get_config_file(tags_file);
        let mut existing_tags = read_known_tags(&path).unwrap_or_else(|_| {
            info!("No existing tags file found, starting fresh.");
            HashSet::new()
        });
        info!("Found {} existing tags", existing_tags.len());

        info!("Fetching all tags from {}/{} repository...", owner, name);

        let output = Command::new("curl")
            .args([
                "-H",
                "User-Agent: BGT-Builder",
                &format!(
                    "https://api.github.com/repos/{}/{}/git/refs/tags",
                    owner, name
                ),
            ])
            .output()
            .context("Failed to execute curl command")?;

        let tags: Vec<Value> = serde_json::from_slice(&output.stdout)
            .context("Failed to parse JSON response from GitHub API")?;

        let mut new_tags = Vec::new();
        for tag in &tags {
            if let Some(ref_value) = tag.get("ref") {
                if let Some(ref_str) = ref_value.as_str() {
                    let tag_name = ref_str.trim_start_matches("refs/tags/").to_string();
                    if existing_tags.insert(tag_name.clone()) {
                        new_tags.push(tag_name.clone());
                        tag_set.insert(tag_name);
                    }
                }
            }
        }

        if !new_tags.is_empty() {
            info!(
                "New tags detected for {} repository since last startup:",
                repo_type
            );
            for tag in &new_tags {
                info!("New tag: {}", tag);
            }
        } else {
            info!(
                "No new tags detected for {} repository since last startup",
                repo_type
            );
        }

        info!(
            "Total known tags for {}: {}",
            repo_type,
            existing_tags.len()
        );
        debug!("All tags for {}: {:?}", repo_type, existing_tags);

        info!("Writing updated known tags to file for {}...", repo_type);
        write_known_tags(&existing_tags, &path)
            .context("Failed to write updated known tags to file")?;

        tag_set.extend(existing_tags);
    }
    info!(
        "Total known tags across both repositories: {}",
        bitcoin_tags.len() + sig_tags.len()
    );

    info!(
        "Initialized with {} existing tags for {}/{}",
        bitcoin_tags.len(),
        &config.repo_owner,
        &config.repo_name
    );
    info!(
        "Initialized with {} existing tags for {}/{}",
        sig_tags.len(),
        &config.repo_owner_detached,
        &config.repo_name_detached
    );

    Ok((bitcoin_tags, sig_tags))
}

/// Checks for new tags in the GitHub repository.
///
/// # Returns
///
/// A Result containing a Vector of new tags, or an error if the check failed.
pub async fn check_for_new_tags(
    seen_tags: &mut HashSet<String>,
    repo_owner: &str,
    repo_name: &str,
) -> Result<Vec<String>> {
    let output = Command::new("curl")
        .args([
            "-H",
            "User-Agent: BGT-Builder",
            &format!(
                "https://api.github.com/repos/{}/{}/git/refs/tags",
                repo_owner, repo_name
            ),
        ])
        .output()
        .context("Failed to execute curl command to fetch tags")?;

    let tags: Vec<Value> = serde_json::from_slice(&output.stdout)
        .context("Failed to parse JSON response from GitHub API")?;

    info!("Fetched {} tags", tags.len());
    let mut new_tags = Vec::new();
    for tag in tags {
        let tag_name = tag["ref"]
            .as_str()
            .context("Failed to extract tag name from GitHub API response")?
            .trim_start_matches("refs/tags/")
            .to_string();
        if !seen_tags.contains(&tag_name) {
            info!("New tag detected: {}", tag_name);
            new_tags.push(tag_name.clone());
            seen_tags.insert(tag_name);
        }
    }
    Ok(new_tags)
}

/// Reads known tags from a file.
///
/// # Returns
///
/// A Result containing a HashSet of known tags, or an error if the file couldn't be read.
fn read_known_tags(path: &PathBuf) -> Result<HashSet<String>> {
    let file = File::open(path).context("Failed to open known tags file")?;
    let reader = BufReader::new(file);
    let tags: HashSet<String> = reader
        .lines()
        .map(|line| line.context("Failed to read line from known tags file"))
        .collect::<Result<_>>()?;
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
fn write_known_tags(tags: &HashSet<String>, path: &PathBuf) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .context("Failed to open file for writing known tags")?;

    let mut sorted_tags: Vec<_> = tags.iter().collect();
    sorted_tags.sort_by(|a, b| compare_versions(a, b));

    for tag in sorted_tags {
        writeln!(file, "{}", tag).context("Failed to write tag to file")?;
    }
    Ok(())
}
