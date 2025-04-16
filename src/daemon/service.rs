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
    dir: String,
    enabled: bool,
    name: String,
    max_retries: u32,
    retries: Arc<Mutex<u32>>,
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
        let max_retries = config["max_retries"].as_i64().unwrap_or(3) as u32;
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
        let parent_path = std::path::Path::new(&path)
            .parent()
            .ok_or_else(|| anyhow!("Failed to get parent directory of path: {}", path))?
            .to_str()
            .ok_or_else(|| anyhow!("Failed to convert path to string: {}", path))?
            .to_string();

        Ok(Self {
            state: ServiceState::Loaded,
            pid: None,
            path,
            dir: parent_path,
            enabled,
            name,
            as_root,
            retries: Arc::new(Mutex::new(0)),
            max_retries,
            start,
            stop,
            logs: Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&mut self) -> Result<String> {
        if !self.enabled || self.state == ServiceState::Running {
            return Ok("Already running".to_string());
        }

        let command = if self.as_root {
            format!("sudo {}", self.start)
        } else {
            self.start.clone()
        };

        let parts = command.split_whitespace().collect::<Vec<_>>();
        let program = parts[0];
        let args = &parts[1..];

        let mut child = match Command::new(program)
            .args(args)
            .current_dir(&self.dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                self.state = ServiceState::Failed;
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let mut logs = self.logs.lock().unwrap();
                logs.push((ts, format!("Failed to start process: {}", e)));
                return Ok(format!("Failed to start process: {}", e));
            }
        };

        {
            let mut retries = self.retries.lock().unwrap();
            *retries = 0;
        }

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

        {
            let logs = self.logs.clone();
            let start_cmd = self.start.clone();
            let as_root = self.as_root;
            let logs = Arc::clone(&logs);
            let retries = self.retries.clone();
            let max_retries = self.max_retries;
            let dir = self.dir.clone();
            std::thread::spawn(move || loop {
                match child.wait() {
                    Ok(status) => {
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let code = status.code().unwrap_or(-1);
                        {
                            let mut logs = logs.lock().unwrap();
                            logs.push((ts, format!("Process exited with code {}", code)));
                        }

                        if status.success() {
                            break;
                        }

                        {
                            let mut retries = retries.lock().unwrap();
                            *retries += 1;
                            let mut logs = logs.lock().unwrap();
                            logs.push((ts, format!("Restarting attempt #{}", retries)));
                        }

                        let retries = {
                            let retries = retries.lock().unwrap();
                            *retries
                        };

                        if retries >= max_retries {
                            {
                                let ts = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let mut logs = logs.lock().unwrap();
                                logs.push((ts, "Max retries reached, stopping".to_string()));
                            }
                            break;
                        }

                        std::thread::sleep(std::time::Duration::from_secs(1));

                        let command = if as_root {
                            format!("sudo {}", start_cmd)
                        } else {
                            start_cmd.clone()
                        };
                        let parts = command.split_whitespace().collect::<Vec<_>>();
                        let program = parts[0];
                        let args = &parts[1..];

                        let child = Command::new(program)
                            .args(args)
                            .current_dir(dir.clone())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn();
                        let mut child = match child {
                            Ok(new_child) => {
                                let ts = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let mut logs = logs.lock().unwrap();
                                logs.push((ts, "Restarted process".to_string()));
                                new_child
                            }
                            Err(e) => {
                                {
                                    let ts = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs();
                                    let mut logs = logs.lock().unwrap();
                                    logs.push((ts, format!("Failed to restart: {}", e)));
                                }
                                break;
                            }
                        };

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
                    }
                    Err(e) => {
                        {
                            let ts = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            let mut logs = logs.lock().unwrap();
                            logs.push((ts, format!("Failed to wait for child: {}", e)));
                        }
                        break;
                    }
                }
            });
        }

        {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut logs = self.logs.lock().unwrap();
            logs.push((ts, "Started".to_string()));
        }
        info!("Started service: {}", self.name);

        Ok("Started successfully".to_string())
    }

    pub fn stop(&mut self) -> Result<String> {
        if !self.enabled || self.state == ServiceState::Stopped {
            return Ok("Already stopped".to_string());
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

        {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut logs = self.logs.lock().unwrap();
            logs.push((ts, "Stopped".to_string()));
        }

        self.state = ServiceState::Stopped;
        self.pid = None;
        info!("Stopped service: {}", self.name);

        Ok("Stopped successfully".to_string())
    }

    pub fn restart(&mut self) -> Result<String> {
        let mut result = String::new();
        match self.stop() {
            Ok(_) => {
                result.push_str("Stopped successfully. ");
            }
            Err(e) => {
                return Err(anyhow!("Failed to stop: {}", e));
            }
        }
        match self.start() {
            Ok(_) => {
                result.push_str("Started successfully.");
            }
            Err(e) => {
                return Err(anyhow!("Failed to start: {}", e));
            }
        }

        Ok(result)
    }

    pub fn status(&self) -> String {
        let mut result = String::new();
        result += &format!("Service: {}", self.name);
        if self.enabled {
            result += " (enabled)";
        } else {
            result += " (disabled)";
        }
        result += &format!(" ({})", self.state);
        if self.as_root {
            result += " (AS ROOT)";
        }
        if let Some(pid) = self.pid {
            result += &format!(" (PID: {})", pid);
        }
        result += "\n";
        result += &format!("{} at {}", self.state, self.path);
        result += "\n\n";
        for (log_time, log) in self.logs.lock().unwrap().iter() {
            let epoch = *log_time as i64;
            let naive = chrono::DateTime::from_timestamp(epoch, 0).expect("Failed to convert");
            let datetime = naive.with_timezone(&chrono::Local);
            result += &format!("[{}] {}", datetime.format("%a %b %e %T %Y"), log);
            result += "\n";
        }

        result
    }
}
