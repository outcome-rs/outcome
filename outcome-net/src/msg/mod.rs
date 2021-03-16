//! Message definitions.

#![allow(unused)]

use std::convert::{TryFrom, TryInto};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

pub mod coord_worker;
pub mod server_client;

pub use server_client::*;

use crate::socket::Encoding;
use crate::{error::Error, Result};

/// Enumeration of all available message types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, TryFromPrimitive, Deserialize, Serialize)]
pub enum MessageType {
    Bytes,
    Heartbeat,
    Disconnect,
    Connect,

    PingRequest,
    PingResponse,

    RegisterClientRequest,
    RegisterClientResponse,

    IntroduceCoordRequest,
    IntroduceCoordResponse,
    IntroduceWorkerToCoordRequest,
    IntroduceWorkerToCoordResponse,

    ExportSnapshotRequest,
    ExportSnapshotResponse,

    RegisterRequest,
    RegisterResponse,

    StatusRequest,
    StatusResponse,

    DataTransferRequest,
    DataTransferResponse,
    TypedDataTransferRequest,
    TypedDataTransferResponse,

    DataPullRequest,
    DataPullResponse,
    TypedDataPullRequest,
    TypedDataPullResponse,

    ScheduledDataTransferRequest,
    ScheduledDataTransferResponse,

    TurnAdvanceRequest,
    TurnAdvanceResponse,

    SpawnEntitiesRequest,
    SpawnEntitiesResponse,
}

/// Self-describing message wrapping a payload.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Message {
    /// Describes what is stored within the payload
    pub type_: u8,
    /// Byte representation of the target message
    pub payload: Vec<u8>,
}

/// Takes a payload struct and turns it directly into a serialized message.
pub(crate) fn msg_bytes_from_payload<P>(payload: P, encoding: &Encoding) -> Result<Vec<u8>>
where
    P: Serialize,
    P: Payload,
{
    match encoding {
        Encoding::Bincode => {
            let type_ = payload.type_();
            let payload_bytes = pack_payload(payload, encoding)?;
            let msg_bytes = prefix_with_msg_code(payload_bytes, type_);
            Ok(msg_bytes)
        }
        #[cfg(feature = "msgpack_encoding")]
        Encoding::MsgPack => {
            let type_ = payload.type_();
            let payload_bytes = pack_payload(payload, encoding)?;
            let msg = Message {
                type_: type_ as u8,
                payload: payload_bytes,
            };
            let msg_bytes = pack(msg, encoding)?;
            // let msg_bytes = prefix_with_msg_code(payload_bytes, type_);
            Ok(msg_bytes)
        }
        _ => unimplemented!(),
    }
}

impl Message {
    /// Creates a complete `Message` from a payload struct.
    pub fn from_payload<P>(payload: P, encoding: &Encoding) -> Result<Message>
    where
        P: Clone,
        P: Serialize,
        P: Payload,
    {
        let msg_type = payload.type_();
        let bytes = pack_payload(payload, encoding)?;
        Ok(Message {
            type_: msg_type as u8,
            payload: bytes,
        })
    }

    /// Deserializes from bytes.
    pub fn from_bytes(mut bytes: Vec<u8>) -> Result<Message> {
        let type_ = bytes.remove(0);
        let msg: Message = Message {
            type_: MessageType::try_from(type_)? as u8,
            payload: bytes,
        };
        Ok(msg)
    }

    /// Serializes into bytes.
    pub fn to_bytes(mut self) -> Result<Vec<u8>> {
        // let bytes = prefix_with_msg_code(self.payload, self.type_);
        self.payload.insert(0, self.type_ as u8);
        Ok(self.payload)
    }

    /// Unpacks message payload into a payload struct of provided type.
    pub fn unpack_payload<'de, P: Payload + Deserialize<'de>>(
        &'de self,
        encoding: &Encoding,
    ) -> Result<P> {
        let unpacked = unpack(&self.payload, encoding)?;
        Ok(unpacked)
    }
}

/// Prefixes a payload with a one byte code representing message type.
pub(crate) fn prefix_with_msg_code(mut payload: Vec<u8>, type_: MessageType) -> Vec<u8> {
    payload.insert(0, type_ as u8);
    payload
}

/// Packs a payload struct to bytes.
pub(crate) fn pack_payload<P: Payload + Serialize>(
    payload: P,
    encoding: &Encoding,
) -> Result<Vec<u8>> {
    let packed = pack(payload, encoding)?;
    Ok(packed)
}

/// Packs serializable object to bytes based on selected encoding.
pub(crate) fn pack<S: Serialize>(obj: S, encoding: &Encoding) -> Result<Vec<u8>> {
    let packed: Vec<u8> = match encoding {
        Encoding::Bincode => bincode::serialize(&obj)?,
        #[cfg(feature = "msgpack_encoding")]
        Encoding::MsgPack => {
            let mut buf = Vec::new();
            obj.serialize(&mut rmp_serde::Serializer::new(&mut buf))?;
            buf
        }
        #[cfg(feature = "json_encoding")]
        Encoding::Json => unimplemented!(),
    };
    Ok(packed)
}

/// Unpacks message payload into a payload struct of provided type.
pub fn unpack<'de, P: Deserialize<'de>>(bytes: &'de [u8], encoding: &Encoding) -> Result<P> {
    let unpacked = match encoding {
        Encoding::Bincode => bincode::deserialize(bytes)?,
        #[cfg(feature = "msgpack_encoding")]
        Encoding::MsgPack => {
            // println!("{:?}", bytes);
            let mut de = rmp_serde::Deserializer::new(bytes);
            Deserialize::deserialize(&mut de)?
        }
        #[cfg(feature = "json_encoding")]
        Encoding::Json => unimplemented!(),
    };
    Ok(unpacked)
}

// TODO allow for different compression modes
/// Compress bytes using lz4.
#[cfg(feature = "lz4")]
pub(crate) fn compress(bytes: &Vec<u8>) -> Result<Vec<u8>> {
    let compressed = lz4::block::compress(bytes.as_slice(), None, true)?;
    Ok(compressed)
}

pub trait Payload: Clone {
    /// Allows payload message structs to state their message type.
    fn type_(&self) -> MessageType;
}
