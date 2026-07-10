use midir::{MidiInput, MidiOutput};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::current,
};

const HEADER: &[u8] = &[0xF0, 0x00, 0x20, 0x0D, 0x71];

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
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
    type Error = &'static str;

    fn try_from(output: u8) -> Result<Self, Self::Error> {
        match output & 0x03 {
            0 => Ok(Output::Line),
            1 => Ok(Output::Phones),
            2 => Ok(Output::IEMs),
            _ => Err("Invalid output value"),
        }
    }
}

#[derive(Debug)]
enum ADI2Message {
    Status(StatusMessage),
    Parameters(Parameters),
}

#[derive(Debug)]
struct StatusMessage {
    active_output: Output,
}

#[derive(Debug)]
struct Parameters {
    channel: Vec<ChannelParameterStatus>,
    device: Vec<DeviceParameterStatus>,
}

#[derive(Debug)]
struct ChannelParameterMessage {
    address: u8,
    index: u8,
    value: i16,
}

#[derive(Debug)]
struct ChannelParameters {
    parameters: Vec<ChannelParameterStatus>,
}

#[derive(Debug)]
struct ChannelParameterStatus {
    address: ChannelAddress,
    parameter: ChannelParameter,
    value: i16,
}

#[derive(Debug)]
struct DeviceParameterMessage {
    index: u8,
    value: u16,
}

#[derive(Debug)]
struct DeviceParameterStatus {
    parameter: DeviceParameter,
    value: u16,
}

#[repr(u8)]
#[derive(Debug)]
enum ChannelParameter {
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

#[derive(Clone, Debug)]
enum ChannelAddress {
    Output(Output),
    Unsupported(u8),
    Unexpected(u8),
}

#[repr(u8)]
#[derive(Debug)]
enum DeviceParameter {
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

impl TryFrom<u8> for ChannelParameter {
    type Error = &'static str;
    fn try_from(param: u8) -> Result<Self, Self::Error> {
        if param >= ChannelParameter::Source as u8
            && param <= ChannelParameter::LoudnessLowVolRef as u8
        {
            return Ok(unsafe { std::mem::transmute(param) });
        }
        Err(("Invalid channel parameter index"))
    }
}

impl TryFrom<u8> for DeviceParameter {
    type Error = &'static str;
    fn try_from(param: u8) -> Result<Self, Self::Error> {
        match param {
            1 => Ok(DeviceParameter::MuteLineVs),
            2 => Ok(DeviceParameter::AutoStandby),
            3 => Ok(DeviceParameter::DSDDetection),
            4 => Ok(DeviceParameter::DSDFilter),
            5 => Ok(DeviceParameter::DSDDirectLine),
            6 => Ok(DeviceParameter::BasicMode),
            7 => Ok(DeviceParameter::DigitalOutSource),
            8 => Ok(DeviceParameter::DualPhones),
            9 => Ok(DeviceParameter::BalTRSPhonesMode),
            10 => Ok(DeviceParameter::TogglePhonesLine),
            11 => Ok(DeviceParameter::MuteLineVsPH12),
            12 => Ok(DeviceParameter::MuteLineVsPH34),
            15 => Ok(DeviceParameter::ClockSource),
            16 => Ok(DeviceParameter::SampleRate),
            22 => Ok(DeviceParameter::RemapKeysIR5),
            23 => Ok(DeviceParameter::RemapKeysIR6),
            24 => Ok(DeviceParameter::RemapKeysIR7),
            25 => Ok(DeviceParameter::RemapKeys),
            26 => Ok(DeviceParameter::RemapKeysVOL),
            27 => Ok(DeviceParameter::RemapKeysIO),
            28 => Ok(DeviceParameter::RemapKeysEQ),
            29 => Ok(DeviceParameter::RemapKeysSETUP),
            32 => Ok(DeviceParameter::DisplayMode),
            33 => Ok(DeviceParameter::MeterColor),
            34 => Ok(DeviceParameter::HorMeter),
            35 => Ok(DeviceParameter::AutoDarkMode),
            36 => Ok(DeviceParameter::ShowVolScreen),
            37 => Ok(DeviceParameter::LockUI),
            40 => Ok(DeviceParameter::SPDIFInput),
            41 => Ok(DeviceParameter::SRCMode),
            42 => Ok(DeviceParameter::SRCGainDig),
            43 => Ok(DeviceParameter::OpticalOutSource),
            _ => Err("Invalid device parameter index"),
        }
    }
}

fn create_bytes(output: Output, index: u8, value: i16) -> [u8; 3] {
    let address = output.channel_address();
    let first_byte = (address << 3) | (index & 0x1C) >> 2;
    let second_byte =
        (index & 0x03) << 5 | ((value & 0x800) >> 7) as u8 | ((value >> 7) & 0xF) as u8;
    let third_byte = (value & 0x7F) as u8;

    [first_byte, second_byte, third_byte]
}

impl TryFrom<DeviceParameterMessage> for DeviceParameterStatus {
    type Error = &'static str;
    fn try_from(msg: DeviceParameterMessage) -> Result<Self, Self::Error> {
        let parameter = DeviceParameter::try_from(msg.index)?;

        Ok(DeviceParameterStatus {
            parameter,
            value: msg.value,
        })
    }
}

impl TryFrom<ChannelParameterMessage> for ChannelParameterStatus {
    type Error = &'static str;
    fn try_from(msg: ChannelParameterMessage) -> Result<Self, Self::Error> {
        let address = match msg.address {
            a @ (3 | 6 | 9) => ChannelAddress::Output(Output::try_from((a / 3) - 1)?),
            a @ 1..=14 => ChannelAddress::Unsupported(a),
            a => ChannelAddress::Unexpected(a),
        };
        let parameter = ChannelParameter::try_from(msg.index)?;

        Ok(ChannelParameterStatus {
            address,
            parameter,
            value: msg.value,
        })
    }
}

impl TryFrom<&[u8]> for Parameters {
    type Error = &'static str;

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

                let msg = DeviceParameterMessage { index, value };
                let device_parameter = DeviceParameterStatus::try_from(msg)?;
                device_parameters.push(device_parameter);
            } else {
                let third_byte = data[i * 3 + 2] as i16;

                let index = (first_byte & 0x07) << 2 | (second_byte & 0x60) >> 5;
                // channel parameter values are signed 12-bit
                let value = match ((second_byte & 0x1F) as i16) << 7 | third_byte {
                    value if value & 0x800 != 0 => value | !0xFFF,
                    value => value,
                };

                let msg = ChannelParameterMessage {
                    address,
                    index,
                    value,
                };
                let channel_parameter = ChannelParameterStatus::try_from(msg)?;
                channel_parameters.push(channel_parameter);
            }
        }
        Ok(Parameters {
            channel: channel_parameters,
            device: device_parameters,
        })
    }
}

impl TryFrom<&[u8]> for ADI2Message {
    type Error = &'static str;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // Bytes 1-4 = always F0 00 20 0D
        // Byte 5 = Device ID (0x71 for ADI-2 FS)
        // Final byte = 0xF7
        if &data[0..5] != HEADER || *data.last().ok_or("Failed to get last")? != 0xF7 {
            return Err("Failed data structure check");
        }

        // Byte 6 = Command
        // Respond only to parameters (0x01) or status (0x07) to remote
        if data[5] != 0x01 && data[5] != 0x07 {
            return Err("Ignored command");
        }

        match data[5] {
            0x07 => Ok(ADI2Message::Status(StatusMessage {
                active_output: Output::try_from(data[6])?,
            })),
            0x01 => Ok(ADI2Message::Parameters(Parameters::try_from(
                &data[6..data.len() - 1],
            )?)),
            _ => unreachable!(),
        }
    }
}

fn main() {
    let midi_input = MidiInput::new("ad2ctl input").expect("Failed to create MIDI Input");
    let midi_output = MidiOutput::new("adi2ctl output").expect("Failed to create MIDI Output");

    //for port in midi_input.ports().iter() {
    //    println!(
    //        "Input {} - {}",
    //        port.id(),
    //        midi_input.port_name(port).unwrap()
    //    );
    //}

    //for port in midi_output.ports().iter() {
    //    println!(
    //        "Output {} - {}",
    //        port.id(),
    //        midi_output.port_name(port).unwrap()
    //    );
    //}

    let input_ports = midi_input.ports();
    let output_ports = midi_output.ports();
    let adi_input_port = input_ports.get(1).expect("Failed to get ADI-2 input port");
    let adi_output_port = output_ports
        .get(1)
        .expect("Failed to get ADI-2 output port");

    let mut adi_output_connection = midi_output
        .connect(adi_output_port, "adi2ctl output port")
        .expect("Failed to connect to ADI-2 output port");
    let last_timestamp = Arc::new(AtomicU64::new(0));
    let last_timestamp_ = last_timestamp.clone();
    let messages = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let messages_ = messages.clone();
    let _input_connection = midi_input
        .connect(
            adi_input_port,
            "adi2ctl input port",
            move |ts, data, _| {
                last_timestamp_.store(ts, Ordering::Relaxed);
                //messages_
                //    .lock()
                //    .expect("Failed to object messages lock")
                //    .push(data.to_vec());
                //println!("{:?}", data);
                println!("{:?}", ADI2Message::try_from(data));
            },
            (),
        )
        .expect("Failed to connect to ADI-2 input port");

    let data = vec![0xF0, 0x00, 0x20, 0x0D, 0x71, 0x03, 0x09, 0xF7];
    //let hehe = create_bytes(Output::Line, 12, -555);
    //println!("created {:X?}", hehe);
    //let data = vec![0xf0, 0x00, 0x20, 0x0d, 0x71, 0x02, 0x1b, 0x1c, 0x11, 0xf7];
    //let mut data = vec![0xf0, 0x00, 0x20, 0x0d, 0x71, 0x02, 0xf7];
    //data.splice(6..6, hehe.into_iter());
    adi_output_connection
        .send(&data)
        .expect("Failed to send status request to ADI-2");

    let mut prev_timestamp = last_timestamp.load(Ordering::Relaxed);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(3000));
        let current_timestamp = last_timestamp.load(Ordering::Relaxed);
        if current_timestamp == prev_timestamp {
            break;
        }
        prev_timestamp = current_timestamp;
        //let data = vec![0xF0, 0x00, 0x20, 0x0D, 0x71, 0x03, 0x09, 0xF7];
        //adi_output_connection
        //    .send(&data)
        //    .expect("Failed to send status request to ADI-2");
    }

    let messages = messages.lock().expect("Failed.");

    println!("{}", messages.len());
}
