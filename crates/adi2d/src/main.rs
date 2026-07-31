mod commands;
mod midi_client;
mod server;

use adi2::devices::Device;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Server,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Server) {
        Commands::Server => commands::server::run("ADI-2", Device::Adi2DacFs)?,
    }

    Ok(())
}
