use std::collections::HashMap;

use fnv::FnvHashMap;

use crate::address::{Address, LocalAddress};
use crate::error::{Error, Result};
use crate::model::ComponentModel;
use crate::{arraystring, CompName, StringId, Var, VarName, VarType};

pub type StorageIndex = (CompName, VarName);
// type TypedStorageIndex = (StorageIndex, VarType);

/// Entity's main data storage structure.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub map: FnvHashMap<StorageIndex, Var>,
    // TODO benchmark performance of the alternative storage layout
    // _map: FnvHashMap<CompId, FnvHashMap<VarId, Var>>,
}

impl Storage {
    pub fn get_var(&self, idx: &StorageIndex) -> Result<&Var> {
        self.map
            .get(&idx)
            .ok_or(Error::FailedGettingVarFromEntityStorage(*idx))
    }

    pub fn get_var_mut(&mut self, idx: &StorageIndex) -> Result<&mut Var> {
        self.map
            .get_mut(&idx)
            .ok_or(Error::FailedGettingVarFromEntityStorage(*idx))
    }

    pub fn get_all_coerce_to_string(&self) -> HashMap<String, String> {
        let mut out_map = HashMap::new();
        for (index, var) in &self.map {
            // println!("{:?}, {:?}", index, var);
            let (comp_name, var_name) = index;
            out_map.insert(
                format!("{}:{}:{}", comp_name, var.get_type().to_str(), var_name),
                var.to_string(),
            );
        }
        out_map
    }

    pub fn insert(&mut self, idx: (CompName, VarName), var: Var) {
        self.map.insert(idx, var);
    }

    pub fn set_from_str(&mut self, target: &Address, val: &str) {
        unimplemented!();
    }
    pub fn set_from_addr(&mut self, target: &Address, source: &Address) {
        unimplemented!();
    }
    pub fn set_from_var(&mut self, target: &Address, comp_uid: Option<&CompName>, var: &Var) {
        let target = self
            .get_var_mut(&(target.component, target.var_name))
            .unwrap();
        *target = var.clone();
    }

    pub fn remove_comp_vars(&mut self, comp_name: &CompName, comp_model: &ComponentModel) {
        for var_model in &comp_model.vars {
            self.map.remove(&(*comp_name, var_model.id));
        }
    }
}
