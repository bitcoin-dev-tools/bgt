use anyhow::{Context, Result};
use dirs::state_dir;
use flate2::read::GzDecoder;
use log::warn;
use log::{debug, info};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tar::Archive;
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::xor::xor_decrypt;

use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug)]
pub enum BuildAction {
    Setup,
    Build,
    NonCodeSigned,
    CodeSigned,
    Clean,
}

pub struct Builder {
    config: Config,
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
        writeln!(f, "  signer_name: {}", self.config.signer_name)?;
        writeln!(f, "  gpg_key_id: {}", self.config.gpg_key_id)?;
        writeln!(f, "  version: {}", self.version)?;
        writeln!(f, "  action: {:?}", self.action)?;
        writeln!(f, "  guix_build_dir: {:?}", self.guix_build_dir)?;
        writeln!(f, "  guix_sigs_dir: {:?}", self.guix_sigs_dir)?;
        writeln!(
            f,
            "  guix_sigs_fork_url: {:?}",
            self.config.guix_sigs_fork_url
        )?;
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
    pub fn new(version: String, action: BuildAction, config: Config) -> Result<Self> {
        let state = state_dir().context("Failed to get a state dir")?;
        let guix_build_dir = PathBuf::from(&state).join("guix-builds");
        let bitcoin_dir = guix_build_dir.join("bitcoin");
        let guix_sigs_dir = guix_build_dir.join("guix.sigs");
        let bitcoin_detached_sigs_dir = guix_build_dir.join("bitcoin-detached-sigs");
        let macos_sdks_dir = guix_build_dir.join("macos-sdks");

        Ok(Self {
            config,
            version,
            action,
            guix_build_dir,
            guix_sigs_dir,
            bitcoin_detached_sigs_dir,
            macos_sdks_dir,
            bitcoin_dir,
        })
    }

    pub async fn init(&self) -> Result<()> {
        // Create guix_build_dir if it doesn't exist
        if !self.guix_build_dir.exists() {
            info!("Creating guix_build_dir: {:?}", self.guix_build_dir);
            fs::create_dir_all(&self.guix_build_dir).context("Failed to create guix_build_dir")?;
        }

        // Clone bitcoin/bitcoin if it doesn't exist
        if !self.bitcoin_dir.exists() {
            info!("Cloning bitcoin repository");
            self.run_command(
                &self.guix_build_dir,
                "git",
                &[
                    "clone",
                    "--depth",
                    "1",
                    "https://github.com/bitcoin/bitcoin",
                    self.bitcoin_dir.file_name().unwrap().to_str().unwrap(),
                ],
            )?;
        }

        // Clone bitcoin-detached-sigs if it doesn't exist
        if !self.bitcoin_detached_sigs_dir.exists() {
            info!("Cloning bitcoin-detached-sigs repository");
            self.run_command(
                &self.guix_build_dir,
                "git",
                &[
                    "clone",
                    "https://github.com/bitcoin-core/bitcoin-detached-sigs",
                    self.bitcoin_detached_sigs_dir
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                ],
            )?;
        }

        // Create macos_sdks_dir if it doesn't exist
        if !self.macos_sdks_dir.exists() {
            info!("Creating macos_sdks_dir: {:?}", self.macos_sdks_dir);
            fs::create_dir_all(&self.macos_sdks_dir).context("Failed to create macos_sdks_dir")?;
        }

        // Clone guix.sigs if it doesn't exist
        if !self.guix_sigs_dir.exists() {
            info!("Cloning guix.sigs repository");
            self.run_command(
                &self.guix_build_dir,
                "git",
                &[
                    "clone",
                    "--origin",
                    "upstream",
                    "https://github.com/bitcoin-core/guix.sigs.git",
                    self.guix_sigs_dir.file_name().unwrap().to_str().unwrap(),
                ],
            )?;

            // Set the origin remote
            self.run_command(
                &self.guix_sigs_dir,
                "git",
                &["remote", "add", "origin", &self.config.guix_sigs_fork_url],
            )?;

            info!(
                "Set origin remote of the guix sigs repo to: {}",
                &self.config.guix_sigs_fork_url
            );
        }

        Ok(())
    }

    async fn check_sdk(&self) -> Result<()> {
        let mut sdks = HashMap::new();
        // xor module contains the equivalent encryption function
        sdks.insert("v25.2", "26,36,59,38,16,68,93,86,75,64,31,1,0,118,118,114,54,111,16,17,24,22,4,17,70,85,86,25,17,3,31,111,2,0,24,12,72,30,91,82,81,76,58,106,60,39,20,13,9,22,22");
        sdks.insert("v26.2", "26,36,59,38,16,68,93,86,75,64,31,1,0,118,118,114,54,111,16,17,24,22,4,17,70,85,86,25,17,3,31,111,2,0,24,12,72,30,91,82,81,76,58,106,60,39,20,13,9,22,22");
        sdks.insert("v27.1", "26,36,59,38,16,68,93,81,75,66,31,1,7,117,112,115,100,38,88,12,20,16,23,19,81,68,87,80,111,20,16,9,88,30,5,16,13,95,94,89,80,87,58,63,121,42,16,8,8,1,23,1");

        let sdk_name_encrypted = sdks.get(&self.version as &str).ok_or_else(|| {
            anyhow::anyhow!(
                "Unsupported version when matching to sdks: {}",
                self.version
            )
        })?;

        let sdk_name = xor_decrypt(sdk_name_encrypted);
        debug!("Using sdk name: {:?}", sdk_name);
        let sdk_path = self.macos_sdks_dir.join(&sdk_name);
        debug!("Using sdk path: {:?}", sdk_path);

        if !sdk_path.exists() {
            info!("SDK not found. Downloading and extracting...");
            self.download_and_extract_sdk(&sdk_name).await?;
        } else {
            info!("SDK found: {:?}", sdk_path);
        }

        Ok(())
    }

    async fn download_and_extract_sdk(&self, sdk_name: &str) -> Result<()> {
        let base_url_encrypted = "42,51,32,50,6,83,67,75,7,27,70,83,93,93,44,36,59,48,16,71,3,22,2,93,86,85,66,81,44,35,39,111,6,6,25,22,6,23,65,31,65,80,41,52,123";
        let base_url = xor_decrypt(base_url_encrypted);

        let url = format!("{}{}.tar.gz", base_url, sdk_name);
        let tar_gz_path = self.macos_sdks_dir.join(format!("{}.tar.gz", sdk_name));
        debug!("Using tar.gz path: {:?}", tar_gz_path);

        info!("Downloading SDK {}", sdk_name);
        let response = reqwest::get(&url).await?;
        let bytes = response.bytes().await?;

        // Write the file
        let mut file = tokio::fs::File::create(&tar_gz_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        // Extract the SDK (this part remains synchronous)
        info!("Extracting SDK");
        let tar_gz = std::fs::File::open(&tar_gz_path)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive.unpack(&self.macos_sdks_dir)?;

        // Remove the tar.gz file
        tokio::fs::remove_file(&tar_gz_path).await?;

        info!("SDK downloaded and extracted successfully");
        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.refresh_repos()?;
        self.checkout_bitcoin()?;
        match self.action {
            BuildAction::Setup => {}
            BuildAction::Build => {
                self.check_sdk().await?;
                self.guix_build()?;
            }
            BuildAction::NonCodeSigned => self.guix_attest("non-codesigned")?,
            BuildAction::CodeSigned => {
                self.guix_codesign()?;
                self.guix_attest("codesigned")?;
            }
            BuildAction::Clean => self.guix_clean()?,
        }
        Ok(())
    }

    fn checkout_bitcoin(&self) -> Result<()> {
        info!("Checking out Bitcoin version {}", self.version);

        // Fetch the tag
        let mut command = Command::new("git");
        command
            .current_dir(&self.bitcoin_dir)
            .args([
                "fetch",
                "origin",
                "tag",
                &self.version,
                "--no-tags",
                "--depth=1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command_with_output(command)?;

        // Checkout the version
        let mut command = Command::new("git");
        command
            .current_dir(&self.bitcoin_dir)
            .args(["checkout", &self.version])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command_with_output(command)?;

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
            &["checkout", "master"],
        )?;
        self.run_command(
            &self.guix_build_dir.join("bitcoin-detached-sigs"),
            "git",
            &["pull", "origin", "master"],
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
            .env("SDK_PATH", self.guix_build_dir.join("macos-sdks"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.config.multi_package {
            command
                .env("JOBS", "1")
                .env("ADDITIONAL_GUIX_COMMON_FLAGS", "--max-jobs=8");
        }

        self.run_command_with_output(command)?;
        Ok(())
    }

    fn guix_attest(&self, a_type: &str) -> Result<()> {
        info!("Attesting {} binaries", a_type);
        let mut command = Command::new(self.bitcoin_dir.join("contrib/guix/guix-attest"));
        command
            .current_dir(&self.bitcoin_dir)
            .env(
                "GUIX_SIGS_REPO",
                self.guix_build_dir.join("guix.sigs").to_str().unwrap(),
            )
            // SIGNER=0x96AB007F1A7ED999=dongcarl
            .env(
                "SIGNER",
                format!(
                    "{}={}",
                    self.config.gpg_key_id,
                    self.config.signer_name.as_str()
                ),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_with_output(command)?;
        self.commit_attestations(a_type)?;
        Ok(())
    }

    fn guix_codesign(&self) -> Result<()> {
        info!("Codesigning binaries");
        let mut command = Command::new(self.bitcoin_dir.join("contrib/guix/guix-codesign"));
        command
            .current_dir(&self.bitcoin_dir)
            .env(
                "DETACHED_SIGS_REPO",
                self.guix_build_dir
                    .join("bitcoin-detached-sigs")
                    .to_str()
                    .unwrap(),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_with_output(command)?;
        Ok(())
    }

    fn guix_clean(&self) -> Result<()> {
        info!("Running guix-clean");
        let mut command = Command::new(self.bitcoin_dir.join("contrib/guix/guix-clean"));
        command
            .current_dir(&self.bitcoin_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_with_output(command)?;
        Ok(())
    }

    fn commit_attestations(&self, attestation_type: &str) -> Result<()> {
        info!("Committing attestations");
        let branch_name = format!(
            "{}-{}-{}-attestations",
            self.config.signer_name, self.version, attestation_type
        );
        let commit_message = format!(
            "Add {} attestations by {} for {}",
            attestation_type, self.config.signer_name, self.version
        );

        // Create new branch
        let mut command = Command::new("git");
        command
            .current_dir(&self.guix_build_dir.join("guix.sigs"))
            .args(["checkout", "-b", &branch_name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command_with_output(command)?;

        // Add files
        let add_files = if attestation_type == "all" {
            vec![
                format!(
                    "{}/{}/all.SHA256SUMS",
                    &self.version[1..],
                    &self.config.signer_name
                ),
                format!(
                    "{}/{}/all.SHA256SUMS.asc",
                    &self.version[1..],
                    &self.config.signer_name
                ),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS",
                    &self.version[1..],
                    &self.config.signer_name
                ),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..],
                    &self.config.signer_name
                ),
            ]
        } else {
            vec![
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS",
                    &self.version[1..],
                    &self.config.signer_name
                ),
                format!(
                    "{}/{}/noncodesigned.SHA256SUMS.asc",
                    &self.version[1..],
                    &self.config.signer_name
                ),
            ]
        };

        let mut git_add_args = vec!["add"];
        git_add_args.extend(add_files.iter().map(String::as_str));

        let mut command = Command::new("git");
        command
            .current_dir(&self.guix_build_dir.join("guix.sigs"))
            .args(&git_add_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command_with_output(command)?;

        // Commit changes
        let mut command = Command::new("git");
        command
            .current_dir(&self.guix_build_dir.join("guix.sigs"))
            .args(["commit", "-m", &commit_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command_with_output(command)?;
        warn!(
            r#"Must manually push to GitHub and open PR.
To push the changes, run the following commands:
    cd {:?}
    git push origin"#,
            &self.guix_sigs_dir
        );

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

    fn run_command_with_output(&self, mut command: Command) -> Result<()> {
        let mut child = command
            .spawn()
            .with_context(|| format!("Failed to execute command: {:?}", command))?;

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        // Spawn a thread to handle stdout
        let stdout_handle = std::thread::spawn(move || {
            stdout_reader.lines().for_each(|line| {
                if let Ok(line) = line {
                    println!("{}", line);
                }
            });
        });

        // Spawn a thread to handle stderr
        let stderr_handle = std::thread::spawn(move || {
            stderr_reader.lines().for_each(|line| {
                if let Ok(line) = line {
                    eprintln!("{}", line);
                }
            });
        });

        // Wait for the command to finish
        let status = child.wait()?;

        // Wait for the output threads to finish
        stdout_handle.join().expect("Stdout thread panicked");
        stderr_handle.join().expect("Stderr thread panicked");

        if !status.success() {
            return Err(anyhow::anyhow!("Command failed: {:?}", command));
        }

        Ok(())
    }
}
