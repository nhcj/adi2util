use std::{env, fs, process, sync::{Arc, PoisonError, RwLock}};

use adi2::{devices::Device, ipc::{GetRequest, Request, Response, SetRequest}, midi::{ChannelAddress, ChannelParameter, Message}};
use log::{error, info};

use crate::{config::{Config, Hooks}, midi_client::{MidiClient, MidiClientError}, server::{HandlerError, Server, ServerError}, server_state::ServerState};

pub fn run(config: Config, port_name: &str, device: Device) -> Result<(), ServerError> {
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
    let state = Arc::new(RwLock::new(ServerState::default()));
    let midi_client = MidiClient::from_port_name(port_name, device, midi_message_handler(state.clone(), config.hooks()));
    info!(target: "midi_client", "Using port '{}'", midi_client.port_name());

    let mut server = Server::bind(&socket_path)?;
    info!(target: "server", "Listening on socket at {}", socket_path);
    server.start(server_handler(midi_client, state));
    Ok(())
}

fn midi_message_handler(state: Arc<RwLock<ServerState>>, hooks: Hooks) -> impl Fn(Message, u64) {
    move |message, _| {
        let mut state = state.write().unwrap_or_else(PoisonError::into_inner);
        match message {
            Message::Status(status) => {
                let old = state.active_output;
                if old.is_some_and(|o| o == status.active_output) {
                    return;
                }
                state.set_active_output(status.active_output);
                hooks.run_active_output_hooks(old, status.active_output);
            }
            Message::Parameters(params) => {
                for param in params.channel.iter() {
                    if state.active_output.is_some_and(|o| ChannelAddress::from(o) == param.address) {
                        let old = state.get_channel_parameter_value(None, param.parameter);
                        if state.set_channel_parameter_from_device(param) {
                            match old {
                                Some(old) => { hooks.run_channel_hooks(param.parameter, old, param.value) }
                                None => { hooks.run_channel_hooks(param.parameter, "-", param.value); }
                            }
                        }
                    }
                }

                for param in params.device.iter() {
                    state.set_device_parameter_from_device(param);
                }
            }
            Message::Ignore => {}
        }
    }
}

fn server_handler(mut client: MidiClient, state: Arc<RwLock<ServerState>>) -> impl FnMut(Request) -> Result<Response, HandlerError> {
    move |req| {
        match req {
            Request::Get(g) => {
                let state = state.read().unwrap_or_else(PoisonError::into_inner);
                match g {
                    GetRequest::State => Ok(Response::State(state.clone())),
                    GetRequest::CurrentChannelParameter(_type) => Ok(Response::ChannelParameter(state.get_channel_parameter_value(None, _type))),
                    GetRequest::ChannelParameter(address, _type) => Ok(Response::ChannelParameter(state.get_channel_parameter_value(Some(address), _type))),
                    GetRequest::DeviceParameter(_type) => Ok(Response::DeviceParameter(state.get_device_parameter_value(_type))),
                }
            }
            Request::Set(s) => {
                let mut state = state.write().unwrap_or_else(PoisonError::into_inner);
                match s {
                    SetRequest::CurrentChannelParameter(_type, value) => {
                        let active_output = state.active_output.ok_or(HandlerError::ActiveOutputUnset)?;
                        let param = ChannelParameter {
                            address: active_output.into(),
                            parameter: _type,
                            value,
                        };
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                    }
                    SetRequest::AdjustCurrentChannelParameter { parameter, change, allowed_range } => {
                        let active_output = state.active_output.ok_or(HandlerError::ActiveOutputUnset)?;
                        let Some(current_value) = state.get_channel_parameter_value(None, parameter) else {
                            return Ok(Response::Err(format!("No current value for the active output's channel parameter {}", parameter)));
                        };
                        let (min, max) = allowed_range;
                        if min > max {
                            return Ok(Response::Err(format!("Adjustment range has min ({}) larger than max ({})", min, max)));
                        }
                        let new_value = current_value.saturating_add(change).clamp(min, max);
                        let param = ChannelParameter {
                            address: active_output.into(),
                            parameter,
                            value: new_value,
                        };
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                        return Ok(Response::ChannelParameter(Some(new_value)));
                    }
                    SetRequest::ChannelParameter(param) => {
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                    }
                    SetRequest::DeviceParameter(param) => {
                        client.send_command(&param)?;
                        state.change_device_parameter(&param);
                    }
                }
                Ok(Response::Ok)
            }
        }
    }
}
