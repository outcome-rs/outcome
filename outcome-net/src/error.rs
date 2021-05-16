use crate::msg::{Message, MessageType};
use crate::{msg, Transport};
use num_enum::TryFromPrimitiveError;
use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

/// Enumeration of errors that may occur during network operations.
#[derive(Error, Debug)]
pub enum Error {
    #[error("would block")]
    WouldBlock,
    #[error("read wrong type")]
    ReadWrongType,
    #[error("timed out")]
    TimedOut,
    #[error("host unreachable")]
    HostUnreachable,
    #[error("socket not connected")]
    SocketNotConnected,
    #[error("socket not bound to address")]
    SocketNotBoundToAddress,
    #[error("wrong socket address type")]
    WrongSocketAddressType,
    #[error("handshake failed, got: {0}")]
    HandshakeFailed(String),

    #[error("other: {0}")]
    Other(String),

    #[error("failed parsing int: {0}")]
    IntParseError(#[from] std::num::ParseIntError),
    #[error("failed parsing address: {0}")]
    AddrParseError(#[from] std::net::AddrParseError),
    #[error("transport unavailable: {0}")]
    TransportUnavailable(Transport),

    #[error("no activity for {0} milliseconds, terminating server")]
    ServerKeepaliveLimitReached(u32),

    #[error("data store disconnected")]
    Disconnect(#[from] std::io::Error),

    #[cfg(feature = "laminar_transport")]
    #[error("laminar error")]
    LaminarError(#[from] laminar::ErrorKind),
    #[cfg(feature = "nng_transport")]
    #[error("nng error")]
    NngError(#[from] nng::Error),
    #[cfg(feature = "zmq_transport")]
    #[error("zmq error")]
    ZmqError(#[from] zmq::Error),

    #[error("bincode error")]
    BincodeError(#[from] bincode::Error),

    #[cfg(feature = "msgpack_encoding")]
    #[error("rmp_serde decode error: {0}")]
    RmpsDecodeError(#[from] rmp_serde::decode::Error),
    #[cfg(feature = "msgpack_encoding")]
    #[error("rmp_serde encode error")]
    RmpsEncodeError(#[from] rmp_serde::encode::Error),

    #[cfg(feature = "json_encoding")]
    #[error("serde_json error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("unknown message code: {0}")]
    UnknownMsgCode(#[from] TryFromPrimitiveError<msg::MessageType>),

    #[error("core error")]
    CoreError(#[from] outcome_core::error::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader { expected: String, found: String },
    #[error("unknown error")]
    Unknown,
}
