//! This module defines commands used for assembling the model at runtime.

use std::path::PathBuf;
use std::str::FromStr;

use shlex::Shlex;

use crate::address::Address;
use crate::entity::Storage;
use crate::model::{ComponentModel, EntityPrefabModel, EventModel, SimModel};
use crate::sim::interface::SimInterface;
use crate::sim::Sim;
use crate::var::Var;
use crate::{MedString, ShortString, StringId};

#[cfg(feature = "machine_script")]
use super::super::script::parse_script_at;

use super::super::LocationInfo;
use super::{CentralExtCommand, CommandResult};
use crate::machine;
use crate::machine::error::{Error, ErrorKind, Result};
use crate::var::VarType::Str;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterVar {
    addr: Address,
    val: Option<Var>,
}
impl RegisterVar {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        let addr = match Address::from_str(&args[0]) {
            Ok(a) => a,
            Err(e) => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody(format!("{}", e)),
                ))
            }
        };

        match args.len() {
            1 => return Ok(RegisterVar { addr, val: None }),
            // 2 => if args[1] != "=" {
            //     let val
            //     return Ok(RegisterVar {})
            // },
            3 => {
                if args[1] == "=" {
                    return Ok(RegisterVar {
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
        let comp_signature = StringId::from(&args[0]).unwrap();
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
pub struct Register {
    kind: RegisterKind,
    // signature: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterKind {
    EntityType(RegEntityType),
    ComponentType(RegComponentType),
    Entity(RegEntity),
    Component(RegComponent),
    Event,
    Var(RegisterVar),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegComponent {
    signature: StringId,
    trigger_events: Vec<ShortString>,
    do_attach: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RegComponentType {
    signature: StringId,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RegEntity {
    signature: StringId,
    do_spawn: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RegEntityType {
    signature: StringId,
}

impl Register {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        let mut options = getopts::Options::new();
        let cmd_name = "register";

        let kind = match args[0].as_str() {
            "entity_type" => RegisterKind::EntityType(RegEntityType {
                signature: StringId::from(&args[1])
                    .map_err(|e| Error::new(*location, ErrorKind::InvalidAddress(e.to_string())))?,
            }),
            "component_type" => RegisterKind::ComponentType(RegComponentType {
                signature: StringId::from(&args[1]).unwrap(),
            }),
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
                RegisterKind::Entity(RegEntity {
                    signature: StringId::from(&matches.free[0]).unwrap(),
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
                let trigger_events: Vec<ShortString> = match matches.opt_str("trigger") {
                    Some(str) => str
                        .split(',')
                        .map(|s| ShortString::from(s).unwrap())
                        .collect::<Vec<ShortString>>(),
                    None => Vec::new(),
                };
                RegisterKind::Component(RegComponent {
                    signature: StringId::from(&matches.free[0]).unwrap(),
                    trigger_events,
                    do_attach: matches.opt_present("attach"),
                })
            }
            "event" => RegisterKind::Event,
            "var" => RegisterKind::Var(RegisterVar {
                addr: Address::from_str(&args[0]).unwrap(),
                val: None,
            }),
            _ => {
                return Err(Error::new(
                    *location,
                    ErrorKind::InvalidCommandBody("invalid register kind".to_string()),
                ))
            }
        };
        Ok(Register { kind })
    }
    pub fn execute_loc(&self) -> CommandResult {
        // match self.kind {
        //     RegisterKind::Entity(ref mut reg) => reg.signature.resolve_loc(storage),
        //     _ => (),
        // }
        // println!("{:?}", self);
        CommandResult::ExecCentralExt(CentralExtCommand::Register(self.clone()))
    }
    pub fn execute_ext(
        &self,
        sim: &mut Sim,
        ent_uid: &crate::EntityId,
        comp_uid: &crate::CompId,
    ) -> Result<()> {
        match &self.kind {
            RegisterKind::Entity(reg) => {
                // debug!("registering entity");
                // let signature = Address::from_str(&self.args[0]).unwrap().resolve(sim);
                // println!("{:?}", signature);
                let mut ent_model = EntityPrefabModel {
                    name: ShortString::from(&reg.signature.to_string()).unwrap(),
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
            RegisterKind::Component(reg) => {
                debug!("registering component");
                let comp_model = ComponentModel {
                    name: ShortString::from(&reg.signature.to_string()).unwrap(),
                    vars: Vec::new(),
                    start_state: ShortString::from("idle").unwrap(),
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
            RegisterKind::Event => Ok(()),
            RegisterKind::Var(reg) => {
                debug!("registering var");

                sim.model
                    .get_component_mut(comp_uid)
                    .unwrap()
                    .vars
                    .push(crate::model::VarModel {
                        id: reg.addr.var_id.to_string(),
                        type_: reg.addr.var_type,
                        default: crate::var::Var::Int(0),
                        internal: false,
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
