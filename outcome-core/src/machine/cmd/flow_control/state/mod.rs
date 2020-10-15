use crate::address::Address;
use crate::component::Component;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::sim::interface::SimInterface;
use crate::{CompId, EntityId, ShortString, Sim, StringId};
use std::iter::FromIterator;

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CentralExtCommand, Command, CommandPrototype, CommandResult, LocationInfo};
use crate::machine::error::ErrorKind;

pub const STATE_COMMAND_NAMES: [&'static str; 1] = ["state"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct State {
    pub signature: Option<Address>,
    pub name: ShortString,
    pub start_line: usize,
    pub end_line: usize,
    pub output_variable: Option<Address>,
}

impl State {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        // commands: &Vec<(CommandPrototype, LocationInfo)>,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Command, Error> {
        let line = location.line.unwrap();

        // TODO all these names should probably be declared in a
        // better place start names
        let mut start_names = Vec::new();
        start_names.extend(&STATE_COMMAND_NAMES);
        // middle names
        let mut middle_names = Vec::new();
        // end names
        let mut end_names = Vec::new();
        end_names.extend(&super::end::END_COMMAND_NAMES);
        // other block starting names
        let mut start_blocks = Vec::new();
        start_blocks.extend(&super::ifelse::IF_COMMAND_NAMES);
        start_blocks.extend(&super::ifelse::ELSE_COMMAND_NAMES);
        start_blocks.extend(&super::forin::FOR_COMMAND_NAMES);
        start_blocks.extend(&super::procedure::PROCEDURE_COMMAND_NAMES);
        // other block ending names
        let mut end_blocks = Vec::new();
        end_blocks.extend(&super::end::END_COMMAND_NAMES);

        let positions_options = match super::super::super::command_search(
            location,
            &commands,
            (line + 1, None),
            (&start_names, &middle_names, &end_names),
            (&start_blocks, &end_blocks),
            true,
        ) {
            Ok(po) => po,
            Err(e) => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody(e.to_string()),
                ))
            }
        };

        match positions_options {
            Some(positions) => Ok(Command::State(State {
                signature: None,
                name: ShortString::from(&args[0]).unwrap(),
                start_line: line + 1,
                end_line: positions.0,
                output_variable: None,
            })),
            None => Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("end of state block not found".to_string()),
            )),
        }
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        ent_uid: &EntityId,
        comp_uid: &CompId,
        line: usize,
    ) -> Vec<CommandResult> {
        unimplemented!()
        // let mut new_self = self.clone();
        // let mut addr = Address::from_uids(ent_uid, comp_uid);
        // new_self.signature = Some(addr);
        //
        // //println!("{:?}", new_self);
        // let mut out_vec = Vec::new();
        // // out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::State(
        // //     new_self,
        // // )));
        // out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        // out_vec
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        //println!("execute ext on state cmd");
        //println!("{:?}", sim.model.components);
        let comp_name = self.signature.unwrap().component;
        for component in &mut sim.model.components {
            if component.name != comp_name {
                continue;
            }
            component
                .logic
                .states
                .insert(self.name, (self.start_line, self.end_line));
            debug!("inserted state at comp: {:?}", comp_name);
        }
        Ok(())
    }
}
