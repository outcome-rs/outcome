use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::{arraystring, CompName, EntityId, EntityName, ShortString, Sim, StringId};
use std::iter::FromIterator;

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CentralRemoteCommand, Command, CommandPrototype, CommandResult, LocationInfo};
use crate::distr::SimCentral;
use crate::machine::error::ErrorKind;
use crate::machine::ComponentCallInfo;

pub const COMMAND_NAMES: [&'static str; 1] = ["state"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct State {
    pub comp: CompName,
    pub signature: Option<Address>,
    pub name: StringId,
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
            Some(positions) => Ok(Command::State(State {
                comp: arraystring::new_truncate(""),
                signature: None,
                name: arraystring::new_truncate(&args[0]),
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
        comp_name: &CompName,
        line: usize,
    ) -> Vec<CommandResult> {
        // unimplemented!()
        let mut new_self = self.clone();
        // let mut addr = Address::from_uids(ent_name, comp_uid);
        // new_self.signature = Some(addr);
        if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
            CallInfo::Component(c) => Some(c),
            _ => None,
        }) {
            new_self.comp = comp_info.name;
            new_self.start_line = self.start_line - comp_info.start_line;
            new_self.end_line = self.end_line - comp_info.start_line;
        }

        //println!("{:?}", new_self);
        let mut out_vec = Vec::new();
        out_vec.push(CommandResult::ExecCentralExt(CentralRemoteCommand::State(
            new_self,
        )));
        out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        out_vec
    }

    pub fn execute_ext(&self, sim: &mut Sim) -> Result<(), Error> {
        //println!("execute ext on state cmd");
        //println!("{:?}", sim.model.components);
        // let comp_name = self.signature.unwrap().component;
        let comp_name = self.comp;
        // trace!("comp_name: {:?}", comp_name);
        for component in &mut sim.model.components {
            if component.name != comp_name {
                continue;
            }
            component
                .logic
                .states
                .insert(self.name, (self.start_line, self.end_line));
            debug!("inserted state {:?} at comp {:?}", self, comp_name);
        }
        Ok(())
    }

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<(), Error> {
        //println!("execute ext on state cmd");
        //println!("{:?}", sim.model.components);
        // let comp_name = self.signature.unwrap().component;
        let comp_name = self.comp;
        // trace!("comp_name: {:?}", comp_name);
        for component in &mut central.model.components {
            if component.name != comp_name {
                continue;
            }
            component
                .logic
                .states
                .insert(self.name, (self.start_line, self.end_line));
            debug!("inserted state {:?} at comp {:?}", self, comp_name);
        }
        Ok(())
    }
}
