//! Defines common interface for referencing simulation data.

use std::str::FromStr;

use crate::entity::{Storage, StorageIndex};
use crate::error::{Error, Result};
use crate::StringId;
use crate::{Sim, VarType};

pub const SEPARATOR_SYMBOL: &'static str = ":";

/// Unique reference to simulation data point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Address {
    pub entity: StringId,
    pub component: StringId,
    pub var_type: VarType,
    pub var_id: StringId,
}

impl Address {
    /// Creates a new Address from a &str
    pub fn from_str(mut input: &str) -> Result<Address> {
        let split = input.split(SEPARATOR_SYMBOL).collect::<Vec<&str>>();
        if split.len() != 4 {
            return Err(Error::Other(format!(
                "failed creating address from: {}",
                input
            )));
        }

        unimplemented!()
    }
    pub fn to_string(&self) -> String {
        unimplemented!()
    }
    pub fn get_storage_index(&self) -> StorageIndex {
        unimplemented!()
    }
}
