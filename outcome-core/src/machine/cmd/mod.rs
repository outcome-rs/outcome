//! Command definitions.
//!
//! Command struct serves as the basic building block for the in-memory logic
//! representation used by the runtime. Each command provides an implementation
//! for it's interpretation (converting a command prototype into target struct)
//! and for it's execution (performing work, usually work on some piece of
//! data).
//!
//! Individual *component runtimes*, as seen declared within a model, exist
//! as collections of command structs. During logic execution, these
//! collections are iterated on, executing the commands one by one.
//!
//! Command structs are stored on component models, making each component of
//! certain type contain the same set of commands.

#![allow(unused)]

extern crate getopts;
extern crate strsim;

use std::collections::HashMap;
use std::env::args;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use arrayvec::ArrayVec;
use fnv::FnvHashMap;
// use serde_yaml::Value;
use smallvec::SmallVec;

#[cfg(feature = "machine_dynlib")]
use libloading::Library;

use crate::{model, util, CompId, ShortString};
use crate::{EntityId, MedString, Sim, StringId, VarType};

use crate::address::Address;
use crate::entity::{Entity, EntityNonSer, Storage};
// use crate::error::Error;
use crate::model::SimModel;
use crate::sim::interface::SimInterface;
// use crate::Result;
use crate::Var;

pub mod assembly;
// pub mod equal;
pub mod blocks;
pub mod eval;
pub mod get_set;

#[cfg(feature = "machine_dynlib")]
pub mod lib;
#[cfg(feature = "machine_lua")]
pub mod lua;

pub mod prefab;
pub mod print;
pub mod range;
pub mod set;
pub mod sim;

// use self::equal::*;
// use self::eval::*;
use self::get_set::*;
// use self::lib::*;
// use self::lua::*;

use super::{CommandPrototype, CommandResultVec, LocationInfo};
use crate::distr::{CentralCommunication, SimCentral};
use crate::machine;
use crate::machine::error::{Error, ErrorKind, Result};
// use std::ops::Try;

// pub type CmdResult = std::result::Result<CommandResult, Error>;

/// Used for controlling the flow of execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandResult {
    /// Continue execution
    Continue,
    /// Break execution
    Break,
    /// Jump to line
    JumpToLine(usize),
    /// Jump to tag
    JumpToTag(StringId),
    /// Execute command that needs access to another entity
    ExecExt(ExtCommand),
    /// Execute command that needs access to central authority
    ExecCentralExt(CentralExtCommand),
    /// Signalize that an error has occurred during execution of command
    Err(machine::Error),
}
impl CommandResult {
    pub fn from_str(s: &str) -> Option<CommandResult> {
        if s.starts_with("jump.") {
            let c = &s[5..];
            return Some(CommandResult::JumpToLine(c.parse::<usize>().unwrap()));
        }
        //else if s.starts_with("goto.") {
        //let c = &s[5..];
        //return Some(CommandResult::GoToState(SmallString::from_str(c).unwrap()));
        //}
        else {
            match s {
                "ok" | "Ok" | "OK" | "continue" => Some(CommandResult::Continue),
                "break" => Some(CommandResult::Break),
                _ => None,
            }
        }
    }
}

/// Defines all the local commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Sim(sim::SimControl),
    Print(print::Print),
    PrintFmt(print::PrintFmt),

    Set(set::Set),
    SetIntIntAddr(set::SetIntIntAddr),

    Eval(eval::Eval),
    // EvalReg(EvalReg),

    // Equal(Equal),
    // BiggerThan(BiggerThan),
    #[cfg(feature = "machine_lua")]
    LuaScript(lua::LuaScript),
    #[cfg(feature = "machine_lua")]
    LuaCall(lua::LuaCall),
    #[cfg(feature = "machine_dynlib")]
    LibCall(lib::LibCall),

    Attach(Attach),
    Detach(Detach),
    Goto(Goto),
    Jump(Jump),

    // ext
    Get(Get),

    // central ext
    Invoke(Invoke),
    Spawn(Spawn),
    Prefab(prefab::Prefab),

    // model assembly commands
    Register(assembly::Register),
    Extend(assembly::Extend),

    // flow control
    If(blocks::ifelse::If),
    Else(blocks::ifelse::Else),
    End(blocks::end::End),
    Call(blocks::call::Call),
    ForIn(blocks::forin::ForIn),

    Procedure(blocks::procedure::Procedure),
    State(blocks::state::State),
    Component(blocks::component::ComponentBlock),

    Range(range::Range),
}

impl Command {
    /// Creates new command struct from a prototype.
    pub fn from_prototype(
        proto: &CommandPrototype,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Command> {
        let cmd_name = match &proto.name {
            Some(c) => c,
            None => return Err(Error::new(*location, ErrorKind::NoCommandPresent)),
        };
        let args = match &proto.arguments {
            Some(a) => a.clone(),
            None => Vec::new(),
        };
        match cmd_name.as_str() {
            "sim" => Ok(sim::SimControl::new(args)?),
            "set" => Ok(set::Set::new(args, location)?),
            "extend" => Ok(Command::Extend(assembly::Extend::new(args, location)?)),
            "register" => Ok(Command::Register(assembly::Register::new(args, location)?)),
            "print" => Ok(Command::PrintFmt(print::PrintFmt::new(args)?)),
            "spawn" => Ok(Command::Spawn(Spawn::new(args, location)?)),
            "prefab" => Ok(prefab::Prefab::new(args, location)?),

            "invoke" => Ok(Command::Invoke(Invoke::new(args)?)),

            "range" => Ok(Command::Range(range::Range::new(args)?)),

            // flow control
            "if" => Ok(Command::If(blocks::ifelse::If::new(
                args, location, &commands,
            )?)),
            "else" => Ok(Command::Else(blocks::ifelse::Else::new(args)?)),
            "proc" | "procedure" => Ok(Command::Procedure(blocks::procedure::Procedure::new(
                args, location, &commands,
            )?)),
            "call" => Ok(Command::Call(blocks::call::Call::new(
                args, location, &commands,
            )?)),
            "end" => Ok(Command::End(blocks::end::End::new(args)?)),
            "for" => Ok(Command::ForIn(blocks::forin::ForIn::new(
                args, location, commands,
            )?)),

            "component" => Ok(blocks::component::ComponentBlock::new(
                args, location, &commands,
            )?),
            "state" => Ok(blocks::state::State::new(args, location, &commands)?),

            "eval" => Ok(eval::Eval::new(args)?),
            // "evalreg" => Ok(eval::EvalReg::new(args, location, commands)?),
            _ => Err(Error::new(
                *location,
                ErrorKind::UnknownCommand(cmd_name.to_string()),
            )),
        }
    }
    /// Execute `loc` phase command (within the context of
    /// single entity).
    pub fn execute(
        &self,
        mut ent_storage: &mut Storage,
        mut ent_insta: &mut EntityNonSer,
        mut comp_state: &mut StringId,
        mut call_stack: &mut super::CallStackVec,
        mut registry: &mut super::Registry,
        comp_uid: &CompId,
        ent_name: &EntityId,
        sim_model: &SimModel,
        location: &LocationInfo,
    ) -> CommandResultVec {
        let line = location.line.unwrap();
        let mut out_res = CommandResultVec::new();
        match self {
            Command::Sim(cmd) => {
                out_res.push(cmd.execute_loc(ent_storage, comp_state, comp_uid, location))
            }
            Command::Print(cmd) => out_res.push(cmd.execute_loc(ent_storage)),
            Command::PrintFmt(cmd) => {
                out_res.push(cmd.execute_loc(ent_storage, comp_state, comp_uid, location))
            }
            Command::Set(cmd) => {
                out_res.push(cmd.execute_loc(ent_storage, ent_name, comp_state, comp_uid, location))
            }
            Command::SetIntIntAddr(cmd) => {
                out_res.push(cmd.execute_loc(ent_storage, comp_uid, location))
            }

            Command::Eval(cmd) => out_res.push(cmd.execute_loc(comp_uid, ent_storage, registry)),
            // Command::EvalReg(cmd) => out_res.push(cmd.execute_loc(registry)),

            //Command::Eval(cmd) => out_res.push(cmd.execute_loc(ent_storage)),
            //Command::Equal(cmd) => out_res.push(cmd.execute_loc(ent_storage)),
            //Command::BiggerThan(cmd) => out_res.push(cmd.execute_loc(ent_storage)),

            //// Command::LuaScript(cmd) => out_res.push(cmd.execute_loc(ent_storage, comp_uid)),
            //Command::LuaCall(cmd) => out_res.extend(cmd.execute_loc_lua(sim_model, ent)),
            ////Command::LibCall(cmd) => out_res.push(cmd.execute_loc(libs, ent_storage)),
            //Command::Attach(cmd) => out_res.push(cmd.execute_loc(ent, sim_model)),
            //Command::Detach(cmd) => out_res.push(cmd.execute_loc(ent, sim_model)),
            //Command::Goto(cmd) => out_res.push(cmd.execute_loc()),
            //Command::Jump(cmd) => out_res.push(cmd.execute_loc()),

            //Command::Get(cmd) => out_res.push(cmd.execute_loc()),
            Command::Invoke(cmd) => out_res.push(cmd.execute_loc()),
            Command::Spawn(cmd) => out_res.push(cmd.execute_loc()),
            Command::Prefab(cmd) => out_res.extend(cmd.execute_loc()),
            Command::Call(cmd) => {
                out_res.push(cmd.execute_loc(call_stack, line, sim_model, comp_uid, location))
            }

            Command::If(cmd) => out_res.push(cmd.execute_loc(call_stack, ent_storage, line)),
            Command::Else(cmd) => out_res.push(cmd.execute_loc(call_stack, ent_storage, location)),
            Command::ForIn(cmd) => {
                out_res.push(cmd.execute_loc(call_stack, registry, comp_uid, ent_storage, location))
            }
            Command::End(cmd) => {
                out_res.push(cmd.execute_loc(call_stack, comp_uid, ent_storage, location))
            }
            Command::Procedure(cmd) => out_res.push(cmd.execute_loc(call_stack, ent_storage, line)),

            Command::State(cmd) => {
                out_res.extend(cmd.execute_loc(call_stack, ent_name, comp_uid, line))
            }
            Command::Component(cmd) => {
                out_res.extend(cmd.execute_loc(call_stack, ent_name, comp_uid, line))
            }

            Command::Extend(cmd) => out_res.push(cmd.execute_loc()),
            Command::Register(cmd) => out_res.extend(cmd.execute_loc(call_stack)),

            Command::Range(cmd) => out_res.push(cmd.execute_loc(ent_storage, comp_uid, location)),

            _ => out_res.push(CommandResult::Continue),
        };
        out_res
    }
    pub fn run_with_model_context(&self, sim_model: &mut SimModel) -> CommandResult {
        match self {
            // Command::Register(cmd) => cmd.execute(sim_model),
            // Command::Print(cmd) => cmd.run(),
            _ => CommandResult::Continue,
        }
    }
}

/// External, meaning not entity-local, command meant for execution
/// within central authority scope.
///
/// ### Distinction between external and central-external
///
/// Distinction is made because of the potentially distributed nature of the
/// simulation. In such setting, there are certain things, like the simulation
/// model, that have to be managed from a central point.
///
/// Central authority requirement is necessary for commands that make changes
/// to the model or the event queue, or those that spawn entities, or attach
/// components to entities other than the caller's.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CentralExtCommand {
    Sim(sim::SimControl),
    Register(assembly::Register),
    Extend(assembly::Extend),
    Invoke(Invoke),
    Spawn(Spawn),
    Prefab(prefab::Prefab),

    State(blocks::state::State),
    Component(blocks::component::ComponentBlock),
}
impl CentralExtCommand {
    pub fn execute(&self, mut sim: &mut Sim, ent_uid: &EntityId, comp_uid: &CompId) -> Result<()> {
        match self {
            //            ExtCommand::Get(cmd) => return cmd.execute_ext(sim, ent_uid),
            //            ExtCommand::Set(cmd) => return cmd.execute_ext(sim, ent_puid, comp_puid),
            //            ExtCommand::Eval(cmd) => return cmd.execute_ext(sim),
            //            ExtCommand::Equal(cmd) => return cmd.execute_ext(sim),
            CentralExtCommand::Sim(cmd) => return cmd.execute_ext(sim),
            //TODO
            CentralExtCommand::Extend(cmd) => return cmd.execute_ext(sim, ent_uid),
            CentralExtCommand::Register(cmd) => return cmd.execute_ext(sim, ent_uid, comp_uid),
            CentralExtCommand::Invoke(cmd) => return cmd.execute_ext(sim),
            CentralExtCommand::Spawn(cmd) => return cmd.execute_ext(sim),
            CentralExtCommand::Prefab(cmd) => return cmd.execute_ext(sim),

            CentralExtCommand::State(cmd) => return cmd.execute_ext(sim),
            CentralExtCommand::Component(cmd) => return cmd.execute_ext(sim),

            _ => return Ok(()),
        }
    }
    pub fn execute_distr<N: CentralCommunication>(
        &self,
        mut central: &mut SimCentral,
        net: &mut N,
        ent_name: &EntityId,
        comp_name: &CompId,
    ) -> Result<()> {
        match self {
            CentralExtCommand::Spawn(cmd) => cmd.execute_ext_distr(central).unwrap(),
            CentralExtCommand::Register(cmd) => {
                cmd.execute_ext_distr(central, ent_name, comp_name).unwrap()
            }
            _ => (),
        }
        Ok(())
    }
}

/// External command meant for execution on an entity scope
/// that includes operations that don't require access to
/// central authority.
///
/// Can be used to allow message-based communication between
/// entity objects.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExtCommand {
    Get(Get),
    Set(ExtSet),
    SetVar(ExtSetVar),
    // RemoteExec(Command),
    // CentralizedExec(CentralExtCommand),
}
impl ExtCommand {
    pub fn execute(&self, mut sim: &mut Sim, ent_uid: &EntityId, comp_uid: &CompId) -> Result<()> {
        match self {
            ExtCommand::Get(cmd) => return cmd.execute_ext(sim, ent_uid),
            ExtCommand::Set(cmd) => return cmd.execute_ext(sim, ent_uid),
            ExtCommand::SetVar(cmd) => return cmd.execute_ext(sim, ent_uid),
            _ => return Ok(()),
        }
    }

    pub fn execute_pre(
        &self,
        mut storage: &mut Storage,
        ent_uid: &EntityId,
    ) -> Option<(Address, Var)> {
        match self {
            ExtCommand::Get(cmd) => return cmd.exec_pre(storage, ent_uid),
            _ => None,
        }
    }

    pub fn get_type_as_str(&self) -> &str {
        match self {
            // ExtCommand::Set(_) => "set",
            _ => "not implemented",
        }
    }
}

/// Attach
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Attach {
    pub model_type: StringId,
    pub model_id: StringId,
    pub new_id: StringId,
}
impl Attach {
    // fn from_str(args_str: &str) -> MachineResult<Self> {
    //     let split: Vec<&str> = args_str.split(" ").collect();
    //     Ok(Attach {
    //         model_type: StringIndex::from(split[0]).unwrap(),
    //         model_id: StringIndex::from(split[1]).unwrap(),
    //         new_id: StringIndex::from(split[2]).unwrap(),
    //     })
    // }
    pub fn execute_loc(&self, ent: &mut Entity, sim_model: &SimModel) -> CommandResult {
        unimplemented!();
        // ent.components.attach(
        //     sim_model,
        //     &mut ent.storage,
        //     self.model_type.as_str(),
        //     self.model_id.as_str(),
        //     self.new_id.as_str(),
        // );
        CommandResult::Continue
    }
}
/// Detach
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Detach {
    pub signature: Address,
    /* pub comp_model_type: SmallString,
     * pub comp_model_id: Option<SmallString>,
     * pub comp_id: SmallString, */
}
impl Detach {
    // TODO develop further
    fn from_str(args_str: &str) -> Result<Self> {
        let split: Vec<&str> = args_str.split(" ").collect();
        let signature = Address::from_str(split[0]).unwrap();
        Ok(Detach { signature })
        // if split.len() == 2 {
        // Ok(Detach {
        // comp_model_type:
        // SmallString::from_str_truncate(split[0]),
        // comp_id: ArrSSmallStringtr10::from_str_truncate(split[1]),
        // comp_model_id: None,
        //})
        //} else if split.len() == 3 {
        // Ok(Detach {
        // comp_model_type:
        // SmallString::from_str_truncate(split[0]),
        // comp_id: SmallString::from_str_truncate(split[1]),
        // comp_model_id:
        // Some(SmallString::from_str_truncate(split[2])),
        //})
        //} else {
        // Err(format!("wrong number of args"))
        //}
    }
    pub fn execute_loc(&self, ent: &mut Entity, sim_model: &SimModel) -> CommandResult {
        unimplemented!();
        // let comp_model = sim_model
        //     .get_component(
        //         &ent.model_type,
        //         &self.signature.get_comp_type_safe().unwrap(),
        //         &self.signature.get_comp_id_safe().unwrap(),
        //     )
        //     .unwrap();
        // ent.components.detach(
        //     &mut ent.storage,
        //     &comp_model,
        //     //&self.signature.comp_type.unwrap(),
        //     &self.signature.get_comp_id_safe().unwrap(),
        // );
        // CommandResult::Continue
    }
}

/// Goto
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Goto {
    pub target_state: StringId,
}
impl Goto {
    fn from_str(args_str: &str) -> Result<Self> {
        Ok(Goto {
            target_state: StringId::from_truncate(args_str),
        })
    }
    pub fn execute_loc(&self) -> CommandResult {
        unimplemented!();
        //CommandResult::GoToState(self.target_state.clone())
    }
}

/// Jump
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Jump {
    pub target_cmd: u16,
}
impl Jump {
    fn from_str(args_str: &str) -> Result<Self> {
        Ok(Jump {
            target_cmd: args_str.parse::<u16>().unwrap(),
        })
    }
    pub fn execute_loc(&self) -> CommandResult {
        CommandResult::JumpToLine(self.target_cmd as usize)
    }
}

/// Invoke
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoke {
    pub events: Vec<StringId>,
}
impl Invoke {
    pub fn new(args: Vec<String>) -> Result<Self> {
        let mut events = Vec::new();
        for arg in &args {
            if let Ok(event_id) = StringId::from(arg) {
                events.push(event_id);
            } else {
                // throw error
            }
        }
        Ok(Invoke { events })
    }
}
impl Invoke {
    pub fn execute_loc(&self) -> CommandResult {
        return CommandResult::ExecCentralExt(CentralExtCommand::Invoke(self.clone()));
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<()> {
        for event in &self.events {
            if !sim.event_queue.contains(event) {
                sim.event_queue.push(event.to_owned());
            }
        }
        Ok(())
    }
}

/// Spawn
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Spawn {
    pub prefab: Option<StringId>,
    pub spawn_id: Option<StringId>,
}
impl Spawn {
    fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        if args.len() == 0 {
            Ok(Self {
                prefab: None,
                spawn_id: None,
            })
        } else if args.len() == 1 {
            Ok(Self {
                prefab: Some(StringId::from_truncate(&args[0])),
                spawn_id: None,
            })
        } else if args.len() == 2 {
            Ok(Self {
                prefab: Some(StringId::from_truncate(&args[0])),
                spawn_id: Some(StringId::from_truncate(&args[1])),
            })
        } else {
            return Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody(
                    "`spawn` can't accept more than 2 arguments".to_string(),
                ),
            ));
        }
    }
    // pub fn from_str(args_str: &str) -> MachineResult<Self> {
    //     let split: Vec<&str> = args_str.split(" ").collect();
    //     if split.len() < 3 {
    //         return Err(MachineError::Initialization(format!(
    //             "spawn needs at least 3 arguments"
    //         )));
    //     }
    //     return Ok(Spawn {
    //         prefab: match StringIndex::from_str(split[0]) {
    //             Ok(a) => a,
    //             Err(e) => return Err(MachineError::Initialization(format!("{}", split[0]))),
    //         },
    //         spawn_id: StringIndex::from_str(split[1]).unwrap(),
    //         /* model_type: SmallString::from_str("reg").unwrap(),
    //          * model_id: SmallString::from_str("germany").unwrap(),
    //          * spawn_id: SmallString::from_str("ger_new").unwrap(), */
    //     });
    // }
    pub fn execute_loc(&self) -> CommandResult {
        CommandResult::ExecCentralExt(CentralExtCommand::Spawn(*self))
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<()> {
        // let model = &sim.model;
        // let my_model_n = model
        //.entities
        //.iter()
        //.enumerate()
        //.find(|(n, e)| &e.type_ == self.model_type.as_str() &&
        //.find(|(n, &e.id == self.model_id.as_str())
        //.map(|(n, _)| n)
        //.unwrap();
        sim.spawn_entity(self.prefab.as_ref(), self.spawn_id);
        #[cfg(feature = "machine_lua")]
        sim.setup_lua_state_ent();
        Ok(())
    }
    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        central
            .spawn_entity(self.prefab.clone(), self.spawn_id.clone())
            .unwrap();
        Ok(())
    }
}
