use anyhow::{Context, Result};
use dirs::state_dir;
use log::{error, info};
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub enum BuildAction {
    Build,
    NonCodeSigned,
    CodeSigned,
    All,
}

pub struct Builder {
    pub signer: String,
    version: String,
    action: BuildAction,
    guix_build_dir: PathBuf,
    guix_sigs_dir: PathBuf,
    bitcoin_detached_sigs_dir: PathBuf,
    macos_sdks_dir: PathBuf,
    bitcoin_dir: PathBuf,
}

impl fmt::Display for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Builder {{")?;
        writeln!(f, "  signer: {}", self.signer)?;
        writeln!(f, "  version: {}", self.version)?;
        writeln!(f, "  action: {:?}", self.action)?;
        writeln!(f, "  guix_build_dir: {:?}", self.guix_build_dir)?;
        writeln!(f, "  guix_sigs_dir: {:?}", self.guix_sigs_dir)?;
        writeln!(
            f,
            "  bitcoin_detached_sigs_dir: {:?}",
            self.bitcoin_detached_sigs_dir
        )?;
        writeln!(f, "  macos_sdks_dir: {:?}", self.macos_sdks_dir)?;
        writeln!(f, "  bitcoin_dir: {:?}", self.bitcoin_dir)?;
        writeln!(f, "}}")
    }
}

impl Builder {
    pub fn new(signer: String, version: String, action: BuildAction) -> Result<Self> {
        let state = state_dir().context("Failed to get a state dir")?;
        let guix_build_dir = PathBuf::from(&state).join("guix-builds");
        let bitcoin_dir = PathBuf::from(
            env::var("BITCOIN_SOURCE_DIR")
                .expect("Failed to get BITCOIN_SOURCE_DIR environment variable"),
        );
        let guix_sigs_dir = guix_build_dir.join("guix.sigs");
        let bitcoin_detached_sigs_dir = guix_build_dir.join("bitcoin-detached-sigs");
        let macos_sdks_dir = guix_build_dir.join("macos-sdks");

        Ok(Self {
            signer,
            version,
            action,
            guix_build_dir,
            guix_sigs_dir,
            bitcoin_detached_sigs_dir,
            macos_sdks_dir,
            bitcoin_dir,
        })
    }

    pub fn run(&self) -> Result<()> {
        self.checkout_bitcoin()?;
        self.refresh_repos()?;

        match self.action {
            BuildAction::Build => self.guix_build()?,
            BuildAction::NonCodeSigned => self.guix_attest()?,
            BuildAction::CodeSigned => self.guix_codesign()?,
            BuildAction::All => {
                self.guix_build()?;
                self.guix_attest()?;
                self.guix_codesign()?;
                self.attest_all()?;
            }
        }

        Ok(())
    }

    fn checkout_bitcoin(&self) -> Result<()> {
        info!("Checking out Bitcoin version {}", self.version);
        self.run_command(&self.bitcoin_dir, "git", &["fetch", "upstream"])?;
        self.run_command(&self.bitcoin_dir, "git", &["checkout", &self.version])?;
        Ok(())
    }

    fn refresh_repos(&self) -> Result<()> {
        info!("Refreshing guix.sigs and bitcoin-detached-sigs repos");
        self.run_command(
            &self.guix_build_dir.join("guix.sigs"),
            "git",
            &["checkout", "main"],
        )?;
        self.run_command(
            &self.guix_build_dir.join("guix.sigs"),
            "git",
            &["pull", "upstream", "main"],
        )?;
        self.run_command(
            &self.guix_build_dir.join("bitcoin-detached-sigs"),
            "git",
            &["checkout", "main"],
        )?;
        self.run_command(
            &self.guix_build_dir.join("bitcoin-detached-sigs"),
            "git",
            &["pull", "upstream", "main"],
        )?;
        Ok(())
    }

    fn guix_build(&self) -> Result<()> {
        info!("Starting build process");
        let mut command = Command::new(self.bitcoin_dir.join("contrib/guix/guix-build"));
        command
            .current_dir(&self.bitcoin_dir)
            .env(
                "SOURCES_PATH",
                self.guix_build_dir.join("depends-sources-cache"),
            )
            .env("BASE_CACHE", self.guix_build_dir.join("depends-base-cache"))
            .env("SDK_PATH", self.guix_build_dir.join("macos-sdks"));

        self.run_command_with_output(command)?;
        Ok(())
    }

    fn guix_attest(&self) -> Result<()> {
        info!("Attesting non-codesigned binaries");
        self.run_command_with_env(
            &self.bitcoin_dir,
            "contrib/guix/guix-attest",
            &[],
            &[
                (
                    "GUIX_SIGS_REPO",
                    self.guix_build_dir.join("guix.sigs").to_str().unwrap(),
                ),
                ("SIGNER", self.signer.as_str()),
            ],
        )?;
        self.commit_attestations("noncodesigned")?;
        Ok(())
    }

    fn guix_codesign(&self) -> Result<()> {
        info!("Codesigning binaries");
        self.run_command_with_env(
            &self.bitcoin_dir,
            "contrib/guix/guix-codesign",
            &[],
            &[(
                "DETACHED_SIGS_REPO",
                self.guix_build_dir
                    .join("bitcoin-detached-sigs")
                    .to_str()
                    .unwrap(),
            )],
        )?;
        Ok(())
    }

    fn attest_all(&self) -> Result<()> {
        info!("Attesting all binaries");
        self.run_command_with_env(
            &self.bitcoin_dir,
            "contrib/guix/guix-attest",
            &[],
            &[
                (
                    "GUIX_SIGS_REPO",
                    self.guix_build_dir.join("guix.sigs").to_str().unwrap(),
                ),
                ("SIGNER", self.signer.as_str()),
            ],
        )?;
        self.commit_attestations("all")?;
        Ok(())
    }

    fn commit_attestations(&self, attestation_type: &str) -> Result<()> {
        info!("Committing attestations");
        let branch_name = format!(
            "{}-{}-{}-attestations",
            self.signer, self.version, attestation_type
        );
        let commit_message = format!(
            "Add {} attestations by {} for {}",
            attestation_type, self.signer, self.version
        );

        self.run_command(
            &self.guix_build_dir.join("guix.sigs"),
            "git",
            &["checkout", "-b", &branch_name],
        )?;

        let add_files = if attestation_type == "all" {
            vec![
                format!("{}/{}/all.SHA256SUMS", &self.version[1..], &self.signer),
                format!("{}/{}/all.SHA256SUMS.asc", &self.version[1..], &self.signer),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS",
                    &self.version[1..],
                    &self.signer
                ),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..],
                    &self.signer
                ),
            ]
        } else {
            vec![
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS",
                    &self.version[1..],
                    &self.signer
                ),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..],
                    &self.signer
                ),
            ]
        };

        let mut git_add_args = vec!["add"];
        git_add_args.extend(add_files.iter().map(String::as_str));

        self.run_command(&self.guix_build_dir.join("guix.sigs"), "git", &git_add_args)?;

        self.run_command(
            &self.guix_build_dir.join("guix.sigs"),
            "git",
            &["commit", "-m", &commit_message],
        )?;

        Ok(())
    }

    fn run_command(&self, dir: &PathBuf, command: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(command)
            .current_dir(dir)
            .args(args)
            .status()
            .with_context(|| format!("Failed to execute command: {} {:?}", command, args))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Command failed: {} {:?}", command, args));
        }
        Ok(())
    }

    fn run_command_with_env(
        &self,
        dir: &PathBuf,
        command: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<()> {
        let status = Command::new(command)
            .current_dir(dir)
            .args(args)
            .envs(env.iter().map(|(k, v)| (k, v.to_string())))
            .status()
            .with_context(|| format!("Failed to execute command: {} {:?}", command, args))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Command failed: {} {:?}", command, args));
        }
        Ok(())
    }

    fn run_command_with_output(&self, mut command: Command) -> Result<()> {
        let output = command
            .output()
            .with_context(|| format!("Failed to execute command: {:?}", command))?;

        if !output.status.success() {
            error!("Command failed: {:?}", command);
            error!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("Command failed: {:?}", command));
        }

        info!(
            "Command output: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        Ok(())
    }
}
