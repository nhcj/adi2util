use std::{io::{Read, Write}, os::unix::net::UnixStream, path::Path};

use crate::{midi::{ChannelAddress, ChannelParameter, ChannelParameterType, DeviceParameter, DeviceParameterType}};
use crate::state::State;

use bitcode::{Decode, Encode};

pub trait IPCMessage<'a>: bitcode::Encode + bitcode::Decode<'a> {}
impl<'a> IPCMessage<'a> for Request {}
impl<'a> IPCMessage<'a> for Response {}

#[derive(Debug, Decode, Encode)]
pub enum Request {
    Get(GetRequest),
    Set(SetRequest),
}

#[derive(Debug, Decode, Encode)]
pub enum GetRequest {
    State,
    CurrentChannelParameter(ChannelParameterType),
    ChannelParameter(ChannelAddress, ChannelParameterType),
    DeviceParameter(DeviceParameterType),
}

#[derive(Debug, Decode, Encode)]
pub enum SetRequest {
    CurrentChannelParameter(ChannelParameterType, i16),
    ChannelParameter(ChannelParameter),
    DeviceParameter(DeviceParameter),
}

#[derive(Debug, Decode, Encode)]
pub enum Response {
    Ok,
    State(State),
    ChannelParameter(Option<i16>),
    DeviceParameter(Option<u16>),
    Err(()),
}

pub const DEFAULT_MAX_MESSAGE_SIZE: u32 = 1<<20;

pub struct IPCStream {
    stream: UnixStream,
    max_message_size: u32,
}

impl IPCStream {
    pub fn new(stream: UnixStream) -> IPCStream {
        Self::new_with_max_size(stream, DEFAULT_MAX_MESSAGE_SIZE)
    }

    pub fn connect<P: AsRef<Path>>(socket_path: P) -> std::io::Result<IPCStream> {
        let stream = UnixStream::connect(socket_path)?;
        Ok(Self::new(stream))
    }

    pub fn new_with_max_size(stream: UnixStream, max_message_size: u32) -> IPCStream {
        IPCStream {
            stream,
            max_message_size,
        }
    }

    pub fn make_request(&mut self, r: Request) -> std::io::Result<Response> {
        self.send_message(r)?;
        self.read_message()
    }

    pub fn read_message<M: for<'a> IPCMessage<'a>>(&mut self) -> std::io::Result<M> {
        let mut size_buffer = [0; 4];
        self.stream.read_exact(&mut size_buffer)?;
        let size = u32::from_be_bytes(size_buffer);
        if size > self.max_message_size {
            unimplemented!()
        }
        let mut data = vec![0; size as usize];
        self.stream.read_exact(&mut data)?;
        Ok(bitcode::decode(&data).unwrap())
    }

    pub fn send_message<'a>(&mut self, data: impl IPCMessage<'a>) -> std::io::Result<()> {
        let data = bitcode::encode(&data);
        let size: u32 = data.len().try_into().unwrap();
        if size > self.max_message_size {
            unimplemented!()
        }
        self.stream.write_all(&size.to_be_bytes())?;
        self.stream.write_all(&data)?;
        Ok(())
    }
}
