use crate::midi::{self, ChannelAddress, ChannelParameter, ChannelParameterType, DeviceParameter, DeviceParameterType, Output};
use std::collections::HashMap;

type ChannelParameters = HashMap<ChannelParameterType, i16>;
type DeviceParameters = HashMap<DeviceParameterType, u16>;

#[derive(Debug, Default)]
pub struct State {
    active_output: Option<midi::Output>,
    channel_parameters: HashMap<ChannelAddress, ChannelParameters>,
    device_parameters: DeviceParameters,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_active_output(&mut self, output: Output) -> Option<midi::Output> {
        let prev = self.active_output;
        self.active_output = Some(output);
        prev
    }

    // Assumes the active output if address not given
    pub fn get_channel_parameter_value(&self, address: Option<ChannelAddress>, _type: ChannelParameterType) -> Option<i16> {
        let address = address.or_else(|| self.active_output.map(Into::into))?;
        self.channel_parameters.get(&address)?.get(&_type).copied()
    }

    pub fn set_channel_parameter(&mut self, param: &ChannelParameter) {
        self.channel_parameters
            .entry(param.address)
            .or_insert(ChannelParameters::new())
            .insert(param.parameter, param.value);
    }

    pub fn get_device_parameter_value(&self, _type: DeviceParameterType) -> Option<u16> {
        self.device_parameters.get(&_type).copied()
    }

    pub fn set_device_parameter(&mut self, param: &DeviceParameter) {
        self.device_parameters.insert(param.parameter, param.value);
    }
}
