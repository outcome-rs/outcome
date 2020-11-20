//! Common interface for referencing simulation data.

use std::str::FromStr;

use crate::entity::{Storage, StorageIndex};
use crate::error::{Error, Result};
use crate::{CompId, StringId};
use crate::{Sim, VarType};

pub const SEPARATOR_SYMBOL: &'static str = ":";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LocalAddress {
    pub comp: Option<StringId>,
    pub var_type: VarType,
    pub var_id: StringId,
}
impl LocalAddress {
    pub fn from_str(input: &str) -> Result<Self> {
        let split = input
            .split(crate::address::SEPARATOR_SYMBOL)
            .collect::<Vec<&str>>();
        if split.len() == 3 {
            unimplemented!()
        } else if split.len() == 2 {
            Ok(LocalAddress {
                comp: None,
                var_type: VarType::from_str(split[0]).unwrap(),
                var_id: StringId::from_truncate(split[1]),
            })
        } else {
            Err(Error::Other(input.to_string()))
        }
    }
    pub fn storage_index(&self) -> Option<StorageIndex> {
        match self.comp {
            Some(c) => Some((c, self.var_id)),
            None => None,
        }
    }
    pub fn storage_index_using(&self, comp_id: CompId) -> StorageIndex {
        (comp_id, self.var_id)
    }
    pub fn to_string(&self) -> String {
        unimplemented!()
    }
}

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
    pub fn from_str(input: &str) -> Result<Address> {
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

/// Partial reference to simulation data point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PartialAddress {
    EntityLocal {
        component: StringId,
        var_type: VarType,
        var_id: StringId,
    },
    ComponentLocal {
        var_type: VarType,
        var_id: StringId,
    },
}

impl PartialAddress {
    pub fn from_str(input: &str) -> Result<Self> {
        let split = input.split(SEPARATOR_SYMBOL).collect::<Vec<&str>>();
        if split.len() == 2 {
            Ok(PartialAddress::ComponentLocal {
                var_type: VarType::from_str(split[0]).unwrap(),
                var_id: StringId::from_truncate(split[1]),
            })
        } else {
            //if split.len() == 3 {
            Ok(PartialAddress::EntityLocal {
                component: StringId::from_truncate(split[0]),
                var_type: VarType::from_str(split[1]).unwrap(),
                var_id: StringId::from_truncate(split[2]),
            })
        }
    }
}
