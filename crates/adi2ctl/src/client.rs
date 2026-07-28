use std::{os::unix::net::UnixStream, path::PathBuf};

use adi2::{ipc::{self, GetRequest, Request, Response}, state::State};

pub struct Client {
    socket_path: PathBuf,
}

impl Client {
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Client {
            socket_path: socket_path.into(),
        }
    }

    fn send_request(&self, req: Request) -> std::io::Result<Response> {
        let stream = UnixStream::connect(&self.socket_path)?;
        ipc::send_message(stream.try_clone()?, req)?;
        ipc::read_message(stream)
    }

    pub fn get_state(&self) -> std::io::Result<State> {
        self.send_request(Request::Get(GetRequest::State)).map(|r| {
            match r {
                Response::State(s) => s,
                _ => unimplemented!()
            }
        })
    }
}
