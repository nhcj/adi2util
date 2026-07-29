use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use adi2::{devices, midi::{Message, MidiEncode}};

pub struct MidiClient {
    device: devices::Device,
    input_conn: MidiInputConnection<()>,
    output_conn: MidiOutputConnection,
    last_message_ts: Arc<AtomicU64>,
}

impl MidiClient {
    pub fn from_port_name<F>(name: &str, device: devices::Device, on_message: F) -> Self 
        where F: Fn(Message, u64) + Send + 'static
    {
        let (input, output) = Self::create_midi_io();
        let input_port = first_port_with_name(&input, name).expect("Failed to find matching input port");
        let output_port = first_port_with_name(&output, name).expect("Failed to find matching output port");

        let output_conn = output
            .connect(&output_port, "adi2d output port")
            .expect("Failed to connect to ADI-2 output port");
        let last_message_ts = Arc::new(AtomicU64::new(0));
        let last_message_ts_ = last_message_ts.clone();
        let input_conn = input
            .connect(
                &input_port,
                "adi2d intput port",
                move |ts, data, _| {
                    last_message_ts_.store(ts, Ordering::Relaxed);
                    let message = Message::from_midi_bytes(device, data);
                    if message.is_ok() {
                        on_message(message.unwrap(), ts);
                    }
                },
                ()
            )
            .unwrap();

        let mut client = MidiClient {
            device,
            input_conn,
            output_conn,
            last_message_ts: last_message_ts.clone(),
        };
        client.send_status_command().unwrap();
        client
    }

    fn send_status_command(&mut self) -> Result<(), ()> {
        self.send_command(MidiCommands::RequestToDevice, Some(MidiRequests::Status))
    }

    pub fn send_command(&mut self, cmd: MidiCommands, arg: Option<impl MidiEncode>) -> Result<(), ()> {
        let mut data = self.device.header().to_vec();
        // Seems like we don't (consistently?) get additional status messages
        // back from ADI device after sending commands like set channel parameter,
        // so as a hacky solution (for now?), send a status command after any
        // non-status command
        let mut status_command = false;
        if matches!(cmd, MidiCommands::SendParameterToDevice) && arg.is_none() {
            return Err(());
        }
        data.push(cmd as u8);
        if let Some(arg) = arg {
            let arg = arg.encode().map_err(|_| ())?;
            if arg.len() == 1 && arg[0] == 0x09 { status_command = true; }
            data.extend_from_slice(&arg);
        }
        data.push(0xF7);
        self.output_conn.send(&data).unwrap();
        if !status_command { self.send_status_command()?; }
        Ok(())
    }

    pub fn list_input_ports() {
        let (input, output) = Self::create_midi_io();
        for port in input.ports().iter() {
            println!(
                "Input {} - {}",
                port.id(),
                input.port_name(port).unwrap(),
            );
        }
    }

    fn create_midi_io() -> (MidiInput, MidiOutput) {
        let midi_input =
            MidiInput::new("adi2d").expect("Failed to create MIDI Input");
        let midi_output =
            MidiOutput::new("adi2d").expect("Failed to create MIDI Output");

        (midi_input, midi_output)
    }
}

fn first_port_with_name<T: MidiIO>(io: &T, name: &str) -> Option<T::Port> {
    io.ports().into_iter().find(|p| {
        io.port_name(p)
            .map(|n| n.starts_with(name))
            .unwrap_or(false)
    })
}

#[repr(u8)]
#[non_exhaustive]
pub enum MidiCommands {
    SendParameterToDevice = 0x02,
    RequestToDevice = 0x03,
}

#[non_exhaustive]
pub enum MidiRequests {
    Status
}

impl MidiEncode for MidiRequests {
    type Error = ();

    fn encode(&self) -> Result<Vec<u8>, Self::Error> {
        match self {
            MidiRequests::Status => Ok(vec![0x09]),
        }
    }
}
