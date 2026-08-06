use std::{path::PathBuf};

use adi2::{ipc::{GetRequest, IPCError, IPCStream, Request, Response, SetRequest}, midi::ChannelParameterType, state::State};

pub struct Client {
    socket_path: PathBuf,
}

impl Client {
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Client {
            socket_path: socket_path.into(),
        }
    }

    fn send_request(&self, req: Request) -> Result<Response, IPCError> {
        let mut stream = IPCStream::connect(&self.socket_path)?;
        stream.make_request(req)
    }

    pub fn get_state(&self) -> Result<Response, IPCError> {
        self.send_request(Request::Get(GetRequest::State))
    }

    pub fn set_current_channel_parameter_value(&self, _type: ChannelParameterType, value: i16) -> Result<Response, IPCError> {
        self.send_request(Request::Set(SetRequest::CurrentChannelParameter(_type, value)))
    }

    pub fn adjust_current_channel_parameter_value(
        &self,
        _type: ChannelParameterType,
        change: i16,
        min: i16,
        max: i16,
    ) -> Result<Response, IPCError> {
        self.send_request(Request::Set(SetRequest::AdjustCurrentChannelParameter {
            parameter: _type,
            change,
            allowed_range: (min, max),
        }))
    }

    pub fn toggle_current_channel_parameter(&self, _type: ChannelParameterType) -> Result<Response, IPCError> {
        self.send_request(Request::Set(SetRequest::ToggleCurrentChannelParameter(_type)))
    }

    pub fn get_current_channel_parameter_value(&self, _type: ChannelParameterType) -> Result<Option<i16>, IPCError> {
        self.send_request(Request::Get(GetRequest::CurrentChannelParameter(_type))).map(|r| match r {
            Response::ChannelParameter(p) => p,
            _ => None,
        })
    }
}
