use crate::cleanup;
use anyhow::{Result, anyhow};
use justrun::paths::PID;
use log::warn;

pub fn check_running() -> Result<()> {
    if std::path::Path::new(PID).exists() {
        let pid = std::fs::read_to_string(PID)
            .map_err(|e| anyhow!("Failed to read PID file: {}", e))?
            .trim()
            .parse::<u32>()
            .map_err(|e| anyhow!("Failed to parse PID: {}", e))?;

        let is_running_stdout = std::process::Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .output()
            .map_err(|e| anyhow!("Failed to check process: {}", e))?
            .stdout;
        let is_running = String::from_utf8(is_running_stdout)
            .map_err(|e| anyhow!("Failed to convert output to string: {}", e))?
            .lines()
            .any(|line| line.contains(&pid.to_string()));

        if is_running {
            return Err(anyhow!(
                "PID {} is active. Another instance is running, terminating.",
                pid
            ));
        } else {
            warn!(
                "PID {} is not active. Daemon was not gracefully terminated, cleaning up",
                pid
            );
            if !cleanup::cleanup() {
                return Err(anyhow!("Failed to clean up old daemon files"));
            }
        }
    }

    let pid = std::process::id();
    std::fs::create_dir_all("/tmp/justrund")
        .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
    std::fs::write(PID, pid.to_string()).map_err(|e| anyhow!("Failed to write PID file: {}", e))?;

    Ok(())
}
