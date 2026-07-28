use std::{env, fs, sync::{Arc, RwLock}};

use adi2::{devices::Device, ipc::{GetRequest, Request, Response, SetRequest}, midi::{ChannelParameter, Message}, state::State};

use crate::{midi_client::{MidiClient, MidiCommands}, server::Server};

pub fn run(port_name: &str, device: Device) -> std::io::Result<()> {
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR not set");
    let runtime_path = format!("{}/adi2d", xdg_runtime_dir);
    if !fs::exists(&runtime_path).expect("Failed to check if runtime path exists") {
        let _ = fs::create_dir(&runtime_path).expect("Failed to create runtime path");
    }
    let socket_path = format!("{}/socket", &runtime_path);
    if fs::exists(&socket_path).expect("Failed to check if socket path exists") {
        // TODO: return error whenever I actually add error handling
        panic!("Existing socket found")
    }

    let state = Arc::new(RwLock::new(State::new()));
    let midi_client = MidiClient::from_port_name(port_name, device, midi_message_handler(state.clone()));

    let mut server = Server::new(socket_path);
    server.start(server_handler(midi_client, state))
}

fn midi_message_handler(state: Arc<RwLock<State>>) -> impl Fn(Message, u64) {
    move |message, _| {
        let mut state = state.write().unwrap();
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
    }
}

fn server_handler(mut client: MidiClient, state: Arc<RwLock<State>>) -> impl FnMut(Request) -> Response {
    move |req| {
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
    }
}
