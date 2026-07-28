use std::io::{Read, Write};

use crate::{midi::{ChannelAddress, ChannelParameter, ChannelParameterType, DeviceParameter, DeviceParameterType}};
use crate::state::State;

use bitcode::{Decode, Encode};

pub trait Message<'a>: bitcode::Encode + bitcode::Decode<'a> {}
impl<'a> Message<'a> for Request {}
impl<'a> Message<'a> for Response {}

#[derive(Debug, Decode, Encode)]
pub enum Request {
    Get(GetRequest),
    Set(SetRequest),
}

#[derive(Debug, Decode, Encode)]
pub enum GetRequest {
    State,
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

pub fn read_message<M: for<'a> Message<'a>>(mut stream: impl Read) -> std::io::Result<M> {
    let mut size_buffer = [0; 4];
    stream.read_exact(&mut size_buffer)?;
    let size = u32::from_be_bytes(size_buffer) as usize;
    let mut data = vec![0; size];
    stream.read_exact(&mut data)?;
    Ok(bitcode::decode(&data).unwrap())
}

pub fn send_message<'a>(mut stream: impl Write, data: impl Message<'a>) -> std::io::Result<()> {
    let data = bitcode::encode(&data);
    let size: u32 = data.len().try_into().unwrap();
    stream.write_all(&size.to_be_bytes())?;
    stream.write_all(&data)?;
    Ok(())
}
