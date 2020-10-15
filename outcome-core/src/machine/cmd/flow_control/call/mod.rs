use crate::component::Component;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::{CompId, ShortString, StringId};

use super::super::super::{
    error::Error, CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo,
    Registry,
};
use super::super::{CommandPrototype, CommandResult, LocationInfo};
use crate::machine::error::ErrorKind;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Call {
    pub proc_name: ShortString,
}
impl Call {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Call, Error> {
        Ok(Call {
            proc_name: ShortString::from(&args[0]).unwrap(),
        })
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        line: usize,
        sim_model: &SimModel,
        comp_uid: &CompId,
        location: &LocationInfo,
    ) -> CommandResult {
        let comp_model = sim_model.get_component(comp_uid).unwrap();
        let (start_line, end_line) = match comp_model.logic.procedures.get(&self.proc_name) {
            Some(se) => se,
            None => {
                return CommandResult::Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody(format!(
                    "call error: procedure with that name doesn't exist in the current scope: {}",
                    &self.proc_name
                )),
                ))
            }
        };
        call_stack.push(CallInfo::Procedure(ProcedureCallInfo {
            call_line: line,
            start_line: *start_line,
            end_line: *end_line,
            // output_variable: target_proc.output_variable,
        }));
        CommandResult::JumpToLine(start_line + 1)
    }
}
