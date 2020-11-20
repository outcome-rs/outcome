//! This module defines functionalist for dealing with executing command
//! collections within different contexts

use std::sync::{Arc, Mutex};

use crate::entity::{Entity, EntityNonSer, Storage};
use crate::{Address, CompId, EntityId, StringId};
use crate::{Sim, SimModel};

use super::cmd::{CentralExtCommand, Command, CommandResult, ExtCommand};
use super::{error::Error, CallStackVec, ExecutionContext, LocationInfo, Registry};
use crate::machine::{ErrorKind, Result};

/// Executes a given set of central-external commands.
//TODO missing component uid information
pub(crate) fn execute_central_ext(
    central_ext_cmds: &Vec<(ExecutionContext, CentralExtCommand)>,
    sim: &mut Sim,
) -> Result<()> {
    for (exe_loc, central_ext_cmd) in central_ext_cmds {
        if let Err(me) = central_ext_cmd.execute(sim, &exe_loc.ent, &exe_loc.comp) {
            error!("{}", me);
        }
    }

    Ok(())
}
/// Executes a given set of external commands.
//TODO missing component uid information
pub(crate) fn execute_ext(
    ext_cmds: &Vec<(ExecutionContext, ExtCommand)>,
    sim: &mut Sim,
) -> Result<()> {
    for (exe_loc, ext_cmd) in ext_cmds {
        ext_cmd.execute(sim, &exe_loc.ent, &exe_loc.comp)?;
    }
    Ok(())
}

/// Executes a given set of commands within a local entity scope.
///
/// Most of the errors occurring during execution of commands are non-breaking.
/// If a breaking error occurs this function will itself return an `Error`
/// containing the reason and location info of the appropriate command.
/// If no breaking errors occur it returns `Ok`.
///
/// ### External command collection arguments
///
/// Arguments include references to atomically counted collections of `ext` and
/// `central_ext` commands. This is because some commands executed on the
/// entity level that are targeting execution in a higher context will yield
/// command results containing either `ext` or `central_ext` commands.
///
/// ### Optional start and end line arguments
///
/// Execution can optionally be restricted to a subset of all commands using
/// the start and end line numbers. This is used when executing a selected
/// state, since states are essentially described using their start and end
/// line numbers.
pub(crate) fn execute_loc(
    cmds: &Vec<Command>,
    mut ent_storage: &mut Storage,
    mut ent_insta: &mut EntityNonSer,
    mut comp_state: &mut StringId,
    ent_uid: &EntityId,
    comp_uid: &CompId,
    sim_model: &SimModel,
    ext_cmds: &Arc<Mutex<Vec<(ExecutionContext, ExtCommand)>>>,
    central_ext_cmds: &Arc<Mutex<Vec<(ExecutionContext, CentralExtCommand)>>>,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<()> {
    // initialize a new call stack
    let mut call_stack = CallStackVec::new();
    let mut registry = Registry::new();
    let mut cmd_n = match start {
        Some(s) => s,
        None => 0,
    };
    'outer: loop {
        if cmd_n >= cmds.len() {
            break;
        }
        if let Some(e) = end {
            if call_stack.is_empty() && cmd_n >= e {
                break;
            }
        }
        let loc_cmd = cmds.get(cmd_n).unwrap();
        let location_info = sim_model
            .get_component(comp_uid)
            .unwrap()
            .logic
            .cmd_location_map
            .get(&cmd_n)
            .ok_or(Error::new(LocationInfo::empty(), ErrorKind::Panic))?;
        // let mut comp = entity.components.get_mut(&comp_uid).unwrap();
        let results = loc_cmd.execute(
            &mut ent_storage,
            &mut ent_insta,
            &mut comp_state,
            &mut call_stack,
            &mut registry,
            comp_uid,
            ent_uid,
            &sim_model,
            location_info,
        );
        for result in results {
            match result {
                CommandResult::Continue => (),
                CommandResult::Break => break 'outer,
                CommandResult::JumpToLine(n) => {
                    cmd_n = n;
                    continue 'outer;
                }
                CommandResult::JumpToTag(_) => unimplemented!(),
                CommandResult::ExecExt(ext_cmd) => {
                    // push external command to an aggregate vec
                    ext_cmds.lock().unwrap().push((
                        ExecutionContext {
                            ent: *ent_uid,
                            comp: *comp_uid,
                            location: *location_info,
                        },
                        ext_cmd,
                    ));
                }
                CommandResult::ExecCentralExt(cext_cmd) => {
                    // push central external command to an aggregate vec
                    central_ext_cmds.lock().unwrap().push((
                        ExecutionContext {
                            ent: *ent_uid,
                            comp: *comp_uid,
                            location: *location_info,
                        },
                        cext_cmd,
                    ));
                }
                CommandResult::Err(e) => {
                    //TODO implement configurable system for deciding whether to
                    // break state, panic or just print when given error occurs
                    error!("{}", e);
                }
            }
        }
        cmd_n += 1;
    }
    Ok(())
}
/// Executes given set of commands within global sim scope.
pub fn execute(
    cmds: &Vec<Command>,
    ent_uid: &EntityId,
    comp_uid: &CompId,
    mut sim: &mut Sim,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<()> {
    // initialize a new call stack
    let mut call_stack = CallStackVec::new();
    let mut registry = Registry::new();

    let mut empty_locinfo = LocationInfo::empty();
    empty_locinfo.line = Some(0);

    let mut cmd_n = match start {
        Some(s) => s,
        None => 0,
    };
    'outer: loop {
        if cmd_n >= cmds.len() {
            break;
        }
        if let Some(e) = end {
            if call_stack.is_empty() && cmd_n >= e {
                break;
            }
        }
        let loc_cmd = cmds.get(cmd_n).unwrap();
        let location = sim
            .model
            .get_component(comp_uid)
            .expect("can't get component model")
            .logic
            .cmd_location_map
            .get(&cmd_n)
            .unwrap_or(&empty_locinfo);

        let entity = match sim.entities.get_mut(sim.entities_idx.get(ent_uid).unwrap()) {
            Some(e) => e,
            None => {
                unimplemented!();
                // error!(
                //     "{}",
                //     Error::FailedGettingComponent(
                //         Address::from_uids(ent_uid, comp_uid),
                //         location_info.clone(),
                //     )
                // );
                cmd_n += 1;
                continue;
            }
        };
        let mut comp_state = match entity.comp_state.get_mut(comp_uid) {
            Some(c) => c,
            None => {
                error!(
                    "{}",
                    //todo
                    Error::new(*location, ErrorKind::Initialization("".to_string()))
                );
                cmd_n += 1;
                continue;
            }
        };
        let results = loc_cmd.execute(
            &mut entity.storage,
            &mut entity.insta,
            &mut comp_state,
            &mut call_stack,
            &mut registry,
            comp_uid,
            ent_uid,
            &sim.model,
            location,
        );
        for result in results {
            match result {
                CommandResult::Continue => (),
                CommandResult::Break => break 'outer,
                CommandResult::JumpToLine(n) => {
                    cmd_n = n;
                    continue 'outer;
                }
                CommandResult::JumpToTag(_) => unimplemented!("jumping to tag not supported"),
                CommandResult::ExecExt(ext_cmd) => {
                    ext_cmd.execute(sim, ent_uid, comp_uid)?;
                }
                CommandResult::ExecCentralExt(cext_cmd) => {
                    cext_cmd.execute(sim, ent_uid, comp_uid)?;
                }
                CommandResult::Err(e) => {
                    //TODO implement configurable system for deciding whether to
                    // break state, panic or just print when given error occurs
                    error!("{}", e);
                }
            }
        }
        cmd_n += 1;
    }
    Ok(())
}
