use crate::component::Component;
use crate::entity::Storage;
use crate::sim::interface::SimInterface;
use crate::{Address, CompId, Sim};

use super::super::{error::Error, LocationInfo};
use super::{CentralExtCommand, Command, CommandResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimControl {
    pub args: Vec<String>,
}

impl SimControl {
    pub fn get_type() -> String {
        return "sim".to_string();
    }
    pub fn new(args: Vec<String>) -> Result<Command, Error> {
        Ok(Command::Sim(SimControl { args }))
    }
}
impl SimControl {
    pub fn execute_loc(
        &self,
        storage: &mut Storage,
        component: &Component,
        comp_uid: &CompId,
        location: &LocationInfo,
    ) -> CommandResult {
        CommandResult::ExecCentralExt(CentralExtCommand::Sim(self.clone()))
    }

    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        match self.args[0].as_str() {
            "apply_model" => sim.apply_model().unwrap(),
            _ => (),
        }
        // println!("{:?}", sim.model.components[0].entity_type);
        // println!("{:?}", sim.model.components[1]);
        // println!("{:?}", sim.entities.iter().nth(24425).unwrap().1.storage);
        Ok(())
    }
}
