use anyhow::{Context, Result};
use daemonize::Daemonize;
use log::{error, info};
use std::fs::File;
use std::path::PathBuf;

pub fn start_daemon(pid_file: &PathBuf, log_file: &PathBuf) -> Result<()> {
    let stdout = File::create(log_file)
        .with_context(|| format!("Failed to create log file for stdout: {:?}", log_file))?;
    let stderr = File::create(log_file)
        .with_context(|| format!("Failed to create log file for stderr: {:?}", log_file))?;

    let daemonize = Daemonize::new()
        .pid_file(pid_file)
        .chown_pid_file(true)
        .working_directory(".")
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => {
            info!("Daemon started successfully.");
            Ok(())
        }
        Err(e) => {
            error!("Error starting daemon: {}", e);
            Err(e).context("Failed to start daemon")
        }
    }
}

pub fn stop_daemon(pid_file: &PathBuf) -> Result<()> {
    if pid_file.exists() {
        let pid = std::fs::read_to_string(pid_file)
            .with_context(|| format!("Failed to read PID from file: {:?}", pid_file))?
            .trim()
            .parse::<i32>()
            .context("Failed to parse PID as integer")?;

        unsafe {
            if libc::kill(pid, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error())
                    .context("Failed to send SIGKILL to daemon process");
            }
        }

        std::fs::remove_file(pid_file)
            .with_context(|| format!("Failed to remove PID file: {:?}", pid_file))?;

        println!("Daemon stopped successfully.");
    } else {
        println!("Daemon is not running (PID file not found).");
    }

    Ok(())
}
