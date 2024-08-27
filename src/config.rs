use anyhow::{Context, Result};
use dirs::{config_dir, state_dir};
use std::fmt;
use std::{path::PathBuf, time::Duration};

pub static GH_TOKEN_NAME: &str = "GH_API_TOKEN";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub source_repo_owner: String,
    pub source_repo_name: String,
    pub guix_sigs_repo_owner: String,
    pub guix_sigs_repo_name: String,
    pub detached_repo_owner: String,
    pub detached_repo_name: String,
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
    pub github_username: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let state = state_dir().unwrap_or_else(|| PathBuf::from("."));
        let guix_build_dir = state.join("guix-builds");
        Self {
            source_repo_owner: "bitcoin".to_string(),
            source_repo_name: "bitcoin".to_string(),
            guix_sigs_repo_owner: "bitcoin-core".to_string(),
            guix_sigs_repo_name: "guix.sigs".to_string(),
            detached_repo_owner: "bitcoin-core".to_string(),
            detached_repo_name: "bitcoin-detached-sigs".to_string(),
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
            github_username: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_file("config.toml");
        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config: Config = toml::from_str(&config_str).context("Failed to parse config file")?;

        Ok(config)
    }

    pub fn get_github_token(&self) -> Option<String> {
        std::env::var(GH_TOKEN_NAME).ok()
    }
}

#[rustfmt::skip]
impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "BGT Builder Configuration:")?;
        writeln!(f, "{:<32} {}/{}", "Source Repo:", self.source_repo_owner, self.source_repo_name)?;
        writeln!(f, "{:<32} {}/{}", "Guix sigs repo:", self.guix_sigs_repo_owner, self.guix_sigs_repo_name)?;
        writeln!(f, "{:<32} {}/{}", "Detached sigs repo:", self.detached_repo_owner, self.detached_repo_name)?;
        writeln!(f, "{:<32} {:?}",  "Poll Interval:", self.poll_interval)?;
        writeln!(f, "{:<32} {}",    "Signer Name:", self.signer_name)?;
        writeln!(f, "{:<32} {}",    "GPG Key Short ID:", self.gpg_key_id)?;
        writeln!(f, "{:<32} {}",    "Guix Sigs Fork URL:", self.guix_sigs_fork_url)?;
        writeln!(f, "{:<32} {}",    "Multi-package:", self.multi_package)?;
        writeln!(f, "{:<32} {:?}",  "Guix Build Directory:", self.guix_build_dir)?;
        writeln!(f, "{:<32} {:?}",  "Guix Sigs Directory:", self.guix_sigs_dir)?;
        writeln!(f, "{:<32} {:?}",  "Bitcoin Detached Sigs Directory:", self.bitcoin_detached_sigs_dir)?;
        writeln!(f, "{:<32} {:?}",  "macOS SDKs Directory:", self.macos_sdks_dir)?;
        writeln!(f, "{:<32} {:?}",  "Bitcoin Directory:", self.bitcoin_dir)?;
        writeln!(f, "{:<32} {}",    "GitHub Username:", self.github_username.as_deref().unwrap_or("None"))?;
        writeln!(f, "{:<32} {}",    "GitHub Token:", if self.get_github_token().is_some() { "[set in environment]" } else { "Not set" })?;
        Ok(())
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
