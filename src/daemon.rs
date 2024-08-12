use anyhow::Result;
use daemonize::Daemonize;
use log::{error, info};
use std::fs::File;
use std::path::PathBuf;

pub fn start_daemon(pid_file: &PathBuf, log_file: &PathBuf) -> Result<()> {
    let stdout = File::create(log_file)?;
    let stderr = File::create(log_file)?;

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
            Err(e.into())
        }
    }
}

pub fn stop_daemon(pid_file: &PathBuf) -> Result<()> {
    if pid_file.exists() {
        let pid = std::fs::read_to_string(pid_file)?.trim().parse::<i32>()?;
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        std::fs::remove_file(pid_file)?;

        println!("Daemon stopped successfully.");
    } else {
        println!("Daemon is not running (PID file not found).");
    }

    Ok(())
}
