use crate::service::Service;
use anyhow::{anyhow, Result};
use justrun::paths::CONFIG;
use log::warn;
use yaml_rust2 as yaml;

pub struct Maintainer {
    services: Vec<Service>,
    pub require_root: bool,
}

impl Maintainer {
    pub fn new() -> Self {
        Maintainer {
            services: Vec::new(),
            require_root: true,
        }
    }

    pub fn load(&mut self) -> Result<()> {
        if !std::path::Path::new(CONFIG).exists() {
            warn!(
                "Config file does not exist, creating default config at {}",
                CONFIG
            );
            let default_config = include_bytes!("../../default_config.yaml");
            let config_parent = std::path::Path::new(CONFIG).parent().unwrap();
            std::fs::create_dir_all(config_parent)
                .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
            std::fs::write(CONFIG, default_config)
                .map_err(|e| anyhow!("Failed to create config file: {}", e))?;
        }

        let config_str = std::fs::read_to_string(CONFIG)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config = yaml::YamlLoader::load_from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?[0]
            .clone();

        self.require_root = config["require_root"].as_bool().unwrap_or(true);
        let services = config["services"].as_vec();

        match services {
            Some(services) => {
                for service_yaml in services {
                    let path = service_yaml
                        .as_str()
                        .ok_or_else(|| {
                            anyhow!("Service path is not a string in config file: {}", CONFIG)
                        })?
                        .to_string();
                    let path = std::path::Path::new(&path)
                        .join("justrun.yaml")
                        .into_os_string()
                        .into_string()
                        .map_err(|err| anyhow!("Failed to convert path to string: {:?}", err))?
                        .to_string();
                    if !std::path::Path::new(&path).exists() {
                        warn!("Service config file does not exist: {}", path);
                        continue;
                    }
                    let config_str = std::fs::read_to_string(&path)
                        .map_err(|e| anyhow!("Failed to read service config file: {}", e))?;
                    let configs = yaml::YamlLoader::load_from_str(&config_str)
                        .map_err(|e| anyhow!("Failed to parse service config file: {}", e))?;
                    for (i, config) in configs.iter().enumerate() {
                        match Service::new(config.clone(), path.clone()) {
                            Ok(service) => {
                                self.services.push(service);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to parse service #{} from config {}: {}",
                                    i, CONFIG, e
                                );
                            }
                        }
                    }
                }
            }
            None => {}
        }

        Ok(())
    }

    pub fn start_all(&mut self) -> Result<()> {
        for service in &mut self.services {
            service.start()?;
        }
        Ok(())
    }

    pub fn start(&mut self, name: &str) -> Result<String> {
        for service in &mut self.services {
            if service.name() == name {
                return service.start();
            }
        }
        Err(anyhow!("Service not found: {}", name))
    }

    pub fn stop_all(&mut self) -> Result<()> {
        for service in &mut self.services {
            service.stop()?;
        }
        Ok(())
    }

    pub fn stop(&mut self, name: &str) -> Result<String> {
        for service in &mut self.services {
            if service.name() == name {
                return service.stop();
            }
        }
        Err(anyhow!("Service not found: {}", name))
    }

    pub fn restart(&mut self, name: &str) -> Result<String> {
        for service in &mut self.services {
            if service.name() == name {
                return service.restart();
            }
        }
        Err(anyhow!("Service not found: {}", name))
    }

    pub fn status(&self, name: &str) -> Result<String> {
        for service in &self.services {
            if service.name() == name {
                return Ok(service.status());
            }
        }
        Err(anyhow!("Service not found: {}", name))
    }
}
