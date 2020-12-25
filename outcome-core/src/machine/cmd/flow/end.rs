use arrayvec::ArrayVec;

use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::var::Var;
use crate::{CompId, VarType};

use super::super::super::{error::Error, CallInfo, CallStackVec, LocationInfo, ProcedureCallInfo};
use super::super::CommandResult;
use super::forin::ForIn;

pub const COMMAND_NAMES: [&'static str; 1] = ["end"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct End {}

impl End {
    pub fn new(args: Vec<String>) -> Result<End, Error> {
        Ok(End {})
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        comp_uid: &CompId,
        ent_storage: &mut Storage,
        location: &LocationInfo,
    ) -> CommandResult {
        let mut do_pop = false;
        // make sure the stack is not empty
        let clen = call_stack.len();
        if clen <= 0 {
            return CommandResult::Continue;
        }
        // peek the stack and process flow control aspects accordingly
        match call_stack.last_mut().unwrap() {
            // CallInfo::Procedure()
            CallInfo::ForIn(ref mut fici) => {
                // forin that's still not finished iterating should not be popped off
                if fici.iteration < fici.target_len {
                    if let Some(target) = &fici.target {
                        if let Some(source_variable) = &fici.variable {
                            // update the iterator variable
                            ForIn::update_variable(
                                source_variable,
                                // &fici.variable_type,
                                target,
                                fici.iteration,
                                ent_storage,
                            );
                        } else {
                            if let Some(int_var) =
                                ent_storage.get_int_mut(&target.storage_index().unwrap())
                            {
                                *int_var = fici.iteration as i64;
                            }
                        }
                    }

                    fici.iteration = fici.iteration + 1;
                    return CommandResult::JumpToLine(fici.start + 1);
                } else {
                    do_pop = true;
                }
            }
            CallInfo::IfElse(ieci) => {}
            _ => do_pop = true,
        };

        // here we actually pop the stack and process contents as needed
        if do_pop {
            let ci = match call_stack.pop() {
                Some(c) => c,
                //None => return CommandResult::Error()
                None => panic!(),
            };
            match ci {
                CallInfo::Procedure(pci) => {
                    if pci.end_line == location.line.unwrap() {
                        return CommandResult::JumpToLine(pci.call_line + 1);
                    }
                }
                _ => (),
            };
        }
        CommandResult::Continue
    }
}
