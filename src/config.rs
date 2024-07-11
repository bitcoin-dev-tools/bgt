use anyhow::{Context, Result};
use dirs::config_dir;
use std::{path::PathBuf, time::Duration};
use toml::Table;

pub struct Config {
    pub repo_owner: String,
    pub repo_name: String,
    pub poll_interval: Duration,
    pub signer: String,
    pub guix_sigs_fork: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_owner: "bitcoin".to_string(),
            repo_name: "bitcoin".to_string(),
            poll_interval: Duration::from_secs(60),
            signer: String::new(),
            guix_sigs_fork: String::new(),
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
            poll_interval: Duration::from_secs(60),
            signer: parsed_config["SIGNER"]
                .as_str()
                .context("SIGNER not found in config")?
                .to_string(),
            guix_sigs_fork: parsed_config["GUIX_SIGS_FORK"]
                .as_str()
                .context("GUIX_SIGS_FORK not found in config")?
                .to_string(),
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
