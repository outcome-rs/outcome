use smallvec::SmallVec;

use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, SimModel};
use crate::MedString;

use super::super::super::{
    error::Error, CallInfo, CallStackVec, ForInCallInfo, IfElseCallInfo, IfElseMetaData,
    ProcedureCallInfo, Registry,
};
use super::super::{CentralExtCommand, Command, CommandPrototype, CommandResult, LocationInfo};
use crate::machine::error::ErrorKind;
use crate::machine::Result;

pub const IF_COMMAND_NAMES: [&'static str; 1] = ["if"];
pub const ELSE_COMMAND_NAMES: [&'static str; 2] = ["else", "else_if"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    // Command()
    BoolValue(bool),
}
impl Condition {
    pub fn evaluate(&self) -> bool {
        match self {
            Condition::BoolValue(b) => *b,
            _ => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct If {
    pub condition: Condition,
    pub start: usize,
    pub end: usize,
    pub else_lines: SmallVec<[usize; 10]>,
}

impl If {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<If> {
        if args.len() == 0 {
            return Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("no arguments provided".to_string()),
            ));
        }
        let line = location.line.unwrap();

        // start names
        let mut start_names = Vec::new();
        start_names.extend(&IF_COMMAND_NAMES);
        // middle names
        let mut middle_names = Vec::new();
        middle_names.extend(&ELSE_COMMAND_NAMES);
        // TODO push middle_names as start_names?
        // start_names.append(&mut middle_names.clone());
        // end names
        let mut end_names = Vec::new();
        end_names.extend(&super::end::COMMAND_NAMES);
        // other block starting names
        let mut start_blocks = Vec::new();
        start_blocks.extend(&super::procedure::COMMAND_NAMES);
        start_blocks.extend(&super::forin::COMMAND_NAMES);
        // other block ending names
        let mut end_blocks = Vec::new();
        end_blocks.extend(&super::end::COMMAND_NAMES);

        let positions_opt = match super::super::super::command_search(
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

        // condition
        let condition = match args[0].as_str() {
            "true" => Condition::BoolValue(true),
            _ => Condition::BoolValue(false),
        };

        match positions_opt {
            Some(positions) => Ok(If {
                condition,
                start: line,
                end: positions.0,
                else_lines: SmallVec::from(positions.1),
            }),
            None => Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody("end of if/else block not found.".to_string()),
            )),
        }
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        ent_storage: &mut Storage,
        line: usize,
    ) -> CommandResult {
        let mut else_lines_arr = [0; 10];
        for (n, el) in self.else_lines.iter().enumerate() {
            else_lines_arr[n] = *el;
        }
        if self.condition.evaluate() {
            debug!("evaluated to true");
            let next_line = if self.else_lines.is_empty() {
                self.end
            } else {
                self.else_lines[0]
            };

            let call_info = CallInfo::IfElse(IfElseCallInfo {
                current: next_line,
                passed: true,
                else_line_index: 0,
                meta: IfElseMetaData {
                    start: self.start,
                    end: self.end,
                    else_lines: else_lines_arr,
                    //else_lines: self.else_lines.into_iter().collect::<[usize; 10]>(),
                },
            });
            call_stack.push(call_info);
            CommandResult::Continue
        } else {
            if !self.else_lines.is_empty() {
                let goto_line = self.else_lines[0];
                let call_info = CallInfo::IfElse(IfElseCallInfo {
                    current: goto_line,
                    passed: false,
                    else_line_index: 0,
                    meta: IfElseMetaData {
                        start: self.start,
                        end: self.end,
                        else_lines: else_lines_arr,
                    },
                });
                call_stack.push(call_info);
                CommandResult::JumpToLine(goto_line)
            } else {
                let goto_line = self.end + 1;
                CommandResult::JumpToLine(goto_line)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ElseIf {
    condition: Condition,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Else {}
impl Else {
    pub fn new(args: Vec<String>) -> Result<Else> {
        Ok(Else {})
    }
    pub fn execute_loc(
        &self,
        //call_stack: &mut Vec<CallInfo>,
        call_stack: &mut CallStackVec,
        // component: &mut Component,
        ent_storage: &mut Storage,
        location: &LocationInfo,
    ) -> CommandResult {
        debug!("execute else");
        let mut result = CommandResult::Continue;
        match call_stack.pop() {
            Some(call_info) => {
                match &call_info {
                    CallInfo::IfElse(ci) => {
                        if ci.passed {
                            let goto_line = ci.meta.end + 1;
                            result = CommandResult::JumpToLine(goto_line);
                        }
                    }
                    _ => (),
                }
                // return the call info to the stack
                call_stack.push(call_info);
            }
            None => {
                result = CommandResult::Err(Error::new(*location, ErrorKind::StackEmpty));
            }
        }
        result
        // CommandResult::Ok
    }
}
