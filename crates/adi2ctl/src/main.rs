mod client;

use std::{env, fs, process};

use adi2::{ipc::Response, midi::ChannelParameterType};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::client::Client;

// 1 unit = .1 dBr on device
// TODO: make this a config option whenever I get around to adding a config file
const VOLUME_UNIT_CHANGE: i16 = 25;

// don't allow going above -30 dBr
// TODO: also make this configurable/toggle-able
const MAXIMUM_VOLUME: i16 = -300;
const MINIMUM_VOLUME: i16 = -1145;

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
    Mute(MuteArgs),
}

#[derive(Args)]
struct SetVolumeArgs {
    #[arg(allow_hyphen_values = true)]
    volume: i16,
}

#[derive(Args)]
struct MuteArgs {
    #[arg(default_value = "on")]
    state: MuteState,
}

#[derive(Clone, ValueEnum)]
enum MuteState {
    On,
    Off,
    Toggle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR not set");
    let runtime_path = format!("{}/adi2d", xdg_runtime_dir);
    let socket_path = format!("{}/socket", &runtime_path);
    if !fs::exists(&socket_path).expect("Failed to check if adi2d socket exists") {
        eprintln!("adi2d is not running; no socket found in {}", runtime_path);
        process::exit(1);
    }
    let client = Client::new(socket_path);

    match cli.command {
        Commands::Status => {
            match client.get_state() {
                Ok(Response::State(s)) => println!("{:?}", s),
                Ok(Response::Err(e)) => exit_with_error(e.into()),
                Err(e)=> exit_with_error(e.into()),
                Ok(r) => {
                    eprintln!("Unexpected response from server: {:?}", r);
                    process::exit(1);
                }
            }
            let state = client.get_state()?;
            println!("{:?}", state);
        }
        Commands::IncrementVolume | Commands::DecrementVolume => {
            let change = match cli.command {
                Commands::DecrementVolume => -VOLUME_UNIT_CHANGE,
                _ => VOLUME_UNIT_CHANGE,
            };
            match client.adjust_current_channel_parameter_value(ChannelParameterType::Volume, change, MINIMUM_VOLUME, MAXIMUM_VOLUME) {
                Ok(Response::ChannelParameter(Some(v))) => println!("{}", v),
                Ok(Response::Err(e)) => exit_with_error(e.into()),
                Err(e)=> exit_with_error(e.into()),
                Ok(r) => {
                    eprintln!("Unexpected response from server: {:?}", r);
                    process::exit(1);
                }
            }
        }
        Commands::SetVolume(args) => {
            let volume = args.volume.clamp(MINIMUM_VOLUME, MAXIMUM_VOLUME);
            if volume != args.volume {
                println!("Clamped volume to range [{}, {}]", MINIMUM_VOLUME, MAXIMUM_VOLUME);
            }
            match client.set_current_channel_parameter_value(ChannelParameterType::Volume, volume) {
                Ok(Response::Ok) => {}
                Ok(Response::Err(e)) => exit_with_error(e.into()),
                Err(e) => exit_with_error(e.into()),
                Ok(r) => {
                    eprintln!("Unexpected response from server: {:?}", r);
                    process::exit(1);
                }
            }
        }
        Commands::Mute(args) => {
            let response = match args.state {
                MuteState::On => client.set_current_channel_parameter_value(ChannelParameterType::Mute, 1),
                MuteState::Off => client.set_current_channel_parameter_value(ChannelParameterType::Mute, 0),
                MuteState::Toggle => client.toggle_current_channel_parameter(ChannelParameterType::Mute),
            };
            match response {
                Ok(Response::Ok) => {}
                Ok(Response::ChannelParameter(Some(v))) => println!("{}", if v == 0 { "off" } else { "on" }),
                Ok(Response::Err(e)) => exit_with_error(e.into()),
                Err(e) => exit_with_error(e.into()),
                Ok(r) => {
                    eprintln!("Unexpected response from server: {:?}", r);
                    process::exit(1);
                }
            }
        }
    }

    Ok(())
}

fn exit_with_error(err: Box<dyn std::error::Error>) {
    eprintln!("error: {:?}", err);
    process::exit(1);
}
