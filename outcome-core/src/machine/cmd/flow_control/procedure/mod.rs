use std::convert::From;
use std::iter::FromIterator;

use crate::component::Component;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::{Address, ShortString};

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CommandPrototype, CommandResult, LocationInfo};
use crate::machine::error::ErrorKind;
use crate::machine::Result;

pub const PROCEDURE_COMMAND_NAMES: [&'static str; 2] = ["proc", "procedure"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Procedure {
    pub name: ShortString,
    pub start_line: usize,
    pub end_line: usize,
    pub output_variable: Option<Address>,
}
impl Procedure {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Procedure> {
        let line = location.line.unwrap();

        // TODO all these names should probably be declared in a
        // better place start names
        let mut start_names = Vec::new();
        start_names.extend(&PROCEDURE_COMMAND_NAMES);
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
        start_blocks.extend(&super::state::STATE_COMMAND_NAMES);
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
            Some(positions) => Ok(Procedure {
                name: ShortString::from(&args[0]).unwrap(),
                start_line: line,
                end_line: positions.0,
                output_variable: None,
            }),
            None => Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("End of procedure block not found".to_string()),
            )),
        }
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        ent_storage: &mut Storage,
        line: usize,
    ) -> CommandResult {
        // call_stack.push(CallInfo::Procedure(ProcedureCallInfo {
        //     call_line: line,
        //     start_line: self.start_line,
        //     end_line: self.end_line,
        // }));
        CommandResult::JumpToLine(self.end_line + 1)
    }
}
