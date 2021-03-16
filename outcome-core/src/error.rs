//! Error types.

use std::fmt::Display;
use std::io;
use std::num::{ParseFloatError, ParseIntError};
use std::str::ParseBoolError;

use crate::address::Address;

#[cfg(feature = "machine")]
use crate::machine;
use crate::{CompName, EntityName};

pub type Result<T> = core::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

/// Crate-wide error type.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("would block")]
    WouldBlock,
    #[error("would block")]
    NetworkError(String),

    // IoError(#[from] io::Error),
    #[error("io error: {0}")]
    IoError(String),

    #[cfg(feature = "yaml")]
    #[error("yaml deserialization error")]
    YamlDeserError(#[from] serde_yaml::Error),
    #[error("toml deserialization error: {0}")]
    TomlDeserError(#[from] toml::de::Error),
    #[error("semver req parse error")]
    SemverReqParseError(#[from] semver::ReqParseError),
    #[error("semver error")]
    SemverError(#[from] semver::SemVerError),

    #[error("parsing error: {0}")]
    ParsingError(String),
    #[error("failed parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("failed parsing float: {0}")]
    ParseFloatError(#[from] ParseFloatError),
    #[error("failed parsing bool: {0}")]
    ParseBoolError(#[from] ParseBoolError),

    #[error("invalid var type: {0}")]
    InvalidVarType(String),
    #[error("invalid local address: {0}")]
    InvalidAddress(String),
    #[error("invalid local address: {0}")]
    InvalidLocalAddress(String),

    #[error("failed reading snapshot: {0}")]
    FailedReadingSnapshot(String),
    #[error("failed creating snapshot: {0}")]
    FailedCreatingSnapshot(String),

    #[error("failed reading scenario: missing modules")]
    ScenarioMissingModules,

    #[error("model: no entity prefab named: {0}")]
    NoEntityPrefab(EntityName),
    #[error("model: no component named: {0}")]
    NoComponentModel(CompName),

    #[error("no entity found: {0}")]
    NoEntity(u32),
    #[error("no entity found: {0}")]
    NoEntityIndexed(String),
    #[error("failed getting variable: {0}")]
    FailedGettingVariable(String),

    #[error("other error: {0}")]
    Other(String),
    #[cfg(feature = "machine")]
    #[error("runtime machine panic")]
    MachinePanic(#[from] machine::Error),
}

// impl Display for Error {
//     /// Formats the script error using the given formatter.
//     fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
//         match self {
//             Error::FailedReadingSnapshot(ref msg) => {
//                 writeln!(formatter, "Error reading snapshot: {}", msg)?;
//                 Ok(())
//             }
//             Error::Other(ref msg) => write!(formatter, "{}", msg),
//             #[cfg(feature = "machine")]
//             Error::Machine(ref me) => write!(formatter, "{}", me),
//         }
//     }
// }
