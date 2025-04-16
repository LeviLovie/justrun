use justrun::ipc::*;
use log::error;

fn main() {
    let mut transmitter = Transmitter::new().unwrap_or_else(|err| {
        error!("Failed to create Transmitter: {}", err);
        std::process::exit(1);
    });

    transmitter.send("Hello there!").unwrap_or_else(|err| {
        error!("Failed to send message: {}", err);
        std::process::exit(1);
    });
}
