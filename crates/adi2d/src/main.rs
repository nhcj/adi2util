mod client;
mod state;

use adi2::midi::{ChannelParameterType, Message};
use clap::{Parser, Subcommand};

use crate::state::State;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,
}

fn main() {
    let cli = Cli::parse();

    let state = std::sync::Arc::new(std::sync::RwLock::new(State::new()));
    let state_ = state.clone();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            let mut client = client::MidiClient::from_port_name("ADI-2", move |message, _| {
                let mut state = state_.write().unwrap();
                match message {
                    Message::Status(status) => {
                        state.set_active_output(status.active_output);
                    }
                    Message::Parameters(params) => {
                        for param in params.channel.iter() {
                            state.set_channel_parameter(param);
                        }
                    }
                }
});

            loop {
                println!("{:?}", state.read().unwrap().get_channel_parameter_value(None, ChannelParameterType::Volume));
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }

}
