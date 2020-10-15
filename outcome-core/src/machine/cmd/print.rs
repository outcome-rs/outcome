//use std::collections::HashMap;
use std::collections::BTreeMap;

use shlex;

use crate::{CompId, MedString, VarType};

use crate::address::Address;
use crate::component::Component;
use crate::entity::Storage;
use crate::model::ComponentModel;

use super::super::{error::Error, LocationInfo};
use super::CommandResult;
use crate::machine::error::ErrorKind;

/// Print format
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrintFmt {
    pub fmt: String,
    pub inserts: BTreeMap<usize, Address>,
}

impl PrintFmt {
    pub fn get_type() -> String {
        return "printfmt".to_string();
    }
    pub fn new(args: Vec<String>) -> Result<Self, Error> {
        let mut fmt = args[0].to_string();
        let mut inserts = BTreeMap::new();
        loop {
            match fmt.find('$') {
                Some(index) => {
                    let substring_end = fmt[index..].find(' ').unwrap_or(fmt.len());
                    let substring = &fmt[index..substring_end];
                    inserts.insert(index, Address::from_str(&substring[1..]).unwrap());
                    fmt = format!(
                        "{}{}",
                        fmt[..index].to_string(),
                        fmt[substring_end..].to_string()
                    );
                }
                None => break,
            }
        }
        //println!("fmt_string: {}, inserts_map: {:?}", &fmt, &inserts);
        Ok(PrintFmt { fmt, inserts })
    }
    pub fn from_str(args_str: &str, comp_uid: &CompId) -> Result<Self, String> {
        let shl_split = match shlex::split(args_str) {
            Some(s) => s,
            None => return Err(format!("failed parsing command arguments: {}", args_str)),
        };

        Ok(PrintFmt {
            fmt: shl_split[0].to_string(),
            inserts: BTreeMap::new(),
        })
    }
}
impl PrintFmt {
    pub fn execute_loc(
        &self,
        entity_db: &mut Storage,
        component: &Component,
        comp_uid: &CompId,
        location: &LocationInfo,
    ) -> CommandResult {
        //todo
        // unimplemented!()
        if !self.inserts.is_empty() {
            let mut output = self.fmt.clone();
            let mut track_added = 0;
            for (index, addr) in &self.inserts {
                let substring = match entity_db.get_coerce_to_string(&addr, Some(&addr.component)) {
                    Some(s) => s,
                    None => {
                        return CommandResult::Err(Error::new(
                            *location,
                            ErrorKind::FailedGettingFromStorage(addr.to_string()),
                        ))
                    }
                };
                output.insert_str(*index + track_added, &substring);
                track_added += substring.len();
            }
            info!("{}", output);
        } else {
            info!("{}", self.fmt);
        }
        CommandResult::Continue
    }
}

/// Print
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Print {
    pub source: Address,
}
impl Print {
    pub fn new(args: Vec<String>) -> Result<Self, Error> {
        let addr = Address::from_str(&args[0]).unwrap();
        Ok(Print { source: addr })
    }
    pub fn from_str(args_str: &str, comp_uid: &CompId) -> Result<Self, String> {
        //todo
        unimplemented!()
        // let split: Vec<&str> = args_str.split(" ").collect();
        // // only accepted argument is an address?
        // if split.len() != 1 {
        //     return Err("got more than one arguments".to_string());
        // }
        //
        // let source =
        //     Address::from_str_with_context(split[0].trim(), None, Some(&comp_uid)).unwrap();
        // // let source = Address::from_str(split[0].trim()).unwrap();
        //
        // Ok(Print { source })
    }
}
impl Print {
    pub fn execute_loc(&self, entity_db: &mut Storage) -> CommandResult {
        //        let evuid =
        // comp.loc_vars.get(self.source).unwrap();
        let print_string = match &self.source.var_type {
            VarType::Str => format!(
                "{}",
                match entity_db.get_str(&self.source.get_storage_index()) {
                    Some(v) => v,
                    None => return CommandResult::Break,
                }
            ),
            VarType::Int => format!(
                "{}",
                entity_db.get_int(&self.source.get_storage_index()).unwrap()
            ),
            _ => return CommandResult::Continue,
        };
        debug!("print: {}", print_string);
        CommandResult::Continue
    }
}
