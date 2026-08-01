use bitcode::{Encode, Decode};
use thiserror::Error;

use crate::devices::Device;

#[repr(u8)]
pub enum CommandId {
    SendParameterToDevice = 0x02,
    RequestToDevice = 0x03,
}

#[derive(Debug, Error)]
pub enum MidiError {
    #[error("invalid message header: expected {expected:02X?}, received {received:02X?}")]
    InvalidMessageHeader {
        expected: [u8; 5],
        received: Vec<u8>,
    },
    #[error("invalid message terminator: expected {expected:#04X}, received {received:#04X}")]
    InvalidMessageTerminator {
        expected: u8,
        received: u8,
    },
    #[error("unsupported command: {0:#04X}")]
    UnsupportedCommand(u8),
    #[error("unknown channel address: {0}")]
    UnknownChannelAddress(u8),
    #[error("unknown channel parameter: {0}")]
    UnknownChannelParameter(u8),
    #[error("unknown device parameter: {0}")]
    UnknownDeviceParameter(u8),
    #[error("invalid output: {0}")]
    InvalidOutput(u8),
    #[error("channel parameter value {value} out of range [{min:?}, {max:?}]")]
    ChannelValueOutOfRange {
        value: i16,
        min: i16,
        max: i16,
    },
    #[error("device parameter value {value} out of range [0, {max:?}]")]
    DeviceValueOutOfRange {
        value: u16,
        max: u16,
    },
}

pub trait MidiCommand {
    const COMMAND: CommandId;
    fn encode_sysex(&self) -> Result<Vec<u8>, MidiError>;
}

impl MidiCommand for ChannelParameter {
    const COMMAND: CommandId = CommandId::SendParameterToDevice;
    fn encode_sysex(&self) -> Result<Vec<u8>, MidiError> {
        self.to_midi_bytes().map(|b| b.to_vec())
    }
}

impl MidiCommand for DeviceParameter {
    const COMMAND: CommandId = CommandId::SendParameterToDevice;
    fn encode_sysex(&self) -> Result<Vec<u8>, MidiError> {
        self.to_midi_bytes().map(|b| b.to_vec())
    }
}

pub fn encode_sysex<C: MidiCommand>(device: Device, cmd: &C) -> Result<Vec<u8>, MidiError> {
    let mut data = device.header().to_vec();
    data.push(C::COMMAND as u8);
    data.extend(cmd.encode_sysex()?);
    data.push(0xF7);
    Ok(data)
}

pub enum MidiRequests {
    Status,
}

impl MidiCommand for MidiRequests {
    const COMMAND: CommandId = CommandId::RequestToDevice;

    fn encode_sysex(&self) -> Result<Vec<u8>, MidiError> {
        match self {
            MidiRequests::Status => Ok(vec![0x09]),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Decode, Encode, Eq, Hash, PartialEq)]
pub enum Output {
    Line = 0,
    Phones = 1,
    IEMs = 2,
}

impl Output {
    fn channel_address(&self) -> u8 {
        (*self as u8 + 1) * 3
    }
}

impl TryFrom<u8> for Output {
    type Error = MidiError;

    fn try_from(output: u8) -> Result<Self, Self::Error> {
        match output {
            0 => Ok(Output::Line),
            1 => Ok(Output::Phones),
            2 => Ok(Output::IEMs),
            output => Err(MidiError::InvalidOutput(output)),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Status(StatusMessage),
    Parameters(Parameters),
}

impl Message {
    pub fn from_midi_bytes(device: crate::devices::Device, data: &[u8]) -> Result<Self, MidiError> {
        // Bytes 1-4 = always F0 00 20 0D
        // Byte 5 = Device ID (0x71 for ADI-2 FS)
        let expected_header = device.header();
        let received_header = &data[..expected_header.len().min(data.len())];
        if received_header != expected_header {
            return Err(MidiError::InvalidMessageHeader {
                expected: expected_header,
                received: received_header.to_vec(),
            });
        }

        // Final byte = 0xF7
        let terminator = data[data.len() - 1];
        if terminator != 0xF7 {
            return Err(MidiError::InvalidMessageTerminator {
                expected: 0xF7,
                received: terminator,
            });
        }

        // Byte 6 = Command
        // Respond only to parameters (0x01) or status (0x07) to remote
        match data[5] {
            0x07 => Ok(Message::Status(StatusMessage {
                active_output: Output::try_from(data[6])?,
            })),
            0x01 => Ok(Message::Parameters(Parameters::try_from(
                &data[6..data.len() - 1],
            )?)),
            command => Err(MidiError::UnsupportedCommand(command)),
        }
    }
}

#[derive(Debug)]
pub struct StatusMessage {
    pub active_output: Output,
}

#[derive(Debug)]
pub struct Parameters {
    pub channel: Vec<ChannelParameter>,
    pub device: Vec<DeviceParameter>,
}

impl TryFrom<&[u8]> for Parameters {
    type Error = MidiError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let parameters_count = data.len() / 3;
        let mut channel_parameters = Vec::new();
        let mut device_parameters = Vec::new();

        for i in 0..parameters_count {
            let first_byte = data[i * 3];
            let second_byte = data[i * 3 + 1];

            let address = first_byte >> 3;
            if address == 12 {
                let third_byte = data[i * 3 + 2] as u16;

                let index = (first_byte & 0x07) << 3 | (second_byte & 0x70) >> 4;
                let value = ((second_byte & 0xF) as u16) << 7 | third_byte;

                let device_parameter = DeviceParameter::from_raw_values(index, value)?;
                device_parameters.push(device_parameter);
            } else {
                let third_byte = data[i * 3 + 2] as i16;

                let index = (first_byte & 0x07) << 2 | (second_byte & 0x60) >> 5;
                // channel parameter values are signed 12-bit
                let value = match ((second_byte & 0x1F) as i16) << 7 | third_byte {
                    value if value & 0x800 != 0 => value | !0xFFF,
                    value => value,
                };

                let channel_parameter = ChannelParameter::from_raw_values(address, index, value)?;
                channel_parameters.push(channel_parameter);
            }
        }
        Ok(Parameters {
            channel: channel_parameters,
            device: device_parameters,
        })
    }
}

#[derive(Debug, Decode, Encode)]
pub struct ChannelParameter {
    pub address: ChannelAddress,
    pub parameter: ChannelParameterType,
    pub value: i16,
}

impl ChannelParameter {
    fn from_raw_values(address: u8, index: u8, value: i16) -> Result<Self, MidiError> {
        let address = match address {
            3 => Output::Line.into(),
            6 => Output::Phones.into(),
            9 => Output::IEMs.into(),
            a @ 1..=14 => ChannelAddress::Unsupported(a),
            a => return Err(MidiError::UnknownChannelAddress(a)),
        };
        let parameter = ChannelParameterType::try_from(index)?;
        Ok(ChannelParameter {
            address,
            parameter,
            value,
        })
    }

    fn to_midi_bytes(&self) -> Result<[u8; 3], MidiError> {
        // channel parameter values are signed 12-bit
        if self.value < -0x800 || self.value > 0x7FF {
            return Err(MidiError::ChannelValueOutOfRange {
                value: self.value,
                min: -0x800,
                max: 0x7FF,
            });
        }
        let address: u8 = self.address.into();
        let index = self.parameter as u8;
        let first_byte = (address << 3) | (index & 0x1C) >> 2;
        let second_byte = (index & 0x03) << 5 | ((self.value & 0x800) >> 7) as u8 | ((self.value >> 7) & 0xF) as u8;
        let third_byte = (self.value & 0x7F) as u8;
        Ok([first_byte, second_byte, third_byte])
    }
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, Hash, PartialEq)]
pub enum ChannelAddress {
    Output(Output),
    Unsupported(u8),
}

impl From<ChannelAddress> for u8 {
    fn from(address: ChannelAddress) -> Self {
        match address {
            ChannelAddress::Output(o) => o.channel_address(),
            ChannelAddress::Unsupported(a) => a,
        }
    }
}

impl From<Output> for ChannelAddress {
    fn from(output: Output) -> Self {
        Self::Output(output)
    }
}

#[derive(Debug, Decode, Encode)]
pub struct DeviceParameter {
    pub parameter: DeviceParameterType,
    pub value: u16,
}

impl DeviceParameter {
    fn from_raw_values(index: u8, value: u16) -> Result<Self, MidiError> {
        let parameter = DeviceParameterType::try_from(index)?;
        Ok(DeviceParameter {
            parameter,
            value,
        })
    }

    fn to_midi_bytes(&self) -> Result<[u8; 3], MidiError> {
        // device parameter values are unsigned 11-bit
        if self.value > 0x7FF {
            return Err(MidiError::DeviceValueOutOfRange {
                value: self.value,
                max: 0x7FF,
            });
        }
        let index = self.parameter as u8;
        // device parameters always use address 12
        let first_byte = (12 << 3) | (index & 0x38) >> 3;
        let second_byte = (index & 0x07) << 4 | ((self.value >> 7) & 0xF) as u8;
        let third_byte = (self.value & 0x7F) as u8;
        Ok([first_byte, second_byte, third_byte])
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, Hash, PartialEq)]
pub enum ChannelParameterType {
    Source = 1,
    RefLevel,
    AutoRefLevel,
    Mono,
    Width,
    MSProc,
    Polarity,
    Crossfeed,
    DAFilter,
    DeEmphasis,
    DualEQ,
    Volume,
    LockVolume,
    Balance,
    Mute,
    Dim,
    LoopbackToUSB,
    DigDCProtection,
    RearTRSSource,
    LoudnessEnable,
    LoudnessBassGain,
    LoudnessTrebleGain,
    LoudnessLowVolRef,
}

impl TryFrom<u8> for ChannelParameterType {
    type Error = MidiError;

    fn try_from(param: u8) -> Result<Self, Self::Error> {
        match param {
            1 => Ok(ChannelParameterType::Source),
            2 => Ok(ChannelParameterType::RefLevel),
            3 => Ok(ChannelParameterType::AutoRefLevel),
            4 => Ok(ChannelParameterType::Mono),
            5 => Ok(ChannelParameterType::Width),
            6 => Ok(ChannelParameterType::MSProc),
            7 => Ok(ChannelParameterType::Polarity),
            8 => Ok(ChannelParameterType::Crossfeed),
            9 => Ok(ChannelParameterType::DAFilter),
            10 => Ok(ChannelParameterType::DeEmphasis),
            11 => Ok(ChannelParameterType::DualEQ),
            12 => Ok(ChannelParameterType::Volume),
            13 => Ok(ChannelParameterType::LockVolume),
            14 => Ok(ChannelParameterType::Balance),
            15 => Ok(ChannelParameterType::Mute),
            16 => Ok(ChannelParameterType::Dim),
            17 => Ok(ChannelParameterType::LoopbackToUSB),
            18 => Ok(ChannelParameterType::DigDCProtection),
            19 => Ok(ChannelParameterType::RearTRSSource),
            20 => Ok(ChannelParameterType::LoudnessEnable),
            21 => Ok(ChannelParameterType::LoudnessBassGain),
            22 => Ok(ChannelParameterType::LoudnessTrebleGain),
            23 => Ok(ChannelParameterType::LoudnessLowVolRef),
            param => Err(MidiError::UnknownChannelParameter(param)),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, Hash, PartialEq)]
pub enum DeviceParameterType {
    MuteLineVs = 1,
    AutoStandby = 2,
    DSDDetection = 3,
    DSDFilter = 4,
    DSDDirectLine = 5,
    BasicMode = 6,
    DigitalOutSource = 7,
    DualPhones = 8,
    BalTRSPhonesMode = 9,
    TogglePhonesLine = 10,
    MuteLineVsPH12 = 11,
    MuteLineVsPH34 = 12,
    ClockSource = 15,
    SampleRate = 16,
    RemapKeysIR5 = 22,
    RemapKeysIR6 = 23,
    RemapKeysIR7 = 24,
    RemapKeys = 25,
    RemapKeysVOL = 26,
    RemapKeysIO = 27,
    RemapKeysEQ = 28,
    RemapKeysSETUP = 29,
    DisplayMode = 32,
    MeterColor = 33,
    HorMeter = 34,
    AutoDarkMode = 35,
    ShowVolScreen = 36,
    LockUI = 37,
    SPDIFInput = 40,
    SRCMode = 41,
    SRCGainDig = 42,
    OpticalOutSource = 43,
}

impl TryFrom<u8> for DeviceParameterType {
    type Error = MidiError;

    fn try_from(param: u8) -> Result<Self, Self::Error> {
        match param {
            1 => Ok(DeviceParameterType::MuteLineVs),
            2 => Ok(DeviceParameterType::AutoStandby),
            3 => Ok(DeviceParameterType::DSDDetection),
            4 => Ok(DeviceParameterType::DSDFilter),
            5 => Ok(DeviceParameterType::DSDDirectLine),
            6 => Ok(DeviceParameterType::BasicMode),
            7 => Ok(DeviceParameterType::DigitalOutSource),
            8 => Ok(DeviceParameterType::DualPhones),
            9 => Ok(DeviceParameterType::BalTRSPhonesMode),
            10 => Ok(DeviceParameterType::TogglePhonesLine),
            11 => Ok(DeviceParameterType::MuteLineVsPH12),
            12 => Ok(DeviceParameterType::MuteLineVsPH34),
            15 => Ok(DeviceParameterType::ClockSource),
            16 => Ok(DeviceParameterType::SampleRate),
            22 => Ok(DeviceParameterType::RemapKeysIR5),
            23 => Ok(DeviceParameterType::RemapKeysIR6),
            24 => Ok(DeviceParameterType::RemapKeysIR7),
            25 => Ok(DeviceParameterType::RemapKeys),
            26 => Ok(DeviceParameterType::RemapKeysVOL),
            27 => Ok(DeviceParameterType::RemapKeysIO),
            28 => Ok(DeviceParameterType::RemapKeysEQ),
            29 => Ok(DeviceParameterType::RemapKeysSETUP),
            32 => Ok(DeviceParameterType::DisplayMode),
            33 => Ok(DeviceParameterType::MeterColor),
            34 => Ok(DeviceParameterType::HorMeter),
            35 => Ok(DeviceParameterType::AutoDarkMode),
            36 => Ok(DeviceParameterType::ShowVolScreen),
            37 => Ok(DeviceParameterType::LockUI),
            40 => Ok(DeviceParameterType::SPDIFInput),
            41 => Ok(DeviceParameterType::SRCMode),
            42 => Ok(DeviceParameterType::SRCGainDig),
            43 => Ok(DeviceParameterType::OpticalOutSource),
            param => Err(MidiError::UnknownDeviceParameter(param)),
        }
    }
}
