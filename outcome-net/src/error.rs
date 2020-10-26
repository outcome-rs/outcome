use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

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
    #[error("other: {0}")]
    Other(String),

    #[error("data store disconnected")]
    Disconnect(#[from] std::io::Error),

    #[cfg(feature = "transport_nng")]
    #[error("driver error")]
    DriverError(#[from] nng::Error),
    #[cfg(feature = "transport_zmq")]
    #[error("driver error")]
    DriverError(#[from] zmq::Error),

    #[error("rmp_serde decode error")]
    RmpsDecodeError(#[from] rmp_serde::decode::Error),
    #[error("rmp_serde encode error")]
    RmpsEncodeError(#[from] rmp_serde::encode::Error),
    #[error("core error")]
    CoreError(#[from] outcome_core::error::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader { expected: String, found: String },
    #[error("unknown error")]
    Unknown,
}
