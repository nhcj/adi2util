mod midi_client;
mod server;

use adi2::{devices, ipc::{GetRequest, Request, Response, SetRequest}, midi::{ChannelParameter, ChannelParameterType, Message}};
use clap::{Parser, Subcommand};

use adi2::state::State;

use crate::{midi_client::MidiCommands, server::Server};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let state = std::sync::Arc::new(std::sync::RwLock::new(State::new()));
    let state_ = state.clone();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            let mut client = midi_client::MidiClient::from_port_name("ADI-2",devices::Device::Adi2DacFs,  move |message, _| {
                let mut state = state_.write().unwrap();
                match message {
                    Message::Status(status) => {
                        state.set_active_output(status.active_output);
                    }
                    Message::Parameters(params) => {
                        for param in params.channel.iter() {
                            state.set_channel_parameter(param);
                        }
                    }
                }
            });

            let mut server = Server::new("/tmp/adi2d.sock");
            server.start(|req| {
                let state = state.read().unwrap();
                match req {
                    Request::Get(g) => {
                        match g {
                            GetRequest::State => Response::State(state.clone()),
                            GetRequest::ChannelParameter(address, _type) => Response::ChannelParameter(state.get_channel_parameter_value(Some(address), _type)),
                            GetRequest::DeviceParameter(_type) => Response::DeviceParameter(state.get_device_parameter_value(_type)),
                        }
                    }
                    Request::Set(s) => {
                        match s {
                            SetRequest::CurrentChannelParameter(_type, value) => {
                                let Some(active_output) = state.active_output else { return Response::Err(()); };
                                let param = ChannelParameter {
                                    address: active_output.into(),
                                    parameter: _type,
                                    value,
                                };
                                client.send_command(MidiCommands::SendParameterToDevice, Some(param)).unwrap();
                            }
                            SetRequest::ChannelParameter(param) => { client.send_command(MidiCommands::SendParameterToDevice, Some(param)).unwrap(); }
                            SetRequest::DeviceParameter(param) => { client.send_command(MidiCommands::SendParameterToDevice, Some(param)).unwrap(); }
                        }
                        Response::Ok
                    }
                }
            })?;

            loop {
                println!("{:?}", state.read().unwrap().get_channel_parameter_value(None, ChannelParameterType::Volume));
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
}
