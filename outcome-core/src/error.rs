//! Defines error types.

use std::fmt::Display;
use std::io;

use crate::address::Address;

#[cfg(feature = "machine")]
use crate::machine;

pub type Result<T> = core::result::Result<T, Error>;

/// Crate-wide error type.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("would block")]
    WouldBlock,

    #[error("io error")]
    IoError(#[from] io::Error),

    #[error("toml deserialization error: {0}")]
    TomlDeserError(#[from] toml::de::Error),
    // #[error("yaml deserialization error")]
    // YamlDeserError(#[from] serde_yaml::Error),
    #[error("semver req parse error")]
    SemverReqParseError(#[from] semver::ReqParseError),
    #[error("semver error")]
    SemverError(#[from] semver::SemVerError),

    #[error("parsing error: {0}")]
    ParsingError(String),

    #[error("failed reading snapshot: {0}")]
    FailedReadingSnapshot(String),
    #[error("failed creating snapshot: {0}")]
    FailedCreatingSnapshot(String),

    #[error("failed reading scenario: missing modules")]
    ScenarioMissingModules,

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
