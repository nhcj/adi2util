mod client;

use crate::client::Client;

fn main() -> std::io::Result<()> {
    let client = Client::new("/tmp/adi2d.sock");
    let state = client.get_state()?;
    println!("STATE: {:?}", state);
    Ok(())
}
