use std::{io::{Read, Write}, os::unix::net::UnixStream, path::Path, time::Duration};

use crate::midi::{ChannelAddress, ChannelParameter, ChannelParameterType, DeviceParameter, DeviceParameterType};
use crate::state::State;

use bitcode::{Decode, Encode};
use thiserror::Error;

pub trait IPCMessage<'a>: bitcode::Encode + bitcode::Decode<'a> {}
impl<'a> IPCMessage<'a> for Request {}
impl<'a> IPCMessage<'a> for Response {}

#[derive(Debug, Error)]
pub enum IPCError {
    #[error("failed to decode message")]
    MessageDecodeFailed {
        data: Vec<u8>,
    },
    #[error("message of size {size:?} bytes exceeds maximum allowed {max_size:?} bytes")]
    MessageTooLarge {
        size: usize,
        max_size: u32,
    },
    #[error("failed to read or send message size: {0}")]
    ReadOrSendMessageSizeFailed(std::io::Error),
    #[error("failed to read or send message with size {size:?}: {cause:?}")]
    ReadOrSendMessageFailed {
        size: u32,
        cause: std::io::Error,
    },
    #[error("an io error has occurred: {0}")]
    OtherIoError(#[from] std::io::Error),
}

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
    Err(String),
}

pub const DEFAULT_MAX_MESSAGE_SIZE: u32 = 1 << 20;
const DEFAULT_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(1);
const DEFAULT_STREAM_WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct IPCStream {
    stream: UnixStream,
    max_message_size: u32,
}

impl IPCStream {
    pub fn new(stream: UnixStream) -> Result<IPCStream, IPCError> {
        Self::new_with_max_size(stream, DEFAULT_MAX_MESSAGE_SIZE)
    }

    pub fn connect<P: AsRef<Path>>(socket_path: P) -> Result<IPCStream, IPCError> {
        let stream = UnixStream::connect(socket_path)?;
        Self::new(stream)
    }

    pub fn new_with_max_size(stream: UnixStream, max_message_size: u32) -> Result<IPCStream, IPCError> {
        stream.set_read_timeout(Some(DEFAULT_STREAM_READ_TIMEOUT))?;
        stream.set_write_timeout(Some(DEFAULT_STREAM_WRITE_TIMEOUT))?;

        Ok(IPCStream {
            stream,
            max_message_size,
        })
    }

    pub fn make_request(&mut self, r: Request) -> Result<Response, IPCError> {
        self.send_message(r)?;
        self.read_message()
    }

    pub fn read_message<M: for<'a> IPCMessage<'a>>(&mut self) -> Result<M, IPCError> {
        let mut size_buffer = [0; 4];
        self.stream
            .read_exact(&mut size_buffer)
            .map_err(|e| IPCError::ReadOrSendMessageSizeFailed(e))?;
        let size = u32::from_be_bytes(size_buffer);
        if size > self.max_message_size {
            return Err(IPCError::MessageTooLarge {
                size: size as usize,
                max_size: self.max_message_size,
            });
        }
        let mut data = vec![0; size as usize];
        self.stream
            .read_exact(&mut data)
            .map_err(|e| IPCError::ReadOrSendMessageFailed {
                size: size,
                cause: e,
            })?;
        Ok(bitcode::decode(&data).map_err(|_| IPCError::MessageDecodeFailed { data })?)
    }

    pub fn send_message<'a>(&mut self, data: impl IPCMessage<'a>) -> Result<(), IPCError> {
        let data = bitcode::encode(&data);
        let size: u32 = data
            .len()
            .try_into()
            .map_err(|_| IPCError::MessageTooLarge {
                size: data.len(),
                max_size: self.max_message_size,
            })?;
        if size > self.max_message_size {
            return Err(IPCError::MessageTooLarge {
                size: size as usize,
                max_size: self.max_message_size,
            });
        }
        self.stream
            .write_all(&size.to_be_bytes())
            .map_err(|e| IPCError::ReadOrSendMessageSizeFailed(e))?;
        self.stream
            .write_all(&data)
            .map_err(|e| IPCError::ReadOrSendMessageFailed {
                size: size,
                cause: e,
            })?;
        Ok(())
    }
}
