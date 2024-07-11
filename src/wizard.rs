use anyhow::Result;
use std::io::{self, Write};
use toml::Table;

use crate::config::get_config_file;

pub(crate) async fn init_wizard() -> Result<()> {
    println!("Welcome to the bgt config wizard!");
    println!("Please provide the following information:");

    let mut config = Table::new();

    // Prompt for SIGNER gpg key
    let gpg_key = prompt_input("Enter your gpg key fingerprint")?;
    config.insert("GPG_KEY_ID".to_string(), toml::Value::String(gpg_key));
    // TODO: Check that gpg key can be found

    // Prompt for SIGNER name
    let signer_name = prompt_input("Enter your signer name")?;
    config.insert("SIGNER_NAME".to_string(), toml::Value::String(signer_name));

    // Prompt for GUIX_SIGS_FORK
    let guix_sigs_fork = prompt_input("Enter the URL of your guix.sigs fork")?;
    config.insert(
        "GUIX_SIGS_FORK".to_string(),
        toml::Value::String(guix_sigs_fork),
    );

    // Write config to file
    let config_path = get_config_file("config.toml");
    let config_str = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_str)?;

    println!("Configuration saved to: {}", config_path.display());
    println!("Initialization complete. You can now use the BGT Builder!");

    Ok(())
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
