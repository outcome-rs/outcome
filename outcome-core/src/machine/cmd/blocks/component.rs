use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, LogicModel, SimModel, VarModel};
use crate::sim::interface::SimInterface;
use crate::{CompId, EntityId, ShortString, Sim, StringId, VarType};
use std::iter::FromIterator;

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CentralExtCommand, Command, CommandPrototype, CommandResult, LocationInfo};
use crate::machine::error::ErrorKind;

pub const COMMAND_NAMES: [&'static str; 1] = ["state"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ComponentBlock {
    pub name: ShortString,
    pub start_line: usize,
    pub end_line: usize,
    pub output_variable: Option<Address>,
}

impl ComponentBlock {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Command, Error> {
        let line = location.line.unwrap();

        // TODO all these names should probably be declared in a
        // better place start names
        let mut start_names = Vec::new();
        start_names.extend(&COMMAND_NAMES);
        // middle names
        let mut middle_names = Vec::new();
        // end names
        let mut end_names = Vec::new();
        end_names.extend(&super::end::COMMAND_NAMES);
        // other block starting names
        let mut start_blocks = Vec::new();
        start_blocks.extend(&super::ifelse::IF_COMMAND_NAMES);
        start_blocks.extend(&super::ifelse::ELSE_COMMAND_NAMES);
        start_blocks.extend(&super::forin::COMMAND_NAMES);
        start_blocks.extend(&super::procedure::COMMAND_NAMES);
        start_blocks.extend(&super::state::COMMAND_NAMES);
        // other block ending names
        let mut end_blocks = Vec::new();
        end_blocks.extend(&super::end::COMMAND_NAMES);

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
            Some(positions) => Ok(Command::Component(ComponentBlock {
                name: ShortString::from_truncate(&args[0]),
                start_line: line + 1,
                end_line: positions.0,
                output_variable: None,
            })),
            None => Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("end of component block not found".to_string()),
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
        // unimplemented!()
        let mut new_self = self.clone();

        //println!("{:?}", new_self);
        let mut out_vec = Vec::new();
        out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::Component(
            new_self,
        )));
        out_vec.push(CommandResult::Continue);
        // out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        out_vec
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        //println!("execute ext on state cmd");
        //println!("{:?}", sim.model.components);
        let component = ComponentModel {
            name: self.name,
            vars: vec![],
            // vars: vec![VarModel {
            //     id: "test_string".to_string(),
            //     type_: VarType::Str,
            //     default: crate::Var::Str("66asads2s".to_string()),
            //     internal: false,
            // }],
            start_state: Default::default(),
            triggers: vec![],
            logic: LogicModel {
                commands: vec![],
                pre_commands: Default::default(),
                states: Default::default(),
                procedures: Default::default(),
                cmd_location_map: Default::default(),
            },
            source_files: vec![],
            script_files: vec![],
            lib_files: vec![],
        };
        sim.model.components.push(component);
        debug!("added new component to model: {:?}", self.name);
        Ok(())
    }
}
