use std::{fs, process, sync::{Arc, PoisonError, RwLock}};

use adi2::{devices::Device, ipc::{GetRequest, Request, Response, SetRequest}, midi::{ChannelAddress, ChannelParameter, Message}, paths};
use log::{error, info};

use crate::{config::Config, hooks::Hooks, midi_client::{MidiClient, MidiClientError, MidiEvent}, server::{HandlerError, Server, ServerError}, server_state::ServerState};

pub fn run(config: Config, port_name: &str, device: Device) -> Result<(), ServerError> {
    let runtime_path = paths::runtime_dir().unwrap_or_else(|e| {
        error!(target: "server", "{}", e);
        process::exit(1);
    });
    info!(target: "server", "Setting runtime path to {}", runtime_path.display());
    if !fs::exists(&runtime_path)? {
        fs::create_dir(&runtime_path).unwrap_or_else(|e| {
            error!(target: "server", "Error creating runtime path {}: {}", runtime_path.display(), e);
            process::exit(1);
        });
    }
    let socket_path = paths::socket_path(&runtime_path);
    let state = Arc::new(RwLock::new(ServerState::default()));
    let hooks = config.hooks();
    let midi_client = MidiClient::from_port_name(port_name, device, config.midi(), midi_event_handler(state.clone(), hooks.clone()));

    let mut server = Server::bind(&socket_path)?;
    info!(target: "server", "Listening on socket at {}", socket_path.display());
    server.start(server_handler(midi_client, state, hooks));
    Ok(())
}

fn midi_event_handler(state: Arc<RwLock<ServerState>>, hooks: Hooks) -> impl Fn(MidiEvent) + Send + Sync {
    move |event| {
        let mut state = state.write().unwrap_or_else(PoisonError::into_inner);
        let message = match event {
            MidiEvent::Message(message) => message,
            MidiEvent::Disconnected => {
                // cached values are stale once the device is gone (and may be changed
                // from another computer), so start over when it comes back
                *state = ServerState::default();
                return;
            }
        };
        // only count as connected once the device has actually replied
        state.set_connected();
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
                    let is_active = state.active_output.is_some_and(|o| ChannelAddress::from(o) == param.address);
                    let old = state.get_channel_parameter_value(Some(param.address), param.parameter);
                    if state.set_channel_parameter_from_device(param) && is_active {
                        run_channel_hooks_on_change(&hooks, param, old);
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

fn server_handler(mut client: MidiClient, state: Arc<RwLock<ServerState>>, hooks: Hooks) -> impl FnMut(Request) -> Result<Response, HandlerError> {
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
                if !state.connected {
                    return Err(HandlerError::NotConnected);
                }
                match s {
                    SetRequest::CurrentChannelParameter(_type, value) => {
                        let active_output = state.active_output.ok_or(HandlerError::ActiveOutputUnset)?;
                        let param = ChannelParameter {
                            address: active_output.into(),
                            parameter: _type,
                            value,
                        };
                        let old = state.get_channel_parameter_value(None, _type);
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                        run_channel_hooks_on_change(&hooks, &param, old);
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
                        run_channel_hooks_on_change(&hooks, &param, Some(current_value));
                        return Ok(Response::ChannelParameter(Some(new_value)));
                    }
                    SetRequest::ToggleCurrentChannelParameter(parameter) => {
                        let active_output = state.active_output.ok_or(HandlerError::ActiveOutputUnset)?;
                        let Some(current) = state.get_channel_parameter_value(None, parameter) else {
                            return Ok(Response::Err(format!("Failed to toggle {}: no current value available", parameter)));
                        };
                        let new = if current == 0 { 1 } else if current == 1 { 0 } else {
                            return Ok(Response::Err(format!("Expected toggleable value of 0 or 1 for {} but found {}", parameter, current)));
                        };
                        let param = ChannelParameter {
                            address: active_output.into(),
                            parameter,
                            value: new,
                        };
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                        run_channel_hooks_on_change(&hooks, &param, Some(current));
                        return Ok(Response::ChannelParameter(Some(new)));
                    }
                    SetRequest::ChannelParameter(param) => {
                        let old = state.get_channel_parameter_value(Some(param.address), param.parameter);
                        client.send_command(&param)?;
                        state.change_channel_parameter(&param);
                        run_channel_hooks_on_change(&hooks, &param, old);
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

fn run_channel_hooks_on_change(hooks: &Hooks, param: &ChannelParameter, old: Option<i16>) {
    let Some(old) = old else { return };
    if old == param.value {
        return;
    }
    hooks.run_channel_hooks(param.parameter, old, param.value);
}
