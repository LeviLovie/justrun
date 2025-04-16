mod check_running;
pub mod cleanup;
mod maintainer;
pub mod service;

use justrun::{ipc, paths::RESULT_BASE};
use log::{error, info};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    if let Err(err) = check_running::check_running() {
        error!("{}", err);
        std::process::exit(1);
    }

    let rt = Runtime::new().unwrap_or_else(|err| {
        error!("Failed to create Tokio runtime: {}", err);
        std::process::exit(1);
    });

    let maintainer = {
        let mut maintainer = maintainer::Maintainer::new();
        maintainer.load().unwrap_or_else(|err| {
            error!("{}", err);
            std::process::exit(1);
        });
        maintainer.start_all().unwrap_or_else(|err| {
            error!("{}", err);
            std::process::exit(1);
        });
        Arc::new(Mutex::new(maintainer))
    };

    rt.block_on(async {
        let require_root = maintainer.lock().unwrap().require_root;
        let listener = ipc::Listener::new(require_root).unwrap_or_else(|err| {
            error!("{}", err);
            std::process::exit(1);
        });
        info!("Started socket successfully");

        let mut sigterm_stream =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .unwrap_or_else(|err| {
                    error!("Failed to create SIGTERM stream: {}", err);
                    std::process::exit(1);
                });

        let maintainer = maintainer.clone();
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok(data) => {
                            let parts = data.split_whitespace().collect::<Vec<_>>();
                            if parts.len() < 3 {
                                error!("Invalid command: {}", data);
                                continue;
                            }
                            let timestamp = parts[0];
                            let action = parts[1];
                            let arg = parts[2];
                            let mut maintainer = maintainer.lock().unwrap();
                            let output = match action {
                                "start" => {
                                    maintainer.start(arg).unwrap_or_else(|err| {
                                        error!("Failed to start service: {}", err);
                                        format!("Failed to start service: {}", err)
                                    })
                                }
                                "stop" => {
                                    maintainer.stop(arg).unwrap_or_else(|err| {
                                        error!("Failed to stop service: {}", err);
                                        format!("Failed to stop service: {}", err)
                                    })
                                }
                                "restart" => {
                                    maintainer.restart(arg).unwrap_or_else(|err| {
                                        error!("Failed to restart service: {}", err);
                                        format!("Failed to restart service: {}", err)
                                    })
                                }
                                "status" => {
                                    maintainer.status(arg).unwrap_or_else(|err| {
                                        error!("Failed to get status: {}", err);
                                        format!("Failed to get status: {}", err)
                                    })
                                }
                                _ => {
                                    error!("Unknown command: {}", parts[0]);
                                    continue;
                                }
                            };
                            drop(maintainer);
                            let path = std::path::Path::new(RESULT_BASE).join(timestamp);
                            let parent = path.parent().unwrap();
                            std::fs::create_dir_all(parent).unwrap_or_else(|err| {
                                error!("Failed to create directory: {}", err);
                            });
                            if path.exists() {
                                std::fs::remove_file(&path).unwrap_or_else(|err| {
                                    error!("Failed to remove file: {}", err);
                                });
                            }
                            std::fs::write(&path, output).unwrap_or_else(|err| {
                                error!("Failed to write to file: {}", err);
                            });
                        }
                        Err(err) => {
                            error!("{}", err);
                            std::process::exit(1);
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                    let mut maintainer = maintainer.lock().unwrap();
                    maintainer.stop_all().unwrap_or_else(|err| {
                        error!("Failed to stop all services: {}", err);
                    });
                    let _ = cleanup::cleanup();
                    break;
                }
                 _ = sigterm_stream.recv() => {
                    info!("Received SIGTERM, shutting down...");
                    let mut maintainer = maintainer.lock().unwrap();
                    maintainer.stop_all().unwrap_or_else(|err| {
                        error!("Failed to stop all services: {}", err);
                    });
                    let _ = cleanup::cleanup();
                    break;
                }
            }
        }
    });
}
