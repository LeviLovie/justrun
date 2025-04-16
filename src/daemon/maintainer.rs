use crate::service::Service;
use anyhow::{anyhow, Result};
use justrun::paths::CONFIG;
use log::warn;
use yaml_rust2 as yaml;

pub struct Maintainer {
    services: Vec<Service>,
}

impl Maintainer {
    pub fn new() -> Self {
        Maintainer {
            services: Vec::new(),
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
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;
        if config.len() == 0 {
            return Ok(());
        }

        let mut services: Vec<Service> = Vec::new();
        for service_yaml in config[0].as_vec().unwrap() {
            let path = service_yaml
                .as_str()
                .ok_or_else(|| anyhow!("Service path is not a string in config file: {}", CONFIG))?
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
                match Service::new(config.clone(), path.clone(), i) {
                    Ok(service) => {
                        services.push(service);
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
        self.services = services;

        Ok(())
    }

    pub fn start_all(&mut self) -> Result<()> {
        for service in &mut self.services {
            service.start()?;
        }
        Ok(())
    }

    pub fn log(&self, name: &str) -> Result<()> {
        for service in &self.services {
            if service.name() == name {
                service.log();
                return Ok(());
            }
        }

        Err(anyhow!("Service not found: {}", name))
    }
}
