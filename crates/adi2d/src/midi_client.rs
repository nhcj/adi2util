use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection
};
use thiserror::Error;
use std::{sync::{Arc, Mutex, PoisonError, atomic::{AtomicU64, Ordering}}, time::Duration};

use adi2::{devices::{self, Device}, midi::{self, Message, MidiCommand, MidiRequests, encode_sysex}};
use log::error;

#[derive(Debug, Error)]
pub enum MidiClientError {
    #[error("encoding message failed: {0}")]
    MidiEncodingError(#[from] midi::MidiError),
    #[error("failed to send message: {0}")]
    SendError(#[from] midir::SendError),
}

pub struct MidiClient {
    device: devices::Device,
    input_conn: MidiInputConnection<()>,
    output_conn: Arc<Mutex<MidiOutputConnection>>,
    last_message_ts: Arc<AtomicU64>,
    port_name: String,
}

impl MidiClient {
    pub fn from_port_name<F>(name: &str, device: devices::Device, on_message: F) -> Self 
        where F: Fn(Message, u64) + Send + 'static
    {
        let (input, output) = Self::create_midi_io();
        let (input_port, input_port_name) = first_port_with_name(&input, name).expect("Failed to find matching input port");
        let (output_port, _name) = first_port_with_name(&output, name).expect("Failed to find matching output port");

        let output_conn = output
            .connect(&output_port, "adi2d output port")
            .expect("Failed to connect to ADI-2 output port");
        let output_conn = Arc::new(Mutex::new(output_conn));
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

        let client = MidiClient {
            device,
            input_conn,
            output_conn,
            last_message_ts: last_message_ts.clone(),
            port_name: input_port_name,
        };
        client.spawn_status_poller(Duration::from_secs(1));
        client
    }

    fn spawn_status_poller(&self, poll_interval: Duration) {
        let device = self.device;
        let output_conn = self.output_conn.clone();
        std::thread::spawn(move || loop {
            if let Err(e) = Self::_send_command(device, &output_conn, &MidiRequests::Status) {
                error!("error polling status: {:?}", e);
            }
            std::thread::sleep(poll_interval);
        });
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    fn _send_command<T: MidiCommand>(device: Device, output_conn: &Mutex<MidiOutputConnection>, cmd: &T) -> Result<(), MidiClientError> {
        let data = encode_sysex(device, cmd)?;
        output_conn
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(&data)?;
        Ok(())
    }

    pub fn send_command<T: MidiCommand>(&mut self, cmd: &T) -> Result<(), MidiClientError> {
        Self::_send_command(self.device, &self.output_conn, cmd)
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

fn first_port_with_name<T: MidiIO>(io: &T, name: &str) -> Option<(T::Port, String)> {
    io.ports().into_iter().find_map(|p| {
        io.port_name(&p)
            .ok()
            .filter(|n| n.starts_with(name))
            .map(|n| (p, n))
    })
}
