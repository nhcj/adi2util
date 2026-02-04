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
enum Parameters {
    Channel(ChannelParameters),
    //Device(DeviceParameters),
}

#[derive(Debug)]
struct ChannelParameters {
    parameters: Vec<ChannelParameter>,
}

#[derive(Debug)]
struct ChannelParameter {
    address: ChannelAddress,
    parameter_value: Option<ChannelParameterValue>,
}

#[derive(Debug)]
enum ChannelParameterValue {
    Source(i16),
    RefLevel(i16),
    AutoRefLevel(i16),
    Mono(i16),
    Width(i16),
    MSProc(i16),
    Polarity(i16),
    Crossfeed(i16),
    DAFilter(i16),
    DeEmphasis(i16),
    DualEQ(i16),
    Volume(i16),
    LockVolume(i16),
    Balance(i16),
    Mute(i16),
    Dim(i16),
    LoopbackToUSB(i16),
    DigDCProtection(i16),
    RearTRSSource(i16),
    LoudnessEnable(i16),
    LoudnessBassGain(i16),
    LoudnessTrebleGain(i16),
    LoudnessLowVolRef(i16),
}

#[derive(Clone, Debug)]
enum ChannelAddress {
    Output(Output),
    Unsupported(u8),
    Unexpected(u8),
}

impl TryFrom<(u8, i16)> for ChannelParameterValue {
    type Error = &'static str;
    fn try_from(param: (u8, i16)) -> Result<Self, Self::Error> {
        match param.0 {
            1 => Ok(ChannelParameterValue::Source(param.1)),
            2 => Ok(ChannelParameterValue::RefLevel(param.1)),
            3 => Ok(ChannelParameterValue::AutoRefLevel(param.1)),
            4 => Ok(ChannelParameterValue::Mono(param.1)),
            5 => Ok(ChannelParameterValue::Width(param.1)),
            6 => Ok(ChannelParameterValue::MSProc(param.1)),
            7 => Ok(ChannelParameterValue::Polarity(param.1)),
            8 => Ok(ChannelParameterValue::Crossfeed(param.1)),
            9 => Ok(ChannelParameterValue::DAFilter(param.1)),
            10 => Ok(ChannelParameterValue::DeEmphasis(param.1)),
            11 => Ok(ChannelParameterValue::DualEQ(param.1)),
            12 => Ok(ChannelParameterValue::Volume(param.1)),
            13 => Ok(ChannelParameterValue::LockVolume(param.1)),
            14 => Ok(ChannelParameterValue::Balance(param.1)),
            15 => Ok(ChannelParameterValue::Mute(param.1)),
            16 => Ok(ChannelParameterValue::Dim(param.1)),
            17 => Ok(ChannelParameterValue::LoopbackToUSB(param.1)),
            18 => Ok(ChannelParameterValue::DigDCProtection(param.1)),
            19 => Ok(ChannelParameterValue::RearTRSSource(param.1)),
            20 => Ok(ChannelParameterValue::LoudnessEnable(param.1)),
            21 => Ok(ChannelParameterValue::LoudnessBassGain(param.1)),
            22 => Ok(ChannelParameterValue::LoudnessTrebleGain(param.1)),
            23 => Ok(ChannelParameterValue::LoudnessLowVolRef(param.1)),
            _ => Err("Invalid channel parameter index"),
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

impl TryFrom<&[u8]> for ChannelParameters {
    type Error = &'static str;
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let parameters_count = data.len() / 3;
        let mut parameters = Vec::new();

        for i in 0..parameters_count {
            let address = match data[i * 3] >> 3 {
                a @ (3 | 6 | 9) => ChannelAddress::Output(Output::try_from((a / 3) - 1)?),
                a @ 1..=14 => ChannelAddress::Unsupported(a),
                a => ChannelAddress::Unexpected(a),
            };

            match address {
                ChannelAddress::Unsupported(_) | ChannelAddress::Unexpected(_) => continue,
                _ => {}
            }

            let first_byte = data[i * 3];
            let second_byte = data[i * 3 + 1];
            let third_byte = data[i * 3 + 2] as i16;

            let index = (first_byte & 0x07) << 2 | (second_byte & 0x60) >> 5;
            // parameter values are signed 12-bit
            let value = match ((second_byte & 0x1F) as i16) << 7 | third_byte {
                value if value & 0x800 != 0 => value | !0xFFF,
                value => value,
            };

            let parameter_value = ChannelParameterValue::try_from((index, value))?;

            parameters.push(ChannelParameter {
                address,
                parameter_value: Some(parameter_value),
            })
        }

        Ok(ChannelParameters { parameters })
    }
}

//impl TryFrom<&[u8]> for DeviceParameter {
//    type Error = &'static str;
//    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
//    }
//}

impl TryFrom<&[u8]> for Parameters {
    type Error = &'static str;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        Ok(Parameters::Channel(ChannelParameters::try_from(data)?))
        //let address = data[0] >> 3;
        //match address {
        //12 => Ok(Parameters::Device(DeviceParameter::try_from(data)?)),
        //_ => Ok(Parameters::Channel(ChannelParameters::try_from(data)?)),
        //}
    }
}

//struct DeviceParameter {}

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
