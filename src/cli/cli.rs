use clap::{Parser, Subcommand};
use justrun::{ipc::*, paths::RESULT_BASE};
use log::error;

#[derive(Subcommand, Debug)]
enum Command {
    Start,
    Stop,
    Restart,
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

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let command = match args.command {
        Command::Start => format!("{} start {}", ts, args.name),
        Command::Stop => format!("{} stop {}", ts, args.name),
        Command::Restart => format!("{} restart {}", ts, args.name),
        Command::Status => format!("{} status {}", ts, args.name),
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
