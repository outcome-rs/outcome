use std::collections::BTreeMap;

use shlex;

use crate::{CompName, StringId, VarType};

use crate::address::{Address, PartialAddress, ShortLocalAddress};
use crate::entity::Storage;
use crate::model::ComponentModel;

use super::super::{error::Error, LocationInfo};
use super::CommandResult;
use crate::machine::error::{ErrorKind, Result};
use std::str::FromStr;

/// Print format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintFmt {
    pub fmt: String,
    pub inserts: Vec<(usize, ShortLocalAddress)>,
}

impl PrintFmt {
    pub fn get_type() -> String {
        return "printfmt".to_string();
    }
    pub fn new(args: Vec<String>) -> Result<Self> {
        let matches = getopts::Options::new().parse(args)?;
        let mut fmt = matches.free[0].clone();
        let mut inserts = Vec::new();
        let mut count = 1;
        loop {
            if let Some(index) = fmt.find("{}") {
                //
                fmt = fmt.replacen("{}", "", 1);
                if let Some(addr_str) = matches.free.get(count) {
                    if let Ok(addr) = ShortLocalAddress::from_str(addr_str) {
                        inserts.push((index, addr));
                    }
                }
                count += 1;
            } else {
                break;
            }
            // match fmt.find("{") {
            //     Some(index) => {
            //         let substring_end = match fmt[index..].find("}") {
            //             Some(se) => se,
            //             None => break,
            //         };
            //         let substring = &fmt[index + 1..index + substring_end];
            //         println!("substring: {}", substring);
            //         // inserts.insert(index, Address::from_str(&substring[1..]).unwrap());
            //         // inserts.insert(index, substring[1..].to_string());
            //         if let Some(addr_str) = matches.free.get(count) {
            //             if let Ok(addr) = ShortLocalAddress::from_str(addr_str) {
            //                 inserts.push((index, addr));
            //             }
            //         }
            //
            //         fmt = format!(
            //             "{}{}",
            //             fmt[..index].to_string(),
            //             fmt[substring_end..].to_string()
            //         );
            //         count += 1;
            //     }
            //     None => break,
            // }
        }
        //println!("fmt_string: {}, inserts_map: {:?}", &fmt, &inserts);
        Ok(PrintFmt { fmt, inserts })
    }
}
impl PrintFmt {
    pub fn execute_loc(
        &self,
        entity_db: &mut Storage,
        comp_state: &StringId,
        comp_uid: &CompName,
        location: &LocationInfo,
    ) -> CommandResult {
        //todo
        // unimplemented!()
        if !self.inserts.is_empty() {
            let mut output = self.fmt.clone();
            let mut track_added = 0;
            for (index, addr) in &self.inserts {
                match entity_db.get_var(&addr.storage_index_using(comp_uid.clone())) {
                    Ok(substring) => {
                        let substring = substring.to_string();
                        output.insert_str(*index + track_added, &substring);
                        track_added += substring.len();
                    }
                    Err(e) => {
                        warn!("{}", e)
                    }
                }
            }
            info!("{}", output);
        } else {
            info!("{}", self.fmt);
        }
        CommandResult::Continue
    }
}

/// Print
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "stack_stringid", derive(Copy))]
pub struct Print {
    pub source: Address,
}
impl Print {
    pub fn new(args: Vec<String>) -> Result<Self> {
        let addr = Address::from_str(&args[0]).unwrap();
        Ok(Print { source: addr })
    }
    pub fn from_str(args_str: &str, comp_uid: &CompName) -> Result<Self> {
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
            VarType::String => format!(
                "{}",
                match entity_db.get_var(&self.source.storage_index()) {
                    Ok(v) => v.to_string(),
                    Err(_) => return CommandResult::Break,
                }
            ),
            VarType::Int => format!(
                "{}",
                entity_db
                    .get_var(&self.source.storage_index())
                    .unwrap()
                    .as_int()
                    .unwrap()
            ),
            _ => return CommandResult::Continue,
        };
        debug!("print: {}", print_string);
        CommandResult::Continue
    }
}
