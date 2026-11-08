use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection, MidiOutputPort
};
use thiserror::Error;
use std::{process, sync::{Arc, Mutex, PoisonError}, thread, time::{Duration, Instant}};

use adi2::{devices::{self, Device}, midi::{self, Message, MidiCommand, MidiRequests, encode_sysex}};
use log::{error, info};

use crate::config::MidiConfig;

const CLIENT_NAME: &str = "adi2d";

#[derive(Debug, Error)]
pub enum MidiClientError {
    #[error("encoding message failed: {0}")]
    MidiEncodingError(#[from] midi::MidiError),
    #[error("failed to send message: {0}")]
    SendError(#[from] midir::SendError),
    #[error("device is not connected")]
    NotConnected,
}

pub enum MidiEvent {
    Message(Message),
    Disconnected,
}

type EventHandler = Arc<dyn Fn(MidiEvent) + Send + Sync>;
type SharedOutput = Arc<Mutex<Option<MidiOutputConnection>>>;

pub struct MidiClient {
    device: devices::Device,
    output_conn: SharedOutput,
}

impl MidiClient {
    pub fn from_port_name<F>(name: &str, device: devices::Device, config: MidiConfig, on_event: F) -> Self
        where F: Fn(MidiEvent) + Send + Sync + 'static
    {
        let output_conn: SharedOutput = Arc::new(Mutex::new(None));
        let port_prefix = name.to_string();
        let on_event: EventHandler = Arc::new(on_event);
        let link_output_conn = output_conn.clone();
        // MIDI clients are created on the link thread so they don't need to be Send
        thread::spawn(move || Link::new(port_prefix, device, config, on_event, link_output_conn).run());

        MidiClient {
            device,
            output_conn,
        }
    }

    fn _send_command<T: MidiCommand>(device: Device, output_conn: &Mutex<Option<MidiOutputConnection>>, cmd: &T) -> Result<(), MidiClientError> {
        let data = encode_sysex(device, cmd)?;
        output_conn
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
            .ok_or(MidiClientError::NotConnected)?
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

// runs on its own thread: connects when the device shows up, polls it, and
// disconnects (or exits, if configured) when it goes quiet
struct Link {
    port_prefix: String,
    device: Device,
    poll_interval: Duration,
    scan_interval: Duration,
    response_timeout: Duration,
    exit_when_unavailable: bool,
    on_event: EventHandler,
    output_conn: SharedOutput,
    last_message_at: Arc<Mutex<Instant>>,
    // only used to list ports, and kept for the whole run since CoreMIDI may shut
    // down the MIDI server when an app disposes its last client
    scan_input: MidiInput,
    scan_output: MidiOutput,
    input_conn: Option<MidiInputConnection<()>>,
}

impl Link {
    fn new(port_prefix: String, device: Device, config: MidiConfig, on_event: EventHandler, output_conn: SharedOutput) -> Self {
        let (scan_input, scan_output) = match (MidiInput::new(CLIENT_NAME), MidiOutput::new(CLIENT_NAME)) {
            (Ok(input), Ok(output)) => (input, output),
            (Err(e), _) | (_, Err(e)) => {
                error!(target: "midi_client", "Failed to create MIDI client: {}", e);
                process::exit(1);
            }
        };

        Link {
            port_prefix,
            device,
            poll_interval: Duration::from_millis(config.poll_interval),
            scan_interval: Duration::from_millis(config.scan_interval),
            response_timeout: Duration::from_millis(config.response_timeout),
            exit_when_unavailable: config.exit_when_unavailable,
            on_event,
            output_conn,
            last_message_at: Arc::new(Mutex::new(Instant::now())),
            scan_input,
            scan_output,
            input_conn: None,
        }
    }

    fn run(mut self) {
        loop {
            if self.input_conn.is_none() {
                self.try_connect();
            }
            if self.input_conn.is_some() {
                if let Err(e) = MidiClient::_send_command(self.device, &self.output_conn, &MidiRequests::Status) {
                    error!(target: "midi_client", "Error polling status: {}", e);
                }
            }
            thread::sleep(if self.input_conn.is_some() { self.poll_interval } else { self.scan_interval });

            let silent_for = self.last_message_at.lock().unwrap_or_else(PoisonError::into_inner).elapsed();
            if silent_for > self.response_timeout {
                if self.exit_when_unavailable {
                    error!(target: "midi_client", "Device unavailable for {} ms, terminating daemon", silent_for.as_millis());
                    // relies on the service manager (launchd/systemd) to restart the daemon
                    process::exit(1);
                }
                if self.input_conn.is_some() {
                    self.disconnect(silent_for);
                }
            }
        }
    }

    fn try_connect(&mut self) {
        let Some((input_port, port_name)) = first_port_with_name(&self.scan_input, &self.port_prefix) else {
            return;
        };
        let Some((output_port, _)) = first_port_with_name(&self.scan_output, &self.port_prefix) else {
            return;
        };
        match self.open(&input_port, &output_port) {
            Ok((input_conn, output_conn)) => {
                // give the new connection a full timeout to answer
                *self.last_message_at.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
                *self.output_conn.lock().unwrap_or_else(PoisonError::into_inner) = Some(output_conn);
                self.input_conn = Some(input_conn);
                info!(target: "midi_client", "Connected to port '{}'", port_name);
            }
            Err(e) => error!(target: "midi_client", "Failed to connect to port '{}': {}", port_name, e),
        }
    }

    fn open(&self, input_port: &MidiInputPort, output_port: &MidiOutputPort) -> Result<(MidiInputConnection<()>, MidiOutputConnection), Box<dyn std::error::Error>> {
        let output_conn = MidiOutput::new(CLIENT_NAME)?.connect(output_port, "adi2d output port")?;
        let device = self.device;
        let on_event = self.on_event.clone();
        let last_message_at = self.last_message_at.clone();
        let input_conn = MidiInput::new(CLIENT_NAME)?.connect(
            input_port,
            "adi2d input port",
            move |_, data, _| {
                // midir's timestamp uses a backend-specific clock, so record our own
                *last_message_at.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
                if let Ok(message) = Message::from_midi_bytes(device, data) {
                    on_event(MidiEvent::Message(message));
                }
            },
            (),
        )?;
        Ok((input_conn, output_conn))
    }

    fn disconnect(&mut self, silent_for: Duration) {
        *self.output_conn.lock().unwrap_or_else(PoisonError::into_inner) = None;
        // drop this while holding no locks: on ALSA it joins midir's input thread,
        // which may be waiting on the state lock
        self.input_conn = None;
        info!(target: "midi_client", "No response from device for {} ms, disconnected", silent_for.as_millis());
        (self.on_event)(MidiEvent::Disconnected);
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
