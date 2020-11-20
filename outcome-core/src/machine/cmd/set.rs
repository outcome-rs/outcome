use super::{Command, CommandResult};
use crate::address::{Address, LocalAddress};
use crate::entity::{Entity, Storage};
use crate::var::{Var, VarType};
use crate::{CompId, EntityId, MedString, StringId};

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
        comp_uid: &CompId,
        location: &LocationInfo,
    ) -> CommandResult {
        *storage.get_int_mut(&self.target).unwrap() = *storage.get_int(&self.source).unwrap();
        CommandResult::Continue
    }
}

/// Generic `set` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Set {
    target: LocalAddress,
    source: SetSource,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SetSource {
    Address(LocalAddress),
    Value(Var),
    None,
}
impl Set {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Command> {
        let target = match LocalAddress::from_str(&args[0]) {
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
                let address = match LocalAddress::from_str(&source_str[1..]) {
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
                    Some(v) => v,
                    None => {
                        return Err(Error::new(
                            *location,
                            ErrorKind::InvalidCommandBody(
                                "can't parse from source into target type".to_string(),
                            ),
                        ))
                    }
                };
                source = SetSource::Value(var);
            }
        }

        // try translating to lower level struct
        if target.var_type == VarType::Int {
            //&& source.var_type.unwrap() == VarType::Int {
            if let SetSource::Address(saddr) = source {
                if saddr.var_type == VarType::Int {
                    let cmd = SetIntIntAddr {
                        target: target.storage_index().unwrap(),
                        source: target.storage_index().unwrap(),
                    };
                    return Ok(Command::SetIntIntAddr(cmd));
                }
            }
        }

        //let source = SetSource::Address(Address::from_str(&args[1]).unwrap());
        Ok(Command::Set(Set { target, source }))
    }
    pub fn execute_loc(
        &self,
        entity_db: &mut Storage,
        ent_name: &EntityId,
        comp_state: &mut StringId,
        comp_uid: &CompId,
        location: &LocationInfo,
    ) -> CommandResult {
        let var_type = &self.target.var_type;
        let target_addr = Address {
            entity: *ent_name,
            component: self.target.comp.unwrap_or(*comp_uid),
            var_type: self.target.var_type,
            var_id: self.target.var_id,
        };
        match &self.source {
            SetSource::Address(addr) => {
                // entity_db.set_from_addr(&self.target, &addr)
                entity_db.set_from_local_addr(&self.target, addr);
            }
            SetSource::Value(val) => {
                if entity_db
                    .get_var_from_addr(&target_addr, Some(comp_uid))
                    .is_some()
                {
                    entity_db.set_from_var(&target_addr, Some(comp_uid), val);
                } else {
                    // find out which comp_uid to use
                    let comp_uid = self.target.comp.unwrap_or(*comp_uid);
                    let var_id = self.target.var_id;
                    entity_db.insert(&comp_uid, &var_id, var_type, val);
                    // return CommandResult::Error(
                    //     Error::FailedGettingFromStorage(self.target, location.clone())
                }
            }
            //TODO return value
            SetSource::None => return CommandResult::Continue,
        }
        CommandResult::Continue
    }
}
