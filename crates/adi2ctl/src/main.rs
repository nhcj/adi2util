mod client;

use std::{env, fs, process};
use adi2::{ipc::{Request, SetRequest}, midi::ChannelParameterType};
use crate::client::Client;

fn main() -> std::io::Result<()> {
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR not set");
    let runtime_path = format!("{}/adi2d", xdg_runtime_dir);
    let socket_path = format!("{}/socket", &runtime_path);
    if !fs::exists(&socket_path).expect("Failed to check if adi2d socket exists") {
        eprintln!("adi2d is not running; no socket found in {}", runtime_path);
        process::exit(1);
    }

    let client = Client::new(socket_path);
    Ok(())
}
