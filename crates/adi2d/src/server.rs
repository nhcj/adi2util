use std::{os::unix::net::UnixListener, path::PathBuf, sync::{Arc, RwLock}};

use adi2::{ipc::{self, GetRequest, Request, Response, SetRequest}, midi::ChannelParameter, state::State};

#[derive(Default)]
pub struct Server {
    socket_path: PathBuf,
    state: Arc<RwLock<State>>,
}

impl Server {
    pub fn new<P: Into<PathBuf>>(socket_path: P, state: Arc<RwLock<State>>) -> Self {
        Server {
            socket_path: socket_path.into(),
            state,
        }
    }

    pub fn start(&mut self) -> std::io::Result<()> {
        let listener = UnixListener::bind(self.socket_path.as_path())?;
        for stream in listener.incoming() {
            let stream = stream?;
            let request = ipc::read_message::<Request>(stream.try_clone()?)?;
            println!("REQ: {:?}", request);
            let response = self.handle_request(request);
            ipc::send_message(stream, response)?;
        }
        Ok(())
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Get(g) => self.handle_get_request(g),
            Request::Set(s) => self.handle_set_request(s),
        }
    }

    fn handle_get_request(&self, get: GetRequest) -> Response {
        let state = self.state.read().unwrap();
        match get {
            GetRequest::State => Response::State(state.clone()),
            GetRequest::ChannelParameter(address, _type) => Response::ChannelParameter(state.get_channel_parameter_value(Some(address), _type)),
            GetRequest::DeviceParameter(_type) => Response::DeviceParameter(state.get_device_parameter_value(_type)),
        }
    }

    fn handle_set_request(&mut self, set: SetRequest) -> Response {
        unimplemented!()
    }
}
