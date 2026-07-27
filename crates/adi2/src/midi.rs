trait ADIMidiParameter: Sized {
    type Error;

    // Address is not applicable to device parameters, so it is always None with
    // them and always Some with channel parameters.
    fn from_midi_bytes(address: Option<u8>, index: u8, value: ParameterValue) -> Result<Self, Self::Error>;

    fn to_midi_bytes(&self) -> Result<[u8; 3], Self::Error>;
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum Output {
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
    type Error = ();

    fn try_from(output: u8) -> Result<Self, ()> {
        match output & 0x03 {
            0 => Ok(Output::Line),
            1 => Ok(Output::Phones),
            2 => Ok(Output::IEMs),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
enum ParameterValue {
    ChannelValue(i16),
    DeviceValue(u16),
}

#[derive(Debug)]
enum Message {
    Status(StatusMessage),
    Parameters(Parameters),
}

#[derive(Debug)]
struct StatusMessage {
    active_output: Output,
}

#[derive(Debug)]
struct Parameters {
    channel: Vec<ChannelParameter>,
    device: Vec<DeviceParameter>,
}

#[derive(Debug)]
struct ChannelParameter {
    address: ChannelAddress,
    parameter: ChannelParameterType,
    value: i16,
}

impl ADIMidiParameter for ChannelParameter {
    type Error = ();

    fn from_midi_bytes(address: Option<u8>, index: u8, value: ParameterValue) -> Result<Self, ()> {
        let address = match address.expect("address is always provided for channel parameters") {
            a @ (3|6|9) => ChannelAddress::Output(Output::try_from((a / 3) - 1)?),
            a @ 1..=14 => ChannelAddress::Unsupported(a),
            a => ChannelAddress::Unexpected(a),
        };
        let parameter = ChannelParameterType::try_from(index)?;
        let ParameterValue::ChannelValue(value) = value else { unreachable!() };

        Ok(ChannelParameter {
            address,
            parameter,
            value,
        })
    }

    fn to_midi_bytes(&self) -> Result<[u8; 3], ()> {
        let address = if let ChannelAddress::Output(output) = self.address {
            output.channel_address()
        } else {
            return Err(());
        };
        let index = self.parameter as u8;
        let first_byte = (address << 3) | (index & 0x1C) >> 2;
        let second_byte = (index & 0x03) << 5 | ((self.value & 0x800) >> 7) as u8 | ((self.value >> 7) & 0xF) as u8;
        let third_byte = (self.value & 0x7F) as u8;
        Ok([first_byte, second_byte, third_byte])
    }
}


#[derive(Clone, Debug)]
enum ChannelAddress {
    Output(Output),
    Unsupported(u8),
    Unexpected(u8),
}

#[derive(Debug)]
struct DeviceParameter {
    parameter: DeviceParameterType,
    value: u16,
}

impl ADIMidiParameter for DeviceParameter {
    type Error = ();

    fn from_midi_bytes(_: Option<u8>, index: u8, value: ParameterValue) -> Result<Self, Self::Error> {
        let parameter = DeviceParameterType::try_from(index)?;
        let ParameterValue::DeviceValue(value) = value else { unreachable!() };

        Ok(DeviceParameter {
            parameter,
            value,
        })
    }

    fn to_midi_bytes(&self) -> Result<[u8; 3], Self::Error> {
        // device parameter values are unsigned 11-bit
        if self.value > 0x7FF {
            return Err(());
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
#[derive(Clone, Copy, Debug)]
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
    type Error = ();
    fn try_from(param: u8) -> Result<Self, Self::Error> {
        if param >= ChannelParameterType::Source as u8
            && param <= ChannelParameterType::LoudnessLowVolRef as u8
        {
            return Ok(unsafe { std::mem::transmute(param) });
        }
        Err(())
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
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
    type Error = ();
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
            _ => Err(()),
        }
    }
}
