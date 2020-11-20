use crate::machine::cmd::{CentralExtCommand, Command, CommandResult};
use crate::machine::{CallStackVec, CommandPrototype, Error, LocationInfo};
use crate::model::EntityPrefabModel;
use crate::{CompId, EntityId, Sim, StringId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefab {
    name: StringId,
    components: Vec<StringId>,
}

impl Prefab {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Command, Error> {
        Ok(Command::Prefab(Prefab {
            name: StringId::from_truncate(&args[0]),
            components: args
                .iter()
                .skip(1)
                .map(|a| StringId::from_truncate(a))
                .collect(),
        }))
    }
    pub fn execute_loc(&self) -> Vec<CommandResult> {
        vec![
            CommandResult::ExecCentralExt(CentralExtCommand::Prefab(self.clone())),
            CommandResult::Continue,
        ]
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        sim.model.entities.push(EntityPrefabModel {
            name: self.name,
            components: self.components.clone(),
        });
        Ok(())
    }
}
