use std::{env, fs, process, sync::{Arc, RwLock}};

use adi2::{devices::Device, ipc::{GetRequest, Request, Response, SetRequest}, midi::{ChannelParameter, Message}, state::State};
use log::{error, info};

use crate::{midi_client::{MidiClient, MidiClientError}, server::{HandlerError, Server, ServerError}};

pub fn run(port_name: &str, device: Device) -> Result<(), ServerError> {
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        error!(target: "server", "XDG_RUNTIME_DIR must be set");
        process::exit(1);
    });
    let runtime_path = format!("{}/adi2d", xdg_runtime_dir);
    info!(target: "server", "Setting runtime path to {}", runtime_path);
    if !fs::exists(&runtime_path)? {
        fs::create_dir(&runtime_path).unwrap_or_else(|e| {
            error!(target: "server", "Error creating runtime path {}: {}", runtime_path, e);
            process::exit(1);
        });
    }
    let socket_path = format!("{}/socket", &runtime_path);
    let state = Arc::new(RwLock::new(State::new()));
    let midi_client = MidiClient::from_port_name(port_name, device, midi_message_handler(state.clone()));
    info!(target: "midi_client", "Using port '{}'", midi_client.port_name());

    let mut server = Server::bind(&socket_path)?;
    info!(target: "server", "Listening on socket at {}", socket_path);
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

                for param in params.device.iter() {
                    state.set_device_parameter(param);
                }
            }
        }
    }
}

fn server_handler(mut client: MidiClient, state: Arc<RwLock<State>>) -> impl FnMut(Request) -> Result<Response, HandlerError> {
    move |req| {
        match req {
            Request::Get(g) => {
                let state = state.read().unwrap();
                match g {
                    GetRequest::State => Ok(Response::State(state.clone())),
                    GetRequest::CurrentChannelParameter(_type) => Ok(Response::ChannelParameter(state.get_channel_parameter_value(None, _type))),
                    GetRequest::ChannelParameter(address, _type) => Ok(Response::ChannelParameter(state.get_channel_parameter_value(Some(address), _type))),
                    GetRequest::DeviceParameter(_type) => Ok(Response::DeviceParameter(state.get_device_parameter_value(_type))),
                }
            }
            Request::Set(s) => {
                match s {
                    SetRequest::CurrentChannelParameter(_type, value) => {
                        let active_output = state.read().unwrap().active_output.ok_or(HandlerError::ActiveOutputUnset)?;
                        let param = ChannelParameter {
                            address: active_output.into(),
                            parameter: _type,
                            value,
                        };
                        client.send_command(&param)?;
                    }
                    SetRequest::ChannelParameter(param) => { client.send_command(&param)?; }
                    SetRequest::DeviceParameter(param) => { client.send_command(&param)?; }
                }
                Ok(Response::Ok)
            }
        }
    }
}
