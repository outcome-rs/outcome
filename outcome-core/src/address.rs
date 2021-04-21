//! Common interface for referencing simulation data.

use std::str::FromStr;

use crate::entity::{Storage, StorageIndex};
use crate::error::{Error, Result};
use crate::{arraystring, CompName, EntityName, StringId, VarName};
use crate::{Sim, VarType};

pub const SEPARATOR_SYMBOL: &'static str = ":";

/// Entity-scope address that can also handle component-scope locality.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShortLocalAddress {
    pub comp: Option<CompName>,
    pub var_type: VarType,
    pub var_id: VarName,
}

impl ShortLocalAddress {
    pub fn into_local_address(self, component: Option<CompName>) -> Result<LocalAddress> {
        match self.comp {
            Some(c) => match component {
                Some(_c) => Ok(LocalAddress {
                    comp: _c,
                    var_type: self.var_type,
                    var_id: self.var_id,
                }),
                None => Ok(LocalAddress {
                    comp: c,
                    var_type: self.var_type,
                    var_id: self.var_id,
                }),
            },
            None => match component {
                Some(_c) => Ok(LocalAddress {
                    comp: _c,
                    var_type: self.var_type,
                    var_id: self.var_id,
                }),
                None => Err(Error::Other(
                    "failed making into local address, missing comp name".to_string(),
                )),
            },
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        let split = s
            .split(crate::address::SEPARATOR_SYMBOL)
            .collect::<Vec<&str>>();
        if split.len() == 2 {
            Ok(ShortLocalAddress {
                comp: None,
                var_type: VarType::from_str(split[0])?,
                var_id: arraystring::new_truncate(split[1]),
            })
        } else if split.len() == 3 {
            Ok(ShortLocalAddress {
                comp: Some(arraystring::new_truncate(split[0])),
                var_type: VarType::from_str(split[1])?,
                var_id: arraystring::new_truncate(split[2]),
            })
        } else {
            Err(Error::InvalidLocalAddress(s.to_string()))
        }
    }

    pub fn storage_index(&self, comp_id: Option<CompName>) -> Result<StorageIndex> {
        match comp_id {
            Some(c) => Ok((c, self.var_id)),
            None => match self.comp {
                Some(_c) => Ok((_c, self.var_id)),
                None => Err(Error::Other(
                    "failed making storage index, short local address missing component name"
                        .to_string(),
                )),
            },
        }
    }

    pub fn storage_index_using(&self, comp_id: CompName) -> StorageIndex {
        (comp_id, self.var_id)
    }

    pub fn to_string(&self) -> String {
        match self.comp {
            Some(c) => format!("{}:{}:{}", c, self.var_type.to_str(), self.var_id),
            None => format!("{}:{}", self.var_type.to_str(), self.var_id),
        }
    }
}

/// Entity-scope address.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LocalAddress {
    pub comp: CompName,
    pub var_type: VarType,
    pub var_id: VarName,
}

impl LocalAddress {
    pub fn from_str(s: &str) -> Result<Self> {
        let split = s
            .split(crate::address::SEPARATOR_SYMBOL)
            .collect::<Vec<&str>>();
        if split.len() == 3 {
            Ok(LocalAddress {
                comp: arraystring::new_truncate(split[0]),
                var_type: VarType::from_str(split[1])?,
                var_id: arraystring::new_truncate(split[1]),
            })
        } else {
            Err(Error::InvalidLocalAddress(s.to_string()))
        }
    }
    pub fn storage_index(&self) -> StorageIndex {
        (self.comp, self.var_id)
    }
    pub fn storage_index_using(&self, comp_id: CompName) -> StorageIndex {
        (comp_id, self.var_id)
    }
    pub fn to_string(&self) -> String {
        unimplemented!()
    }
}

/// Globally unique reference to simulation variable.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct Address {
    pub entity: EntityName,
    pub component: CompName,
    pub var_type: VarType,
    pub var_id: VarName,
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

        Ok(Address {
            entity: arraystring::new_truncate(split[0]),
            component: arraystring::new_truncate(split[1]),
            var_type: VarType::from_str(split[2])?,
            var_id: arraystring::new_truncate(split[3]),
        })
    }
    pub fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.entity,
            self.component,
            self.var_type.to_str(),
            self.var_id
        )
    }
    pub fn storage_index(&self) -> StorageIndex {
        (self.component, self.var_id)
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
                var_id: arraystring::new_truncate(split[1]),
            })
        } else {
            //if split.len() == 3 {
            Ok(PartialAddress::EntityLocal {
                component: arraystring::new_truncate(split[0]),
                var_type: VarType::from_str(split[1]).unwrap(),
                var_id: arraystring::new_truncate(split[2]),
            })
        }
    }
}
