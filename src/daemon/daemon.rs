mod check_running;
pub mod cleanup;
mod maintainer;
pub mod service;

use justrun::ipc;
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

    {
        let maintainer = maintainer.clone();
        let maintainer = maintainer.lock().unwrap();
        maintainer.log("example").unwrap_or_else(|err| {
            error!("{}", err);
            std::process::exit(1);
        });
    }

    rt.block_on(async {
        let listener = ipc::Listener::new().unwrap_or_else(|err| {
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

        loop {
            tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok(data) => {
                                println!("Received: {}", data);
                            }
                            Err(err) => {
                                error!("{}", err);
                                std::process::exit(1);
                            }
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received Ctrl+C, shutting down...");
                        let _ = cleanup::cleanup();
                        break;
                    }
                     _ = sigterm_stream.recv() => {
                        info!("Received SIGTERM, shutting down...");
                        let _ = cleanup::cleanup();
                        break;
            }
                }
        }
    });
}
