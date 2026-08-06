use std::{collections::HashMap, ops::Deref, time::{Duration, Instant}};

use adi2::{midi::{ChannelAddress, ChannelParameter, ChannelParameterType, DeviceParameter, DeviceParameterType, Output}, state::State};

// When doing things like incrementing/decrementing volume, the new get values are saved into
// state immediately, and it's possible they get overwritten by the midi client's status poll
// thread. This is an attempt to fix that issue by recording local changes and trying to
// reconcile them with the device state as we receive new status messages. Not totally sure
// that this is a good solution yet, but it seems to work? Will want to relook at all this
// later since I think all the types getting more complicated than neeeded
#[derive(Default)]
pub struct ServerState {
    state: State,
    pending_channel_changes: HashMap<(ChannelAddress, ChannelParameterType), (i16, Instant)>,
    pending_device_changes: HashMap<DeviceParameterType, (u16, Instant)>,
}

impl Deref for ServerState {
    type Target = State;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl ServerState {
    pub fn set_channel_parameter_from_device(&mut self, param: &ChannelParameter) -> bool {
        let key = (param.address, param.parameter);
        match self.pending_channel_changes.get(&key) {
            // Pending state is reconciled with device state
            Some((v, _)) if *v == param.value => {
                self.pending_channel_changes.remove(&key);
            }
            Some((_, t)) if t.elapsed() < Duration::from_secs(1) => { return false; }
            _ => { self.pending_channel_changes.remove(&key); }
        }
        self.state.set_channel_parameter(param);
        true
    }

    pub fn change_channel_parameter(&mut self, param: &ChannelParameter) {
        self.pending_channel_changes.insert((param.address, param.parameter), (param.value, Instant::now()));
        self.state.set_channel_parameter(param);
    }

    pub fn set_device_parameter_from_device(&mut self, param: &DeviceParameter) {
        match self.pending_device_changes.get(&param.parameter) {
            // Pending state is reconciled with device state
            Some((v, _)) if *v == param.value => {
                self.pending_device_changes.remove(&param.parameter);
            }
            Some((_, t)) if t.elapsed() < Duration::from_secs(1) => { return; }
            _ => { self.pending_device_changes.remove(&param.parameter); }
        }
        self.state.set_device_parameter(param);
    }

    pub fn change_device_parameter(&mut self, param: &DeviceParameter) {
        self.pending_device_changes.insert(param.parameter, (param.value, Instant::now()));
        self.state.set_device_parameter(param);
    }

    pub fn set_active_output(&mut self, output: Output) -> Option<Output> {
        self.state.set_active_output(output)
    }
}
