use super::{Command, CommandResult};
use crate::address::{Address, LocalAddress, ShortLocalAddress};
use crate::entity::{Entity, Storage};
use crate::var::{Var, VarType};
use crate::{CompName, EntityId, EntityName, MedString, StringId};

use super::super::LocationInfo;
use crate::machine::{Error, ErrorKind, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetIntIntAddr {
    target: (StringId, StringId),
    source: (StringId, StringId),
}
impl SetIntIntAddr {
    pub fn execute_loc(
        &self,
        storage: &mut Storage,
        comp_uid: &CompName,
        location: &LocationInfo,
    ) -> CommandResult {
        *storage
            .get_var_mut(&self.target)
            .unwrap()
            .as_int_mut()
            .unwrap() = *storage.get_var(&self.source).unwrap().as_int().unwrap();
        CommandResult::Continue
    }
}

/// Generic `set` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Set {
    target: ShortLocalAddress,
    source: SetSource,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SetSource {
    Address(ShortLocalAddress),
    Value(Var),
    None,
}
impl Set {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Command> {
        let target = match ShortLocalAddress::from_str(&args[0]) {
            Ok(addr) => addr,
            Err(e) => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody(format!(
                        "target argument has to be a valid address: {}",
                        e
                    )),
                ))
            }
        };
        let mut source = SetSource::None;
        let mut source_str = "";
        // is an equals sign '=' present?
        if args.len() > 1 {
            if args[1] == "=" {
                source_str = &args[2];
            } else {
                source_str = &args[1];
            }
            if source_str.starts_with("$") {
                let address = match ShortLocalAddress::from_str(&source_str[1..]) {
                    Ok(addr) => addr,
                    Err(e) => {
                        return Err(Error::new(
                            *location,
                            ErrorKind::InvalidCommandBody(format!(
                                "source argument starts with '$' but the address is invalid: {}",
                                e
                            )),
                        ))
                    }
                };
                source = SetSource::Address(address);
            } else {
                let var = match Var::from_str(source_str, Some(target.var_type)) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(Error::new(
                            *location,
                            ErrorKind::InvalidCommandBody(format!(
                                "can't parse from source into target type: {}",
                                e
                            )),
                        ))
                    }
                };
                source = SetSource::Value(var);
            }
        }

        // // try translating to lower level struct
        // if target.var_type == VarType::Int {
        //     //&& source.var_type.unwrap() == VarType::Int {
        //     if let SetSource::Address(saddr) = source {
        //         if saddr.var_type == VarType::Int {
        //             let cmd = SetIntIntAddr {
        //                 target: target.storage_index(),
        //                 source: target.storage_index(),
        //             };
        //             return Ok(Command::SetIntIntAddr(cmd));
        //         }
        //     }
        // }

        //let source = SetSource::Address(Address::from_str(&args[1]).unwrap());
        Ok(Command::Set(Set { target, source }))
    }
    pub fn execute_loc(
        &self,
        entity_db: &mut Storage,
        ent_uid: &EntityId,
        comp_state: &mut StringId,
        comp_uid: &CompName,
        location: &LocationInfo,
    ) -> CommandResult {
        let var_type = &self.target.var_type;
        let target_addr = Address {
            entity: crate::arraystring::new_truncate(&ent_uid.to_string()),
            component: self.target.comp.unwrap_or(*comp_uid),
            // component: self.target.comp,
            var_type: self.target.var_type,
            var_id: self.target.var_id,
        };
        match &self.source {
            SetSource::Address(addr) => {
                // entity_db.set_from_addr(&self.target, &addr)
                *entity_db
                    .get_var_mut(&self.target.storage_index_using(*comp_uid))
                    .unwrap() = entity_db
                    .get_var(&addr.storage_index_using(*comp_uid))
                    .unwrap()
                    .clone();
            }
            SetSource::Value(val) => {
                if let Ok(target_var) = entity_db.get_var_mut(&target_addr.storage_index()) {
                    *target_var = val.clone();
                } else {
                    entity_db.insert(self.target.storage_index_using(*comp_uid), val.clone());
                }
            }
            //TODO return value
            SetSource::None => return CommandResult::Continue,
        }
        CommandResult::Continue
    }
}
