use anyhow::{anyhow, Result};
use justrun::paths::CONFIG;
use log::info;
use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use yaml_rust2 as yaml;

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceState {
    Loaded,
    Running,
    Stopped,
    Failed,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceState::Loaded => write!(f, "Loaded"),
            ServiceState::Running => write!(f, "Running"),
            ServiceState::Stopped => write!(f, "Stopped"),
            ServiceState::Failed => write!(f, "Failed"),
        }
    }
}

pub struct Service {
    state: ServiceState,
    pid: Option<u32>,
    path: String,
    enabled: bool,
    name: String,
    as_root: bool,
    start: String,
    stop: Option<String>,
    logs: Arc<Mutex<Vec<(u64, String)>>>,
}

impl Service {
    pub fn new(config: yaml::Yaml, path: String) -> Result<Self> {
        let enabled = config["enabled"].as_bool().unwrap_or(true);
        let name = config["name"]
            .as_str()
            .ok_or_else(|| anyhow!("Service name is not specified in config file: {}", CONFIG))?
            .to_string();
        let as_root = config["as_root"].as_bool().unwrap_or(false);
        let start = config["start"]
            .as_str()
            .ok_or_else(|| {
                anyhow!(
                    "Service start command is not specified in config file: {}",
                    CONFIG
                )
            })?
            .to_string();
        let stop = match config["stop"].as_str() {
            Some(stop) => Some(stop.to_string()),
            None => None,
        };

        Ok(Self {
            state: ServiceState::Loaded,
            pid: None,
            path,
            enabled,
            name,
            as_root,
            start,
            stop,
            logs: Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&mut self) -> Result<()> {
        if !self.enabled || self.state == ServiceState::Running {
            return Ok(());
        }

        let command = if self.as_root {
            format!("sudo {}", self.start)
        } else {
            self.start.clone()
        };

        let parts = command.split_whitespace().collect::<Vec<_>>();
        let program = parts[0];
        let args = &parts[1..];

        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;

        self.pid = Some(child.id());
        self.state = ServiceState::Running;

        let logs = Arc::clone(&self.logs);

        if let Some(stdout) = child.stdout.take() {
            let logs = Arc::clone(&logs);
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stdout);
                for line in std::io::BufRead::lines(reader).flatten() {
                    let ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let mut logs = logs.lock().unwrap();
                    logs.push((ts, format!("stdout: {}", line)));
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let logs = Arc::clone(&logs);
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                for line in std::io::BufRead::lines(reader).flatten() {
                    let ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let mut logs = logs.lock().unwrap();
                    logs.push((ts, format!("stderr: {}", line)));
                }
            });
        }

        info!("Started service: {}", self.name);

        Ok(())
    }

    pub fn update(&mut self) {
        if !self.enabled || self.state == ServiceState::Stopped {
            return;
        }

        if let Some(pid) = self.pid {
            match Command::new("ps").arg("-p").arg(pid.to_string()).output() {
                Ok(output) => {
                    if output.status.success() {
                        self.state = ServiceState::Running;
                    } else {
                        self.state = ServiceState::Stopped;
                    }
                }
                Err(e) => {
                    self.state = ServiceState::Failed;
                    let ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let mut logs = self.logs.lock().unwrap();
                    logs.push((ts, format!("Failed to check process: {}", e)));
                }
            }
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.enabled || self.state == ServiceState::Stopped {
            return Ok(());
        }

        if let Some(pid) = self.pid {
            if let Some(stop_command) = &self.stop {
                let command = if self.as_root {
                    format!("sudo {}", stop_command)
                } else {
                    stop_command.clone()
                };
                let parts = command.split_whitespace().collect::<Vec<_>>();
                let program = parts[0];
                let args = &parts[1..];
                Command::new(program)
                    .args(args)
                    .spawn()
                    .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;
            }

            let _ = Command::new("kill")
                .arg("-SIGTERM")
                .arg(pid.to_string())
                .spawn()
                .map_err(|e| anyhow!("Failed to send SIGTERM: {}", e))?;
        }

        self.state = ServiceState::Stopped;
        self.pid = None;
        info!("Stopped service: {}", self.name);
        Ok(())
    }

    pub fn status(&self) {
        print!("Service: {}", self.name);
        if self.enabled {
            print!(" (enabled)");
        } else {
            print!(" (disabled)");
        }
        print!(" ({})", self.state);
        if self.as_root {
            print!(" (AS ROOT)");
        }
        if let Some(pid) = self.pid {
            print!(" (PID: {})", pid);
        }
        println!();
        println!("{} at {}", self.state, self.path);
        println!();
        for (log_time, log) in self.logs.lock().unwrap().iter() {
            let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(*log_time);
            let time_str = time
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string();
            println!("[{}] {}", time_str, log);
        }
    }
}
