use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, LogicModel, SimModel, VarModel};
use crate::sim::interface::SimInterface;
use crate::{CompId, EntityId, LongString, ShortString, Sim, StringId, VarType};
use std::iter::FromIterator;

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CentralExtCommand, Command, CommandPrototype, CommandResult, LocationInfo};
use crate::machine::cmd::assembly::{RegComponent, Register};
use crate::machine::error::ErrorKind;
use crate::machine::script::parse_script_at;
use crate::machine::ComponentCallInfo;

pub const COMMAND_NAMES: [&'static str; 1] = ["component"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBlock {
    pub name: StringId,
    pub source_comp: StringId,
    pub source_file: LongString,
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
        trace!("making new comp block");

        let line = location.line.unwrap();

        // start names
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

        let positions_options = match crate::machine::command_search(
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
                name: StringId::from_truncate(&args[0]),
                source_comp: location.comp_name.unwrap(),
                source_file: location.source.unwrap(),
                start_line: line + 1,
                end_line: positions.0,
                output_variable: None,
            })),
            // {
            //     Ok(Command::Register(Register::Component(RegComponent {
            //         name: StringId::from_truncate(&args[0]),
            //         trigger_events: vec![],
            //     })))
            // }
            None => Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("end of component block not found".to_string()),
            )),
        }
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        ent_name: &EntityId,
        comp_name: &CompId,
        line: usize,
    ) -> Vec<CommandResult> {
        trace!("executing component block: {:?}", self);

        let mut new_self = self.clone();

        call_stack.push(CallInfo::Component(ComponentCallInfo {
            name: new_self.name,
        }));

        let mut out_vec = Vec::new();
        // out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::Component(
        //     new_self,
        // )));
        out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::Register(
            Register::Component(RegComponent {
                name: StringId::from_truncate(&new_self.name),
                trigger_events: vec![],
            }),
        )));
        out_vec.push(CommandResult::Continue);
        // out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        out_vec
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        trace!("registering component");

        let comp_model = sim.model.get_component(&self.source_comp).unwrap();

        let component = ComponentModel {
            name: self.name.into(),
            start_state: StringId::from_unchecked("start"),
            // triggers: vec![StringId::from_unchecked("step")],
            logic: LogicModel {
                commands: comp_model.logic.commands.clone(),
                cmd_location_map: comp_model.logic.cmd_location_map.clone(),
                ..LogicModel::default()
            },
            ..ComponentModel::default()
        };

        sim.model.components.push(component);
        debug!("added new component to model: {:?}", self.name);
        Ok(())
    }
}
