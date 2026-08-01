mod client;

use std::{env, fs, process};

use adi2::midi::ChannelParameterType;
use clap::{Args, Parser, Subcommand};

use crate::client::Client;

// 1 unit = .1 dBr on device
// TODO: make this a config option whenever I get around to adding a config file
const VOLUME_UNIT_CHANGE: i16 = 25;

// don't allow going above -30 dBr
// TODO: also make this configurable/toggle-able
const MAXIMUM_VOLUME: i16 = -300;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    IncrementVolume,
    DecrementVolume,
    SetVolume(SetVolumeArgs),
}

#[derive(Args)]
struct SetVolumeArgs {
    #[arg(allow_hyphen_values = true)]
    volume: i16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR not set");
    let runtime_path = format!("{}/adi2d", xdg_runtime_dir);
    let socket_path = format!("{}/socket", &runtime_path);
    if !fs::exists(&socket_path).expect("Failed to check if adi2d socket exists") {
        eprintln!("adi2d is not running; no socket found in {}", runtime_path);
        process::exit(1);
    }

    let client = Client::new(socket_path);

    let cli = Cli::parse();
    match cli.command {
        Commands::Status => {
            let state = client.get_state()?;
            println!("{:?}", state);
        }
        Commands::IncrementVolume | Commands::DecrementVolume => {
            let current_volume = client.get_current_channel_parameter_value(ChannelParameterType::Volume)?;
            let Some(current_volume) = current_volume else { 
                eprintln!("Volume for currently active output couldn't be found");
                process::exit(2);
            };
            let mut change = VOLUME_UNIT_CHANGE;
            if matches!(cli.command, Commands::DecrementVolume) {
                change *= -1;
            }
            let mut volume = current_volume + change;
            if volume > MAXIMUM_VOLUME {
                eprintln!("Requested volume is higher than maximum {}; setting to maximum", MAXIMUM_VOLUME);
                volume = MAXIMUM_VOLUME;
            }
            client.set_current_channel_parameter_value(ChannelParameterType::Volume, volume)?;
        }
        Commands::SetVolume(args) => {
            let volume = if args.volume <= MAXIMUM_VOLUME { args.volume } else {
                eprintln!("Requested volume is higher than maximum {}; setting to maximum", MAXIMUM_VOLUME);
                MAXIMUM_VOLUME
            };
            client.set_current_channel_parameter_value(ChannelParameterType::Volume, volume)?;
        }
    }

    Ok(())
}
