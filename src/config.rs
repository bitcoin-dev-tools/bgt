use anyhow::{Context, Result};
use dirs::config_dir;
use std::{path::PathBuf, time::Duration};
use toml::Table;

#[derive(Clone)]
pub struct Config {
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_owner_detached: String,
    pub repo_name_detached: String,
    pub poll_interval: Duration,
    pub signer_name: String,
    pub gpg_key_id: String,
    pub guix_sigs_fork_url: String,
    pub multi_package: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_owner: "bitcoin".to_string(),
            repo_name: "bitcoin".to_string(),
            repo_owner_detached: "bitcoin-core".to_string(),
            repo_name_detached: "bitcoin-detached-sigs".to_string(),
            poll_interval: Duration::from_secs(60),
            signer_name: String::new(),
            gpg_key_id: String::new(),
            guix_sigs_fork_url: String::new(),
            multi_package: false,
        }
    }
}
impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_file("config.toml");
        let config_str =
            std::fs::read_to_string(config_path).context("Failed to read config file")?;
        let parsed_config: Table =
            toml::from_str(&config_str).context("Failed to parse config file")?;

        Ok(Self {
            repo_owner: "bitcoin".to_string(),
            repo_name: "bitcoin".to_string(),
            repo_owner_detached: "bitcoin-core".to_string(),
            repo_name_detached: "bitcoin-detached-sigs".to_string(),
            poll_interval: Duration::from_secs(60),
            signer_name: parsed_config["SIGNER_NAME"]
                .as_str()
                .context("SIGNER_NAME not found in config")?
                .to_string(),
            gpg_key_id: parsed_config["GPG_KEY_ID"]
                .as_str()
                .context("GPG_KEY_ID not found in config")?
                .to_string(),
            guix_sigs_fork_url: parsed_config["GUIX_SIGS_FORK"]
                .as_str()
                .context("GUIX_SIGS_FORK not found in config")?
                .to_string(),
            multi_package: false,
        })
    }
}

/// Returns the path to a configuration file.
///
/// # Returns
///
/// A PathBuf representing the path to a configuration file.
pub(crate) fn get_config_file(file: &str) -> PathBuf {
    let mut path = config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("bgt");
    std::fs::create_dir_all(&path).expect("Failed to create config directory");
    path.push(file);
    path
}
