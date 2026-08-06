mod commands;
mod config;
mod midi_client;
mod server;
mod server_state;

use adi2::devices::Device;
use clap::{Parser, Subcommand};

use crate::config::Config;

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
    env_logger::init();

    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command.unwrap_or(Commands::Server) {
        Commands::Server => commands::server::run(config, "ADI-2", Device::Adi2DacFs)?,
    }

    Ok(())
}
