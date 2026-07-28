use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use adi2::{devices, midi::Message};

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
        client.send_status_request();
        client
    }

    pub fn send_status_request(&mut self) {
        let data = &[0xF0, 0x00, 0x20, 0x0D, 0x71, 0x03, 0x09, 0xF7];
        self.output_conn.send(data).unwrap();
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
