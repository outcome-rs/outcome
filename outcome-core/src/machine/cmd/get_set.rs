use std::borrow::Borrow;
use std::collections::HashMap;
use std::str::FromStr;

use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::model::SimModel;
use crate::{model, Var};
use crate::{EntityId, Sim, StringId, VarType};

use super::{Command, CommandResult, ExtCommand};
use crate::machine::error::{Error, ErrorKind};
use crate::machine::Result;

/// Sets var at local address on entity to a value of a var
/// at external address on another entity. Can only be
/// executed during `pre` phase, as it accesses
/// data from another entity (it's an `ExtCommand`).
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Get {
    pub target: Address,
    pub source: Address,
}
impl Get {
    // pub fn from_str(args_str: &str) -> MachineResult<Self> {
    //     let split: Vec<&str> = args_str.split(" ").collect();
    //     // only accepted argument is an address?
    //     if split.len() != 2 {
    //         return Err(MachineError::new(
    //             *location,
    //             MachineErrorKind::Initialization("expected 2 arguments".to_string()),
    //         ));
    //     }
    //
    //     if !split[0].contains("/") {
    //         return Err(MachineError::Initialization(
    //             "first argument invalid".to_string(),
    //         ));
    //     }
    //     let loc_addr = Address::from_str_with_context(split[0].trim(), None, None).unwrap();
    //     let ext_addr = Address::from_str(split[1].trim()).unwrap();
    //
    //     Ok(Get {
    //         target: loc_addr,
    //         source: ext_addr,
    //     })
    // }
    pub fn execute_loc(&self) -> CommandResult {
        CommandResult::ExecExt(ExtCommand::Get(*self))
    }
    pub fn exec_pre(&self, storage: &mut Storage, ent_uid: &EntityId) -> Option<(Address, Var)> {
        unimplemented!();
        // let var = match
        // storage.get_var(&self.source.as_loc()) {
        // Some(v) => v,
        // None => return None,
        //};
        // let (ent_type, ent_id) = ent_uid;
        // return Some((self.target.as_ext(ent_type,
        // ent_id), var));
    }
    //    //TODO it could maybe be faster to not deal with `Var`
    // enum here?
    pub fn execute_ext(&self, sim: &mut Sim, ent_uid: &EntityId) -> Result<()> {
        // println!("{:?}, {:?}", self.source.get_ent_type,
        // self.source.get_ent_id)
        let ext_ent = match sim.get_entity_str(&self.source.entity)
            // .entities
            // .get(&(self.source.get_ent_type(), self.source.get_ent_id()))
        {
            Some(e) => e,
            None => {
                debug!(
                    "executing pre query failed: entity not found: {}",
                    self.source.to_string()
                );
                return Ok(());
            }
        };
        let ext_var = ext_ent
            .storage
            .get_var_from_addr(&self.source, None)
            .unwrap()
            .clone();
        let loc_ent = match sim.get_entity_str_mut(&ent_uid) {
            Some(e) => e,
            None => {
                debug!("failed");
                return Ok(());
            }
        };
        loc_ent.storage.set_from_var(&self.target, None, &ext_var);
        return Ok(());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Set {
    pub var1: Address,
    pub var2: Option<Address>,
    pub val: Option<StringId>,
}
impl Set {
    pub fn from_str(args_str: &str) -> Result<Self> {
        let split: Vec<&str> = args_str.split(" ").collect();
        // only accepted argument is an address?
        if split.len() != 2 {
            // return Err("expected 2 arguments".to_string());
            unimplemented!()
        }

        if !split[0].contains("/") {
            // return Err("first argument invalid".to_string());
            unimplemented!()
        }

        //todo
        unimplemented!()
        // let var1 = Address::from_str_with_context(split[0].trim(), None, None).unwrap();
        //
        // let mut val = None;
        // let mut var2 = None;
        // if split[1].contains("/") {
        //     let ref2 = Address::from_str_with_context(split[1].trim(), None, None).unwrap();
        // } else {
        //     //            val = Some(split[1].to_string());
        //     val = Some(StringId::from(split[1]).unwrap());
        // }
        //
        // Ok(Set { var1, var2, val })
    }
}
impl Set {
    pub fn execute_loc(&self, es: &mut Storage) -> CommandResult {
        if let Some(u) = self.var2 {
            es.set_from_addr(&self.var1, &self.var2.unwrap());
        } else if let Some(v) = &self.val {
            es.set_from_str(&self.var1, v.as_str());
        }
        CommandResult::Continue
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtSetVar {
    pub target: Address,
    pub source: Var,
}
impl ExtSetVar {
    pub fn execute_ext(&self, sim: &mut Sim, ent_uid: &EntityId) -> Result<()> {
        unimplemented!();
        // let ext_ent = match sim
        //.entities
        //.get_mut(&(self.target.get_ent_type(), self.target.get_ent_id()))
        //{
        // Some(e) => e,
        // None => {
        // debug!(
        //"execute ext failed: entity not found: {}",
        // self.target.to_string()
        //);
        // return Ok(());
        //};
        // ext_ent
        //.storage
        //.set_from_var(&self.target.as_loc(), self.source.
        //.set_from_var(&self.target.as_loc(), clone());
        // return Ok(());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ExtSet {
    pub target: Address,
    pub source: Address,
}
impl ExtSet {
    pub fn execute_ext(&self, sim: &mut Sim, ent_uid: &EntityId) -> Result<()> {
        let loc_ent = match sim.get_entity_str(&ent_uid) {
            Some(e) => e,
            None => {
                debug!("");
                return Ok(());
            }
        };
        let loc_var = loc_ent
            .storage
            .get_var_from_addr(&self.source, None)
            .unwrap();
        let ext_ent = match sim.get_entity_str_mut(&(self.target.entity)) {
            Some(e) => e,
            None => {
                debug!(
                    "execute ext failed: entity not found: {}",
                    self.target.to_string()
                );
                return Ok(());
            }
        };
        unimplemented!();
        // ext_ent.storage.set_from_var(&self.target.as_loc(),
        // loc_var);
        return Ok(());
    }
}
