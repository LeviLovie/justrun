use clap::{Parser, Subcommand};
use justrun::ipc::*;
use log::error;

#[derive(Subcommand, Debug)]
enum Command {
    Start,
    Stop,
    Status,
}

#[derive(Parser, Debug)]
#[command(name = "justrun")]
#[command(about = "Control your justrun services", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(short, long, help = "Name of the service")]
    name: String,
}

fn main() {
    let args = Cli::parse();

    let mut transmitter = Transmitter::new().unwrap_or_else(|err| {
        error!("Failed to create Transmitter: {}", err);
        std::process::exit(1);
    });

    let command = match args.command {
        Command::Start => format!("start {}", args.name),
        Command::Stop => format!("stop {}", args.name),
        Command::Status => format!("status {}", args.name),
    };

    transmitter.send(command.as_str()).unwrap_or_else(|err| {
        eprintln!("Failed to send command: {}", err);
        std::process::exit(1);
    });
}
