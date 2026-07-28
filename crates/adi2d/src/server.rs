use std::{os::unix::net::UnixListener, path::PathBuf};

use adi2::ipc::{self, Request, Response};

#[derive(Default)]
pub struct Server {
    socket_path: PathBuf,
}

impl Server {
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Server {
            socket_path: socket_path.into(),
        }
    }

    pub fn start<F>(&mut self, mut f: F) -> std::io::Result<()>
        where F: FnMut(Request) -> Response
    {
        let listener = UnixListener::bind(self.socket_path.as_path())?;
        for stream in listener.incoming() {
            let stream = stream?;
            let request = ipc::read_message::<Request>(stream.try_clone()?)?;
            println!("REQ: {:?}", request);
            let response = f(request);
            ipc::send_message(stream, response)?;
        }
        Ok(())
    }
}
