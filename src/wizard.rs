use anyhow::Result;
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
        prompt_input_with_validation("Enter your gpg key id (e.g. 0xA1B2C3D4E5F6G7H8)", |input| {
            if input.starts_with("0x") {
                Ok(())
            } else {
                Err("GPG key id must start with '0x'")
            }
        })?;

    let signer_name = prompt_input("Enter your signer name")?;

    let guix_sigs_fork_url =
        prompt_input_with_validation("Enter the URL of your guix.sigs fork", |input| {
            if input.starts_with("https://github.com") {
                Ok(())
            } else {
                Err("URL must start with 'https://github.com'")
            }
        })?;

    let guix_build_dir = PathBuf::from(prompt_input(&format!(
        "Enter the path you want to use for the guix_build_dir (press Enter for default of {:?})",
        default_guix_build_dir
    ))?);

    let mut config = Config {
        gpg_key_id,
        signer_name,
        guix_sigs_fork_url,
        guix_build_dir,
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
    let config_str = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_str)?;

    println!("Configuration saved to: {}", config_path.display());
    println!("Initialization complete. You can now use bgt builder!");

    Ok(())
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_input_with_validation<F>(prompt: &str, validator: F) -> Result<String>
where
    F: Fn(&str) -> Result<(), &'static str>,
{
    loop {
        let input = prompt_input(prompt)?;
        match validator(&input) {
            Ok(()) => return Ok(input),
            Err(error_message) => {
                println!("Error: {}. Please try again.", error_message);
            }
        }
    }
}
