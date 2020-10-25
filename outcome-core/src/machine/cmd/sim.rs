use crate::component::Component;
use crate::entity::Storage;
use crate::machine::cmd::{CentralExtCommand, Command, CommandResult};
use crate::machine::{error::Error, LocationInfo};
use crate::sim::interface::SimInterface;
use crate::{Address, CompId, Sim};

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
        Ok(())
    }
}
