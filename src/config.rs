use anyhow::{Context, Result};
use dirs::{config_dir, state_dir};
use std::fmt;
use std::{path::PathBuf, time::Duration};
use toml::Table;

#[derive(Clone, serde::Serialize)]
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
    pub guix_build_dir: PathBuf,
    pub guix_sigs_dir: PathBuf,
    pub bitcoin_detached_sigs_dir: PathBuf,
    pub macos_sdks_dir: PathBuf,
    pub bitcoin_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let state = state_dir().unwrap_or_else(|| PathBuf::from("."));
        let guix_build_dir = state.join("guix-builds");
        Self {
            repo_owner: "bitcoin".to_string(),
            repo_name: "bitcoin".to_string(),
            repo_owner_detached: "bitcoin-core".to_string(),
            repo_name_detached: "bitcoin-detached-sigs".to_string(),
            poll_interval: Duration::from_secs(300),
            signer_name: String::new(),
            gpg_key_id: String::new(),
            guix_sigs_fork_url: String::new(),
            multi_package: false,
            guix_build_dir: guix_build_dir.clone(),
            guix_sigs_dir: guix_build_dir.join("guix.sigs"),
            bitcoin_detached_sigs_dir: guix_build_dir.join("bitcoin-detached-sigs"),
            macos_sdks_dir: guix_build_dir.join("macos-sdks"),
            bitcoin_dir: guix_build_dir.join("bitcoin"),
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "BGT Builder Configuration:")?;
        writeln!(f, "  Repository Owner: {}", self.repo_owner)?;
        writeln!(f, "  Repository Name: {}", self.repo_name)?;
        writeln!(
            f,
            "  Detached Repository Owner: {}",
            self.repo_owner_detached
        )?;
        writeln!(f, "  Detached Repository Name: {}", self.repo_name_detached)?;
        writeln!(f, "  Poll Interval: {:?}", self.poll_interval)?;
        writeln!(f, "  Signer Name: {}", self.signer_name)?;
        writeln!(f, "  GPG Key ID: {}", self.gpg_key_id)?;
        writeln!(f, "  Guix Sigs Fork URL: {}", self.guix_sigs_fork_url)?;
        writeln!(f, "  Multi-package: {}", self.multi_package)?;
        writeln!(f, "  Guix Build Directory: {:?}", self.guix_build_dir)?;
        writeln!(f, "  Guix Sigs Directory: {:?}", self.guix_sigs_dir)?;
        writeln!(
            f,
            "  Bitcoin Detached Sigs Directory: {:?}",
            self.bitcoin_detached_sigs_dir
        )?;
        writeln!(f, "  macOS SDKs Directory: {:?}", self.macos_sdks_dir)?;
        writeln!(f, "  Bitcoin Directory: {:?}", self.bitcoin_dir)?;
        Ok(())
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_file("config.toml");
        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
        let parsed_config: Table =
            toml::from_str(&config_str).context("Failed to parse config file")?;

        let mut config = Config {
            signer_name: parsed_config["signer_name"]
                .as_str()
                .context("signer_name not found in config")?
                .to_string(),
            gpg_key_id: parsed_config["gpg_key_id"]
                .as_str()
                .context("gpg_key_id not found in config")?
                .to_string(),
            guix_sigs_fork_url: parsed_config["guix_sigs_fork_url"]
                .as_str()
                .context("guix_sigs_fork_url not found in config")?
                .to_string(),
            ..Default::default()
        };

        if let Some(guix_build_dir) = parsed_config.get("guix_build_dir") {
            config.guix_build_dir = PathBuf::from(
                guix_build_dir
                    .as_str()
                    .context("guix_build_dir is not a string")?,
            );
            config.guix_sigs_dir = config.guix_build_dir.join("guix.sigs");
            config.bitcoin_detached_sigs_dir = config.guix_build_dir.join("bitcoin-detached-sigs");
            config.macos_sdks_dir = config.guix_build_dir.join("macos-sdks");
            config.bitcoin_dir = config.guix_build_dir.join("bitcoin");
        }

        Ok(config)
    }
}

pub(crate) fn get_config_file(file: &str) -> PathBuf {
    let mut path = config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("bgt");
    std::fs::create_dir_all(&path).expect("Failed to create config directory");
    path.push(file);
    path
}

pub(crate) fn read_config() -> Result<Config> {
    Config::load()
        .context("Failed to load config. Please run 'bgt setup' to set up your configuration.")
}
