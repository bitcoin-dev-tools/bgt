use anyhow::{Context, Result};
use dirs::state_dir;
use std::{
    io::{self, Write},
    path::PathBuf,
};

use crate::config::{get_config_file, Config};

pub(crate) async fn init_wizard() -> Result<()> {
    println!("Welcome to the bgt config wizard!");
    println!("Please provide the following information:");

    let state = state_dir().unwrap_or_else(|| PathBuf::from("."));
    let default_guix_build_dir = state.join("guix-builds");

    let gpg_key_id =
        prompt_input_with_validation("Enter your gpg key short id (e.g. 0xA1B2C3D4E5F6G7H8)", |input| {
            if input.starts_with("0x") {
                Ok(())
            } else {
                Err("GPG key short id must start with '0x'")
            }
        })
        .context("Failed to get valid GPG key short id")?;

    let signer_name =
        prompt_input("Enter your signer name").context("Failed to get signer name")?;

    let guix_sigs_fork_url =
        prompt_input_with_validation("Enter the URL of your guix.sigs fork", |input| {
            if input.starts_with("https://github.com") {
                Ok(())
            } else {
                Err("URL must start with 'https://github.com'")
            }
        })
        .context("Failed to get valid guix.sigs fork URL")?;

    let guix_build_dir = PathBuf::from(
        prompt_input(&format!(
            "Enter the path you want to use for the guix_build_dir (press Enter for default of {:?})",
            default_guix_build_dir
        ))
        .context("Failed to get guix build directory path")?,
    );

    let auto_open_prs = prompt_input_with_validation(
        "Would you like to automatically open PRs on GitHub? (yes/no)",
        |input| {
            let input = input.to_lowercase();
            if input == "yes" || input == "no" {
                Ok(())
            } else {
                Err("Please enter 'yes' or 'no'")
            }
        },
    )
    .context("Failed to get auto-open PRs preference")?
    .to_lowercase()
        == "yes";

    let (github_username, gh_token) = if auto_open_prs {
        let username =
            prompt_input("Enter your GitHub username").context("Failed to get GitHub username")?;
        let token =
            prompt_input("Enter your GitHub token (will be stored in config file unencrypted!)")
                .context("Failed to get GitHub token")?;
        (Some(username), Some(token))
    } else {
        (None, None)
    };

    let mut config = Config {
        gpg_key_id,
        signer_name,
        guix_sigs_fork_url,
        guix_build_dir,
        github_username,
        github_token: gh_token,
        ..Default::default()
    };

    // If the user didn't enter anything, use the default
    if config.guix_build_dir.as_os_str().is_empty() {
        config.guix_build_dir = default_guix_build_dir;
    }

    config.guix_sigs_dir = config.guix_build_dir.join("guix.sigs");
    config.bitcoin_detached_sigs_dir = config.guix_build_dir.join("bitcoin-detached-sigs");
    config.macos_sdks_dir = config.guix_build_dir.join("macos-sdks");
    config.bitcoin_dir = config.guix_build_dir.join("bitcoin");

    // Write config to file
    let config_path = get_config_file("config.toml");
    let config_str =
        toml::to_string_pretty(&config).context("Failed to serialize config to TOML")?;
    std::fs::write(&config_path, config_str)
        .with_context(|| format!("Failed to write config to file: {:?}", config_path))?;

    println!("Configuration saved to: {}", config_path.display());
    Ok(())
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush().context("Failed to flush stdout")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;
    Ok(input.trim().to_string())
}

fn prompt_input_with_validation<F>(prompt: &str, validator: F) -> Result<String>
where
    F: Fn(&str) -> Result<(), &'static str>,
{
    loop {
        let input = prompt_input(prompt).context("Failed to get user input")?;
        match validator(&input) {
            Ok(()) => return Ok(input),
            Err(error_message) => {
                println!("Error: {}. Please try again.", error_message);
            }
        }
    }
}
