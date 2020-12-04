//! This module defines commands used for assembling the model at runtime.

use std::path::PathBuf;
use std::str::FromStr;

use shlex::Shlex;

use crate::address::{Address, LocalAddress};
use crate::entity::Storage;
use crate::model::{ComponentModel, EntityPrefabModel, EventModel, LogicModel, SimModel};
use crate::sim::interface::SimInterface;
use crate::sim::Sim;
use crate::var::Var;
use crate::{CompId, MedString, ShortString, StringId};

#[cfg(feature = "machine_script")]
use super::super::script::parse_script_at;

use super::super::LocationInfo;
use super::{CentralExtCommand, CommandResult};
use crate::distr::SimCentral;
use crate::machine;
use crate::machine::error::{Error, ErrorKind, Result};
use crate::machine::{CallInfo, CallStackVec};
use crate::var::VarType::Str;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterVar {
    comp: CompId,
    addr: LocalAddress,
    val: Option<Var>,
}
impl RegisterVar {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        let addr = match LocalAddress::from_str(&args[0]) {
            Ok(a) => a,
            Err(e) => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody(format!("{}", e)),
                ))
            }
        };

        match args.len() {
            1 => {
                return Ok(RegisterVar {
                    comp: CompId::new(),
                    addr,
                    val: None,
                })
            }
            // 2 => if args[1] != "=" {
            //     let val
            //     return Ok(RegisterVar {})
            // },
            3 => {
                if args[1] == "=" {
                    return Ok(RegisterVar {
                        comp: CompId::new(),
                        addr,
                        val: Var::from_str(&args[2], None),
                    });
                }
            }
            _ => (),
        }
        Err(Error::new(
            *location,
            ErrorKind::InvalidCommandBody("failed".to_string()),
        ))
    }
    pub fn execute_loc(&self, storage: Storage) -> CommandResult {
        //
        CommandResult::Continue
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extend {
    // args: Vec<String>,
    /// Partial address acting as a signature for target component,
    /// including entity type but not the entity id
    pub(crate) comp_signature: StringId,
    pub(crate) source_files: Vec<String>,
    pub(crate) location: LocationInfo,
}
impl Extend {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        if args.len() < 2 {
            return Err(Error::new(
                *location,
                ErrorKind::InvalidCommandBody(
                    "`extend` command requires at least 2 arguments".to_string(),
                ),
            ));
        }
        let comp_signature = StringId::from_truncate(&args[0]);
        let mut source_files = Vec::new();
        for i in 1..args.len() {
            // check for potential recursion and abort if present
            if &args[i]
                == location
                    .source
                    .as_ref()
                    .unwrap()
                    .rsplitn(2, "/")
                    .collect::<Vec<&str>>()[0]
            {
                trace!("detected recursive !extend, removing: {:?}", location);
                continue;
            }
            source_files.push(args[i].clone());
        }
        return Ok(Extend {
            comp_signature,
            source_files,
            location: location.clone(),
        });
    }
    pub fn execute_loc(&self) -> CommandResult {
        CommandResult::ExecCentralExt(CentralExtCommand::Extend(self.clone()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Register {
    Entity(RegEntity),
    Component(RegComponent),
    Event,
    Var(RegisterVar),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegComponent {
    pub name: StringId,
    pub trigger_events: Vec<StringId>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegEntity {
    pub name: StringId,
    pub components: Vec<StringId>,
    pub do_spawn: bool,
}

impl Register {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        let mut options = getopts::Options::new();
        let cmd_name = "register";

        let reg = match args[0].as_str() {
            "entity" => {
                let brief = format!("usage: {} entity <signature> [options]", cmd_name);
                options.optflag(
                    "s",
                    "spawn",
                    "whether to spawn the entity when applying model",
                );
                let matches = match options.parse(&args[1..]) {
                    Ok(m) => m,
                    Err(e) => {
                        return Err(Error::new(
                            *location,
                            ErrorKind::InvalidCommandBody(format!(
                                "{}, {}",
                                e,
                                options.usage(&brief)
                            )),
                        ))
                    }
                };
                if matches.free.len() < 1 {
                    return Err(Error::new(
                        *location,
                        ErrorKind::InvalidCommandBody(format!(
                            "{}, {}",
                            "signature missing",
                            options.usage(&brief)
                        )),
                    ));
                }
                Register::Entity(RegEntity {
                    name: StringId::from_truncate(&matches.free[0]),
                    components: matches
                        .free
                        .iter()
                        .skip(1)
                        .map(|a| StringId::from_truncate(a))
                        .collect(),
                    do_spawn: matches.opt_present("spawn"),
                })
            }
            "component" => {
                let brief = format!("usage: {} component <signature> [options]", cmd_name);
                options.optopt(
                    "t",
                    "trigger",
                    "list of events that will trigger processing of this component",
                    "EVENTS",
                );
                options.optflag(
                    "a",
                    "attach",
                    "whether to attach the component when applying model",
                );
                let matches = match options.parse(&args[1..]) {
                    Ok(m) => m,
                    Err(e) => {
                        return Err(Error::new(
                            *location,
                            ErrorKind::InvalidCommandBody(format!(
                                "{}, {}",
                                e,
                                options.usage(&brief)
                            )),
                        ))
                    }
                };
                if matches.free.len() < 1 {
                    return Err(Error::new(
                        *location,
                        ErrorKind::InvalidCommandBody(format!(
                            "{}, {}",
                            "signature missing",
                            options.usage(&brief)
                        )),
                    ));
                }
                let trigger_events: Vec<StringId> = match matches.opt_str("trigger") {
                    Some(str) => str
                        .split(',')
                        .map(|s| StringId::from_truncate(s))
                        .collect::<Vec<StringId>>(),
                    None => Vec::new(),
                };
                Register::Component(RegComponent {
                    name: StringId::from_truncate(&matches.free[0]),
                    trigger_events,
                    // do_attach: matches.opt_present("attach"),
                })
            }
            "event" => Register::Event,
            "var" => Register::Var(RegisterVar {
                comp: CompId::new(),
                addr: LocalAddress::from_str(&args[1]).unwrap(),
                val: None,
            }),
            _ => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody("invalid register kind".to_string()),
                ))
            }
        };
        Ok(reg)
    }
    pub fn execute_loc(&self, call_stack: &mut CallStackVec) -> Vec<CommandResult> {
        let mut out_vec = Vec::new();
        match &self {
            // Register::Entity()
            Register::Var(reg_var) => {
                let mut new_reg_var = reg_var.clone();
                if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
                    CallInfo::Component(c) => Some(c),
                    _ => None,
                }) {
                    new_reg_var.comp = comp_info.name;
                    debug!("comp_info.name: {}", comp_info.name);
                }
                out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::Register(
                    Register::Var(new_reg_var),
                )));
            }
            Register::Component(reg_comp) => {
                out_vec.push(CommandResult::ExecCentralExt(CentralExtCommand::Register(
                    self.clone(),
                )));
            }
            //     RegisterKind::Entity(ref mut reg) => reg.signature.resolve_loc(storage),
            _ => (),
        }
        out_vec.push(CommandResult::Continue);
        return out_vec;
        // println!("{:?}", self);
    }
    pub fn execute_ext(
        &self,
        sim: &mut Sim,
        ent_name: &crate::EntityId,
        comp_name: &crate::CompId,
    ) -> Result<()> {
        match &self {
            Register::Entity(reg) => {
                // debug!("registering entity");
                // let signature = Address::from_str(&self.args[0]).unwrap().resolve(sim);
                // println!("{:?}", signature);
                let mut ent_model = EntityPrefabModel {
                    name: StringId::from_truncate(&reg.name.to_string()),
                    components: Vec::new(),
                };
                sim.model.entities.push(ent_model);

                // if do_spawn {
                //     sim.add_entity(
                //         &signature.get_ent_type_safe().unwrap(),
                //         &signature.get_ent_id_safe().unwrap(),
                //         &signature.get_ent_id_safe().unwrap(),
                //     );
                // }

                // CommandResult::Ok
                Ok(())
            }
            Register::Component(reg) => {
                debug!("registering component");
                let comp_model = ComponentModel {
                    name: StringId::from_truncate(&reg.name.to_string()),
                    vars: Vec::new(),
                    start_state: StringId::from_unchecked("idle"),
                    triggers: reg.trigger_events.clone(),
                    // triggers: vec![ShortString::from_str_truncate("step")],
                    logic: crate::model::LogicModel::empty(),
                    source_files: Vec::new(),
                    script_files: Vec::new(),
                    lib_files: Vec::new(),
                };
                sim.model.components.push(comp_model);

                // if reg_comp.do_attach {
                //     for (&(ent_type, ent_id), mut entity) in &mut sim.entities {
                //         if &ent_type.as_str() == &addr.get_ent_type_safe().unwrap().as_str() {
                //             // entity.components.attach()
                //             entity.components.attach(
                //                 &sim.model,
                //                 &mut entity.storage,
                //                 &addr.get_comp_type_safe().unwrap(),
                //                 &addr.get_comp_id_safe().unwrap(),
                //                 &addr.get_comp_id_safe().unwrap(),
                //             );
                //         }
                //     }
                // }

                Ok(())
            }
            Register::Event => Ok(()),
            Register::Var(reg) => {
                debug!("registering var: {:?}", reg);

                sim.model
                    .get_component_mut(&reg.comp)
                    .unwrap()
                    .vars
                    .push(crate::model::VarModel {
                        id: reg.addr.var_id.to_string(),
                        type_: reg.addr.var_type,
                        default: reg.val.clone(),
                    });
                Ok(())

                //let mut comp_type_model = ComponentTypeModel {
                //id: signature.get_comp_type_safe().unwrap().to_string(),
                //entity_type: signature.get_ent_type_safe().unwrap().to_string(),
                //};
                //sim.model.component_types.push(comp_type_model);
            }
            _ => Ok(()),
        }
    }

    pub fn execute_ext_distr(
        &self,
        central: &mut SimCentral,
        ent_name: &crate::EntityId,
        comp_name: &crate::CompId,
    ) -> Result<()> {
        match &self {
            Register::Entity(reg) => {
                debug!("registering entity prefab");
                let mut ent_model = EntityPrefabModel {
                    name: StringId::from_truncate(&reg.name.to_string()),
                    components: reg.components.clone(),
                };
                central.model.entities.push(ent_model);
                Ok(())
            }
            Register::Component(reg) => {
                debug!("registering component");
                let comp_model = ComponentModel {
                    name: StringId::from_truncate(&reg.name.to_string()),
                    vars: Vec::new(),
                    start_state: StringId::from_unchecked("idle"),
                    triggers: reg.trigger_events.clone(),
                    // triggers: vec![ShortString::from_str_truncate("step")],
                    logic: LogicModel::empty(),
                    source_files: Vec::new(),
                    script_files: Vec::new(),
                    lib_files: Vec::new(),
                };
                // central.model_changes_queue.components.push(comp_model);
                central.model.components.push(comp_model);

                // if reg_comp.do_attach {
                //     for (&(ent_type, ent_id), mut entity) in &mut sim.entities {
                //         if &ent_type.as_str() == &addr.get_ent_type_safe().unwrap().as_str() {
                //             // entity.components.attach()
                //             entity.components.attach(
                //                 &sim.model,
                //                 &mut entity.storage,
                //                 &addr.get_comp_type_safe().unwrap(),
                //                 &addr.get_comp_id_safe().unwrap(),
                //                 &addr.get_comp_id_safe().unwrap(),
                //             );
                //         }
                //     }
                // }

                Ok(())
            }
            Register::Event => Ok(()),
            Register::Var(reg) => {
                debug!("registering var: {:?}", reg);

                central
                    // .model_changes_queue
                    .model
                    .get_component_mut(&reg.comp)
                    .unwrap()
                    .vars
                    .push(crate::model::VarModel {
                        id: reg.addr.var_id.to_string(),
                        type_: reg.addr.var_type,
                        default: reg.val.clone(),
                    });
                Ok(())

                //let mut comp_type_model = ComponentTypeModel {
                //id: signature.get_comp_type_safe().unwrap().to_string(),
                //entity_type: signature.get_ent_type_safe().unwrap().to_string(),
                //};
                //sim.model.component_types.push(comp_type_model);
            }
            _ => Ok(()),
        }
    }
}
