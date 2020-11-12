#![allow(unused)]

// use crate::msg::server_client::BUF_SIZE;
//use byteorder::ByteOrder;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

pub mod coord_worker;
pub mod server_client;
// pub mod server_client;

use crate::{error::Error, Result};

pub use server_client::*;
use std::convert::TryFrom;

/// Defines a single message, which is a wrapper around a payload.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Message {
    /// Specifies the type of message stored inside the payload
    pub kind: String,
    /// Size of uncompressed payload
    payload_size: u32,
    /// Byte representation of the target message
    payload: Vec<u8>,
}

impl Message {
    /// Creates a complete `Message` from a payload struct, optionally
    /// compressing the payload.
    pub fn from_payload<P>(payload_msg: P, compress: bool) -> Result<Message>
    where
        P: Clone,
        P: Serialize,
        P: Payload,
    {
        let msg_type = payload_msg.kind_str().to_string();
        let (mut payload, payload_size) = pack_payload(payload_msg, compress)?;
        Ok(Message {
            kind: msg_type,
            payload_size,
            payload,
        })
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Message> {
        let mut de = Deserializer::new(bytes);
        let mut msg: Message = Deserialize::deserialize(&mut de)?;
        Ok(msg)
    }

    /// Serialize into bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut mp_buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut mp_buf)).unwrap();
        return mp_buf;
    }

    /// Unpack message payload into a payload struct of provided type.
    /// Returns `None` if unpacking fails.
    pub fn unpack_payload<'de, P>(&'de self) -> Result<P>
    where
        P: Clone,
        P: Deserialize<'de>,
        P: Payload,
    {
        // compressed payload
        if self.payload_size != self.payload.len() as u32 {
            //        println!("compressed size: {}", payload.len());
            //        println!("uncompressed size: {}", uncompressed_size.unwrap() as i32);
            #[cfg(feature = "lz4")]
            let mut decompressed =
                lz4::block::decompress(&self.payload, Some(self.payload_size as i32))
                    .expect("failed decompression err 1");
            #[cfg(feature = "lz4")]
            let mut de = Deserializer::new(&decompressed[..]);
            #[cfg(not(feature = "lz4"))]
            let mut de = Deserializer::new(&self.payload[..]);
            let mut payload_msg: P = Deserialize::deserialize(&mut de)?;
            return Ok(payload_msg);
        } else {
            let mut de = Deserializer::new(&self.payload[..]);
            let mut payload_msg: P = Deserialize::deserialize(&mut de)?;
            return Ok(payload_msg);
        }
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Message> {
        let mut de = Deserializer::new(bytes);
        let msg: Message = Deserialize::deserialize(&mut de)?;
        Ok(msg)
    }
}
/// Pack a payload struct to bytes.
pub(crate) fn pack_payload<P>(payload_msg: P, compress: bool) -> Result<(Vec<u8>, u32)>
where
    P: Clone,
    P: serde::Serialize,
    P: Payload,
{
    let mut buf = Vec::new();
    payload_msg.serialize(&mut Serializer::new(&mut buf))?;
    let uncompressed_size = buf.len() as u32;
    if compress {
        #[cfg(feature = "lz4")]
        return Ok((compress_payload(&buf)?, uncompressed_size));
        #[cfg(not(feature = "lz4"))]
        Ok((buf, uncompressed_size))
    } else {
        Ok((buf, uncompressed_size))
    }
}
#[cfg(feature = "lz4")]
pub(crate) fn compress_payload(bytes: &Vec<u8>) -> Result<Vec<u8>> {
    let compressed = lz4::block::compress(bytes.as_slice(), None, false)?;
    Ok(compressed)
}
/// Unpack bytes into a `Message` object.
/// Returns `None` if unpacking fails.
pub(crate) fn unpack_message(mut bytes: &[u8]) -> Result<Message> {
    use std::io::copy;
    //    let mut de = Deserializer::new(&payload[..]);

    // uncompress
    // use self::lz4::block::decompress;
    // use self::lz4::Decoder;
    //    if use_compression {
    ////        let mut decoder = Decoder::new(bytes.as_slice()).unwrap();
    ////        let mut decoded_msg = Vec::new();
    ////        copy(&mut decoder, &mut decoded_msg);
    //
    //        let mut decoded_msg = Vec::new();
    //        decoded_msg = decompress(bytes.as_slice(), None).expect("failed decompression err 1");
    //
    //        let mut de = Deserializer::new(decoded_msg.as_slice());
    //        let mut msg: Message = match Deserialize::deserialize(&mut de) {
    //            Ok(m) => m,
    //            Err(e) => {
    //                println!("{:?}", e);
    //                return None;
    //            }
    //        };
    //        return Some(msg);
    //    } else {
    let mut de = Deserializer::new(bytes);
    let mut msg: Message = Deserialize::deserialize(&mut de)?;
    return Ok(msg);
    //    }

    //    let msg: Message = match bincode::deserialize(&payload) {
    //        Ok(m) => m,
    //        Err(e) => return None,
    //    };
    //    Some(msg)
}
/// Unpack bytes into a (generic) payload struct.
/// Returns `None` if unpacking fails.
pub(crate) fn unpack_payload<'de, P>(
    payload: &'de [u8],
    compressed: bool,
    uncompressed_size: Option<u32>,
) -> Result<P>
//    where P: Clone, P: serde::Deserialize<'de>, P: PayloadMsg {
where
    P: Clone,
    P: Deserialize<'de>,
    P: Payload,
{
    if compressed {
        //        println!("compressed size: {}", payload.len());
        //        println!("uncompressed size: {}", uncompressed_size.unwrap() as i32);
        #[cfg(feature = "lz4")]
        let mut decompressed =
            lz4::block::decompress(payload, uncompressed_size.map(|num| num as i32))
                .expect("failed decompression err 1");
        #[cfg(feature = "lz4")]
        let mut de = Deserializer::new(&decompressed[..]);
        #[cfg(not(feature = "lz4"))]
        let mut de = Deserializer::new(&payload[..]);
        let mut payload_msg: P = Deserialize::deserialize(&mut de)?;
        return Ok(payload_msg);
    } else {
        let mut de = Deserializer::new(&payload[..]);
        let mut payload_msg: P = Deserialize::deserialize(&mut de)?;
        return Ok(payload_msg);
    }
}

pub trait Payload {
    /// Allows payload message structs to state their message type str.
    fn kind_str(&self) -> &str;
}
