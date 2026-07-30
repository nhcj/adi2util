use std::{path::PathBuf};

use adi2::{ipc::{GetRequest, IPCStream, Request, Response, SetRequest}, midi::ChannelParameterType, state::State};

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
        let mut stream = IPCStream::connect(&self.socket_path)?;
        stream.make_request(req)
    }

    pub fn get_state(&self) -> std::io::Result<State> {
        self.send_request(Request::Get(GetRequest::State)).map(|r| {
            match r {
                Response::State(s) => s,
                // TODO
                _ => unreachable!()
            }
        })
    }

    pub fn set_current_channel_parameter_value(&self, _type: ChannelParameterType, value: i16) -> std::io::Result<Response> {
        self.send_request(Request::Set(SetRequest::CurrentChannelParameter(_type, value)))
    }

    pub fn get_current_channel_parameter_value(&self, _type: ChannelParameterType) -> std::io::Result<Option<i16>> {
        self.send_request(Request::Get(GetRequest::CurrentChannelParameter(_type))).map(|r| {
            match r {
                Response::ChannelParameter(p) => p,
                // TODO
                _ => unreachable!()
            }
        })
    }
}
