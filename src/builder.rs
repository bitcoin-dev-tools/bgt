use anyhow::{Context, Result};
use log::{error, info};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub struct Builder {
    signer: String,
    version: String,
    action: BuildAction,
    guix_build_dir: PathBuf,
    bitcoin_dir: PathBuf,
}

pub enum BuildAction {
    Build,
    NonCodeSigned,
    All,
}

impl Builder {
    pub fn new(signer: String, version: String, action: BuildAction) -> Result<Self> {
        let home = env::var("HOME").context("Failed to get HOME environment variable")?;
        let guix_build_dir = PathBuf::from(&home).join("guix-builds");
        let bitcoin_dir = PathBuf::from(&home).join("bitcoin");

        Ok(Self {
            signer,
            version,
            action,
            guix_build_dir,
            bitcoin_dir,
        })
    }

    pub fn run(&self) -> Result<()> {
        self.checkout_bitcoin()?;
        self.refresh_repos()?;
        self.build()?;

        match self.action {
            BuildAction::Build => {}
            BuildAction::NonCodeSigned => self.attest_noncodesigned()?,
            BuildAction::All => {
                self.codesign()?;
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
        info!("Refreshing repositories");
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
            &["fetch", "origin"],
        )?;
        Ok(())
    }

    fn build(&self) -> Result<()> {
        info!("Starting build process");
        let mut command = Command::new(&self.bitcoin_dir.join("contrib/guix/guix-build"));
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

    fn attest_noncodesigned(&self) -> Result<()> {
        info!("Attesting non-codesigned binaries");
        self.run_command_with_env(
            &self.bitcoin_dir,
            "contrib/guix/guix-attest",
            &[],
            &[
                ("GUIX_SIGS_REPO", self.guix_build_dir.join("guix.sigs")),
                ("SIGNER", self.signer.as_str()),
            ],
        )?;
        self.commit_attestations("noncodesigned")?;
        Ok(())
    }

    fn codesign(&self) -> Result<()> {
        info!("Codesigning binaries");
        self.run_command_with_env(
            &self.bitcoin_dir,
            "contrib/guix/guix-codesign",
            &[],
            &[(
                "DETACHED_SIGS_REPO",
                self.guix_build_dir.join("bitcoin-detached-sigs"),
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
                ("GUIX_SIGS_REPO", self.guix_build_dir.join("guix.sigs")),
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
                format!("{}/$SIGNER/all.SHA256SUMS", &self.version[1..]),
                format!("{}/$SIGNER/all.SHA256SUMS.asc", &self.version[1..]),
                format!("{}/$SIGNER/noncodesigned.SHA256SUMS", &self.version[1..]),
                format!(
                    "{}/$SIGNER/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..]
                ),
            ]
        } else {
            vec![
                format!("{}/$SIGNER/noncodesigned.SHA256SUMS", &self.version[1..]),
                format!(
                    "{}/$SIGNER/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..]
                ),
            ]
        };

        self.run_command(
            &self.guix_build_dir.join("guix.sigs"),
            "git",
            &["add"]
                .iter()
                .chain(add_files.iter())
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
        )?;

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
        env: &[(&str, impl AsRef<std::ffi::OsStr>)],
    ) -> Result<()> {
        let status = Command::new(command)
            .current_dir(dir)
            .args(args)
            .envs(env.iter().cloned())
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
