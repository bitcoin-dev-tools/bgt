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

    let mut config = Config {
        gpg_key_id: prompt_input("Enter your gpg key fingerprint")?,
        signer_name: prompt_input("Enter your signer name")?,
        guix_sigs_fork_url: prompt_input("Enter the URL of your guix.sigs fork")?,
        guix_build_dir: PathBuf::from(prompt_input(&format!(
            "Enter the path you want to use for the guix_build_dir (press Enter for default of {:?})",
            default_guix_build_dir
        ))?),
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
