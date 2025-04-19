use anyhow::anyhow;
use clap::{Parser, Subcommand};
use justrun::{
    ipc::*,
    paths::{CONFIG, DEFAULT_CONFIG, RESULT_BASE},
};
use log::error;
use yaml_rust2 as yaml;

#[derive(Subcommand, Debug, PartialEq)]
enum Command {
    Start {
        #[arg(short, long)]
        name: String,
    },
    Stop {
        #[arg(short, long)]
        name: String,
    },
    Restart {
        #[arg(short, long)]
        name: String,
    },
    Status {
        #[arg(short, long)]
        name: String,
    },
    Reg,
    Unreg,
    Restartd,
    Init,
}

#[derive(Parser, Debug)]
#[command(name = "justrun")]
#[command(about = "Control your justrun services", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() {
    let args = Cli::parse();

    if args.command == Command::Init {
        let default_service = include_bytes!("../../default_service.yaml");
        let current_dit = std::env::current_dir()
            .map_err(|e| anyhow!("Failed to get current directory: {}", e))
            .unwrap();
        let path = current_dit.join("justrun.yaml");
        std::fs::write(&path, default_service)
            .map_err(|e| anyhow!("Failed to create default service file: {}", e))
            .unwrap();
        println!("Default service config created at: {}", path.display());
        println!("Register using \"sudo justrun reg\"");
    }

    if args.command == Command::Reg || args.command == Command::Unreg {
        if !std::path::Path::new(CONFIG).exists() {
            println!("Config file not found, creating default config...");
            let default_config = DEFAULT_CONFIG;
            let config_parent = std::path::Path::new(CONFIG).parent().unwrap();
            std::fs::create_dir_all(config_parent)
                .map_err(|e| anyhow!("Failed to create directory: {}", e))
                .unwrap();
            std::fs::write(CONFIG, default_config)
                .map_err(|e| anyhow!("Failed to create config file: {}", e))
                .unwrap();
        }

        let config_str = std::fs::read_to_string(CONFIG)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))
            .unwrap();
        let mut config = yaml::YamlLoader::load_from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))
            .unwrap()[0]
            .clone();

        let current_dir = std::env::current_dir()
            .map_err(|e| anyhow!("Failed to get current directory: {}", e))
            .unwrap();
        let current_dir = current_dir
            .into_os_string()
            .into_string()
            .map_err(|err| anyhow!("Failed to convert path to string: {:?}", err))
            .unwrap();
        let mut services = config["services"].as_vec().unwrap().clone();
        match args.command {
            Command::Reg => {
                services.push(yaml::Yaml::String(current_dir));
                println!("Registered service succesfully");
            }
            Command::Unreg => {
                services.retain(|s| {
                    if let Some(path) = s.as_str() {
                        path != current_dir
                    } else {
                        true
                    }
                });
                println!("Unregistered service succesfully");
            }
            _ => {}
        }
        config["services"] = yaml::Yaml::Array(services);

        let mut config_out_str = String::new();
        let mut emitter = yaml::YamlEmitter::new(&mut config_out_str);
        emitter
            .dump(&config)
            .map_err(|e| anyhow!("Failed to dump config: {}", e))
            .unwrap();
        std::fs::write(CONFIG, config_out_str)
            .map_err(|e| anyhow!("Failed to write config file: {}", e))
            .unwrap();
    }

    let mut transmitter = Transmitter::new().unwrap_or_else(|err| {
        error!("Failed to create Transmitter: {}", err);
        std::process::exit(1);
    });

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let command = match args.command {
        Command::Start { name } => format!("{} start {}", ts, name),
        Command::Stop { name } => format!("{} stop {}", ts, name),
        Command::Restart { name } => format!("{} restart {}", ts, name),
        Command::Status { name } => format!("{} status {}", ts, name),
        Command::Restartd => format!("{} restartd none", ts),
        _ => {
            error!("Unsupported command to send: {:?}", args.command);
            std::process::exit(1);
        }
    };

    transmitter.send(command.as_str()).unwrap_or_else(|err| {
        eprintln!("Failed to send command: {}", err);
        std::process::exit(1);
    });

    let path = std::path::Path::new(RESULT_BASE).join(ts.to_string());
    loop {
        if path.exists() {
            let result = std::fs::read_to_string(&path).unwrap_or_else(|err| {
                error!("Failed to read result file: {}", err);
                std::process::exit(1);
            });
            println!("{}", result);
            std::fs::remove_file(&path).unwrap_or_else(|err| {
                error!("Failed to remove result file: {}", err);
            });
            break;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
}
