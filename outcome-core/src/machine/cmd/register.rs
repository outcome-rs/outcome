//! This module defines commands used for assembling the model at runtime.
//! This is done by registering different parts of the model one at a time.

use std::path::PathBuf;
use std::str::FromStr;

use shlex::Shlex;

use crate::address::{Address, LocalAddress, ShortLocalAddress};
use crate::entity::Storage;
use crate::model::{ComponentModel, EntityPrefab, EventModel, LogicModel, SimModel};
use crate::sim::Sim;
use crate::var::Var;
use crate::{arraystring, CompName, MedString, ShortString, StringId};

#[cfg(feature = "machine_script")]
use super::super::script::parse_script_at;

use super::super::LocationInfo;
use super::{CentralRemoteCommand, CommandResult};
use crate::distr::SimCentral;
use crate::machine;
use crate::machine::cmd::flow::component::ComponentBlock;
use crate::machine::cmd::Command;
use crate::machine::error::{Error, ErrorKind, Result};
use crate::machine::{CallInfo, CallStackVec, CommandPrototype, ComponentCallInfo};
use crate::var::VarType::Str;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterVar {
    comp: CompName,
    addr: ShortLocalAddress,
    val: Option<Var>,
}
impl RegisterVar {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        let addr = match ShortLocalAddress::from_str(&args[0]) {
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
                    comp: CompName::new(),
                    addr,
                    val: None,
                })
            }
            2 => {
                if args[1] != "=" {
                    let val = Var::from_str(&args[1], Some(addr.var_type))?;
                    return Ok(RegisterVar {
                        comp: CompName::new(),
                        addr,
                        val: Some(val),
                    });
                }
            }
            3 => {
                let val = match args[1].as_str() {
                    "=" => Var::from_str(&args[2], Some(addr.var_type))?,
                    _ => Var::from_str(&args[1], Some(addr.var_type))?,
                };
                return Ok(RegisterVar {
                    comp: CompName::new(),
                    addr,
                    val: Some(val),
                });
            }
            _ => (),
        }
        Err(Error::new(
            *location,
            ErrorKind::InvalidCommandBody("failed".to_string()),
        ))
    }

    pub fn execute_loc(&self, call_stack: &mut CallStackVec) -> Vec<CommandResult> {
        let mut out_vec = Vec::new();
        let mut new_reg_var = self.clone();
        if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
            CallInfo::Component(c) => Some(c),
            _ => None,
        }) {
            new_reg_var.comp = comp_info.name;
        }
        out_vec.push(CommandResult::ExecCentralExt(
            CentralRemoteCommand::RegisterVar(new_reg_var),
        ));
        out_vec.push(CommandResult::Continue);
        return out_vec;
    }

    pub fn execute_ext(
        &self,
        sim: &mut Sim,
        ent_name: &crate::EntityId,
        comp_name: &crate::CompName,
    ) -> Result<()> {
        debug!("registering var: {:?}", self);

        sim.model
            .get_component_mut(&self.comp)
            .unwrap()
            .vars
            .push(crate::model::VarModel {
                id: self.addr.var_id,
                type_: self.addr.var_type,
                default: self.val.clone(),
            });
        Ok(())
    }

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        debug!("registering var: {:?}", self);

        central
            .model
            .get_component_mut(&self.comp)
            .unwrap()
            .vars
            .push(crate::model::VarModel {
                id: self.addr.var_id,
                type_: self.addr.var_type,
                default: self.val.clone(),
            });
        Ok(())
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
        let comp_signature = arraystring::new_truncate(&args[0]);
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
        CommandResult::ExecCentralExt(CentralRemoteCommand::Extend(self.clone()))
    }
}

/// Register an entity prefab, specifying a name and a set of components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterEntityPrefab {
    /// Name of the entity prefab
    name: StringId,
    /// List of components defining the prefab
    components: Vec<StringId>,
}

impl RegisterEntityPrefab {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        Ok(Self {
            name: arraystring::new_truncate(&args[0]),
            components: args
                .iter()
                .skip(1)
                .map(|a| arraystring::new_truncate(a))
                .collect(),
        })
    }

    pub fn execute_loc(&self) -> Vec<CommandResult> {
        debug!("registering entity prefab (loc)");
        vec![
            CommandResult::ExecCentralExt(CentralRemoteCommand::RegisterEntityPrefab(Self {
                name: self.name,
                components: self.components.clone(),
            })),
            CommandResult::Continue,
        ]
    }

    pub fn execute_ext(&self, sim: &mut Sim) -> Result<()> {
        sim.model.entities.push(EntityPrefab {
            name: self.name,
            components: self.components.clone(),
        });
        Ok(())
    }

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        central.model.entities.push(EntityPrefab {
            name: self.name,
            components: self.components.clone(),
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterComponent {
    pub name: StringId,
    pub trigger_events: Vec<StringId>,
    pub source_comp: StringId,
    pub start_line: usize,
    pub end_line: usize,
}

impl RegisterComponent {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Command> {
        // println!("{:?}", args);
        let mut options = getopts::Options::new();
        options.optflag("", "marker", "");

        let matches = options.parse(&args).unwrap();
        if matches.opt_present("marker") && !matches.free.is_empty() {
            Ok(Command::RegisterComponent(Self {
                name: arraystring::new_truncate(&matches.free[0]),
                trigger_events: vec![],
                source_comp: Default::default(),
                start_line: 0,
                end_line: 0,
            }))
        } else {
            ComponentBlock::new(args, location, commands)
        }
    }

    pub fn execute_loc(&self, call_stack: &mut CallStackVec) -> Vec<CommandResult> {
        // trace!("executing component block: {:?}", self);
        //
        // let mut new_self = self.clone();
        //
        // call_stack.push(CallInfo::Component(ComponentCallInfo {
        //     name: new_self.name,
        // }));
        //
        // let mut out_vec = Vec::new();
        // // out_vec.push(CommandResult::ExecCentralExt(
        // //     CentralRemoteCommand::Component(new_self),
        // // ));
        // out_vec.push(CommandResult::ExecCentralExt(
        //     CentralRemoteCommand::RegisterComponent(RegisterComponent {
        //         name: arraystring::new_truncate(&new_self.name),
        //         trigger_events: vec![],
        //         source_comp: self.source_comp,
        //         start_line: self.start_line,
        //         end_line: self.end_line,
        //     }),
        // ));
        // out_vec.push(CommandResult::Continue);
        // out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        // out_vec

        vec![
            CommandResult::ExecCentralExt(CentralRemoteCommand::RegisterComponent(self.clone())),
            CommandResult::Continue,
        ]
    }

    pub fn execute_ext(
        &self,
        sim: &mut Sim,
        ent_name: &crate::EntityId,
        comp_name: &crate::CompName,
    ) -> Result<()> {
        // trace!("executing register component cmd: {:?}", self);

        // let comp_model = sim.model.get_component(&self.source_comp).unwrap();
        // trace!("source_comp: {:?}", comp_model);

        let component = ComponentModel {
            name: self.name.into(),
            triggers: self.trigger_events.clone(),
            // logic: comp_model.logic.get_subset(self.start_line, self.end_line),
            // logic: LogicModel {
            //     commands: comp_model.logic.commands.clone(),
            //     cmd_location_map: comp_model.logic.cmd_location_map.clone(),
            //     ..LogicModel::default()
            // },
            // logic: comp_model.logic.get_subset(self.start_line, self.end_line),
            ..ComponentModel::default()
        };

        debug!("registering component: {:?}", component.name);
        sim.model.components.push(component);

        // let comp_model = ComponentModel {
        //     name: StringId::from_truncate(&reg.name.to_string()),
        //     vars: Vec::new(),
        //     start_state: StringId::from_unchecked("init"),
        //     triggers: reg.trigger_events.clone(),
        //     // triggers: vec![ShortString::from_str_truncate("step")],
        //     logic: crate::model::LogicModel::empty(),
        //     source_files: Vec::new(),
        //     script_files: Vec::new(),
        //     lib_files: Vec::new(),
        // };
        // debug!("registering component: {:?}", comp_model);
        // sim.model.components.push(comp_model);

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

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        let component = ComponentModel {
            name: self.name.into(),
            triggers: self.trigger_events.clone(),
            ..ComponentModel::default()
        };
        trace!(
            "execute_ext_distr: registering component: {:?}",
            component.name
        );
        central.model.components.push(component);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTrigger {
    pub name: StringId,
    pub comp: CompName,
}

impl RegisterTrigger {
    pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
        // TODO handle cases with wrong number of args

        Ok(RegisterTrigger {
            name: arraystring::new_truncate(&args[0]),
            comp: Default::default(),
        })
    }
    pub fn execute_loc(&self, call_stack: &mut CallStackVec) -> Vec<CommandResult> {
        let mut new_reg_trigger = self.clone();
        if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
            CallInfo::Component(c) => Some(c),
            _ => None,
        }) {
            new_reg_trigger.comp = comp_info.name;
            debug!("Register::Trigger: comp_info.name: {}", comp_info.name);
        } else {
            // error: called outside of component block
        }

        vec![
            CommandResult::ExecCentralExt(CentralRemoteCommand::RegisterTrigger(new_reg_trigger)),
            CommandResult::Continue,
        ]
    }
    pub fn execute_ext(
        &self,
        sim: &mut Sim,
        ent_name: &crate::EntityId,
        comp_name: &crate::CompName,
    ) -> Result<()> {
        debug!("registering comp trigger: {:?}", self);

        sim.model
            .get_component_mut(&self.comp)
            .unwrap()
            .triggers
            .push(self.name);
        Ok(())
    }

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        debug!("registering trigger: {:?}", self);

        central
            .model
            .get_component_mut(&self.comp)
            .unwrap()
            .triggers
            .push(self.name);

        Ok(())
    }
}

// impl Register {
//     pub fn new(args: Vec<String>, location: &LocationInfo) -> Result<Self> {
//         let mut options = getopts::Options::new();
//         let cmd_name = "register";
//
//         let reg = match args[0].as_str() {
//             "entity" => {}
//             "component" => {
//                 let brief = format!("usage: {} component <signature> [options]", cmd_name);
//                 options.optopt(
//                     "t",
//                     "trigger",
//                     "list of events that will trigger processing of this component",
//                     "EVENTS",
//                 );
//                 options.optflag(
//                     "a",
//                     "attach",
//                     "whether to attach the component when applying model",
//                 );
//                 let matches = match options.parse(&args[1..]) {
//                     Ok(m) => m,
//                     Err(e) => {
//                         return Err(Error::new(
//                             *location,
//                             ErrorKind::InvalidCommandBody(format!(
//                                 "{}, {}",
//                                 e,
//                                 options.usage(&brief)
//                             )),
//                         ))
//                     }
//                 };
//                 if matches.free.len() < 1 {
//                     return Err(Error::new(
//                         *location,
//                         ErrorKind::InvalidCommandBody(format!(
//                             "{}, {}",
//                             "signature missing",
//                             options.usage(&brief)
//                         )),
//                     ));
//                 }
//                 let trigger_events: Vec<StringId> = match matches.opt_str("trigger") {
//                     Some(str) => str
//                         .split(',')
//                         .map(|s| StringId::from_truncate(s))
//                         .collect::<Vec<StringId>>(),
//                     None => Vec::new(),
//                 };
//                 RegisterComponent {
//                     name: StringId::from_truncate(&matches.free[0]),
//                     trigger_events,
//                     source_comp: Default::default(),
//                     start_line: 0,
//                     end_line: 0,
//                 }
//             }
//             "event" => Register::Event,
//             "var" => Register::Var(RegisterVar {
//                 comp: CompId::new(),
//                 addr: LocalAddress::from_str(&args[1]).unwrap(),
//                 val: None,
//             }),
//             "trigger" => Register::Trigger(RegisterTrigger {
//                 name: StringId::from_truncate(&args[1]),
//                 comp: CompId::new(),
//             }),
//             _ => {
//                 return Err(Error::new(
//                     *location,
//                     ErrorKind::InvalidCommandBody("invalid register kind".to_string()),
//                 ))
//             }
//         };
//         Ok(reg)
//     }
//
//     pub fn execute_loc(&self, call_stack: &mut CallStackVec) -> Vec<CommandResult> {
//         let mut out_vec = Vec::new();
//         match &self {
//             // Register::Entity()
//             Register::Component(reg_comp) => {
//                 out_vec.push(CommandResult::ExecCentralExt(
//                     CentralRemoteCommand::Register(self.clone()),
//                 ));
//             }
//             Register::Var(reg_var) => {
//                 let mut new_reg_var = reg_var.clone();
//                 if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
//                     CallInfo::Component(c) => Some(c),
//                     _ => None,
//                 }) {
//                     new_reg_var.comp = comp_info.name;
//                     // debug!("comp_info.name: {}", comp_info.name);
//                 }
//                 out_vec.push(CommandResult::ExecCentralExt(
//                     CentralRemoteCommand::Register(Register::Var(new_reg_var)),
//                 ));
//             }
//             Register::Trigger(reg_trigger) => {
//                 let mut new_reg_trigger = reg_trigger.clone();
//                 if let Some(comp_info) = call_stack.iter().find_map(|ci: &CallInfo| match ci {
//                     CallInfo::Component(c) => Some(c),
//                     _ => None,
//                 }) {
//                     new_reg_trigger.comp = comp_info.name;
//                     debug!("Register::Trigger: comp_info.name: {}", comp_info.name);
//                 }
//                 out_vec.push(CommandResult::ExecCentralExt(
//                     CentralRemoteCommand::Register(Register::Trigger(new_reg_trigger)),
//                 ));
//             }
//             //     RegisterKind::Entity(ref mut reg) => reg.signature.resolve_loc(storage),
//             _ => (),
//         }
//         out_vec.push(CommandResult::Continue);
//         return out_vec;
//         // println!("{:?}", self);
//     }
//     pub fn execute_ext(
//         &self,
//         sim: &mut Sim,
//         ent_name: &crate::EntityId,
//         comp_name: &crate::CompId,
//     ) -> Result<()> {
//         match &self {
//             Register::Entity(reg) => {
//                 // debug!("registering entity");
//                 // let signature = Address::from_str(&self.args[0]).unwrap().resolve(sim);
//                 // println!("{:?}", signature);
//                 let mut ent_model = EntityPrefabModel {
//                     name: StringId::from_truncate(&reg.name.to_string()),
//                     components: Vec::new(),
//                 };
//                 sim.model.entities.push(ent_model);
//
//                 // if do_spawn {
//                 //     sim.add_entity(
//                 //         &signature.get_ent_type_safe().unwrap(),
//                 //         &signature.get_ent_id_safe().unwrap(),
//                 //         &signature.get_ent_id_safe().unwrap(),
//                 //     );
//                 // }
//
//                 // CommandResult::Ok
//                 Ok(())
//             }
//             Register::Component(reg) => {
//                 trace!("executing register component cmd: {:?}", reg);
//
//                 let comp_model = sim.model.get_component(&reg.source_comp).unwrap();
//                 trace!("source_comp: {:?}", comp_model);
//
//                 let component = ComponentModel {
//                     name: reg.name.into(),
//                     start_state: StringId::from_unchecked("init"),
//                     triggers: reg.trigger_events.clone(),
//                     // logic: LogicModel {
//                     //     commands: comp_model.logic.commands.clone(),
//                     //     cmd_location_map: comp_model.logic.cmd_location_map.clone(),
//                     //     ..LogicModel::default()
//                     // },
//                     logic: comp_model.logic.get_subset(reg.start_line, reg.end_line),
//                     ..ComponentModel::default()
//                 };
//
//                 debug!("registering component: {:?}", comp_model);
//                 sim.model.components.push(component);
//
//                 // let comp_model = ComponentModel {
//                 //     name: StringId::from_truncate(&reg.name.to_string()),
//                 //     vars: Vec::new(),
//                 //     start_state: StringId::from_unchecked("init"),
//                 //     triggers: reg.trigger_events.clone(),
//                 //     // triggers: vec![ShortString::from_str_truncate("step")],
//                 //     logic: crate::model::LogicModel::empty(),
//                 //     source_files: Vec::new(),
//                 //     script_files: Vec::new(),
//                 //     lib_files: Vec::new(),
//                 // };
//                 // debug!("registering component: {:?}", comp_model);
//                 // sim.model.components.push(comp_model);
//
//                 // if reg_comp.do_attach {
//                 //     for (&(ent_type, ent_id), mut entity) in &mut sim.entities {
//                 //         if &ent_type.as_str() == &addr.get_ent_type_safe().unwrap().as_str() {
//                 //             // entity.components.attach()
//                 //             entity.components.attach(
//                 //                 &sim.model,
//                 //                 &mut entity.storage,
//                 //                 &addr.get_comp_type_safe().unwrap(),
//                 //                 &addr.get_comp_id_safe().unwrap(),
//                 //                 &addr.get_comp_id_safe().unwrap(),
//                 //             );
//                 //         }
//                 //     }
//                 // }
//
//                 Ok(())
//             }
//             Register::Event => Ok(()),
//             Register::Var(reg) => {
//                 debug!("registering var: {:?}", reg);
//
//                 sim.model
//                     .get_component_mut(&reg.comp)
//                     .unwrap()
//                     .vars
//                     .push(crate::model::VarModel {
//                         id: reg.addr.var_id.to_string(),
//                         type_: reg.addr.var_type,
//                         default: reg.val.clone(),
//                     });
//                 Ok(())
//
//                 //let mut comp_type_model = ComponentTypeModel {
//                 //id: signature.get_comp_type_safe().unwrap().to_string(),
//                 //entity_type: signature.get_ent_type_safe().unwrap().to_string(),
//                 //};
//                 //sim.model.component_types.push(comp_type_model);
//             }
//             Register::Trigger(reg) => {
//                 debug!("registering comp trigger: {:?}", reg);
//
//                 sim.model
//                     .get_component_mut(&reg.comp)
//                     .unwrap()
//                     .triggers
//                     .push(reg.name);
//                 Ok(())
//
//                 //let mut comp_type_model = ComponentTypeModel {
//                 //id: signature.get_comp_type_safe().unwrap().to_string(),
//                 //entity_type: signature.get_ent_type_safe().unwrap().to_string(),
//                 //};
//                 //sim.model.component_types.push(comp_type_model);
//             }
//             _ => Ok(()),
//         }
//     }
//
//     pub fn execute_ext_distr(
//         &self,
//         central: &mut SimCentral,
//         ent_name: &crate::EntityId,
//         comp_name: &crate::CompId,
//     ) -> Result<()> {
//         match &self {
//             Register::Entity(reg) => {
//                 debug!("registering entity prefab");
//                 let mut ent_model = EntityPrefabModel {
//                     name: StringId::from_truncate(&reg.name.to_string()),
//                     components: reg.components.clone(),
//                 };
//                 central.model.entities.push(ent_model);
//                 Ok(())
//             }
//             Register::Component(reg) => {
//                 debug!("registering component");
//                 let comp_model = ComponentModel {
//                     name: StringId::from_truncate(&reg.name.to_string()),
//                     vars: Vec::new(),
//                     start_state: StringId::from_unchecked("idle"),
//                     triggers: reg.trigger_events.clone(),
//                     // triggers: vec![ShortString::from_str_truncate("step")],
//                     logic: LogicModel::empty(),
//                     source_files: Vec::new(),
//                     script_files: Vec::new(),
//                     lib_files: Vec::new(),
//                 };
//                 // central.model_changes_queue.components.push(comp_model);
//                 central.model.components.push(comp_model);
//
//                 // if reg_comp.do_attach {
//                 //     for (&(ent_type, ent_id), mut entity) in &mut sim.entities {
//                 //         if &ent_type.as_str() == &addr.get_ent_type_safe().unwrap().as_str() {
//                 //             // entity.components.attach()
//                 //             entity.components.attach(
//                 //                 &sim.model,
//                 //                 &mut entity.storage,
//                 //                 &addr.get_comp_type_safe().unwrap(),
//                 //                 &addr.get_comp_id_safe().unwrap(),
//                 //                 &addr.get_comp_id_safe().unwrap(),
//                 //             );
//                 //         }
//                 //     }
//                 // }
//
//                 Ok(())
//             }
//             Register::Event => Ok(()),
//             Register::Var(reg) => {
//                 debug!("registering var: {:?}", reg);
//
//                 central
//                     // .model_changes_queue
//                     .model
//                     .get_component_mut(&reg.comp)
//                     .unwrap()
//                     .vars
//                     .push(crate::model::VarModel {
//                         id: reg.addr.var_id.to_string(),
//                         type_: reg.addr.var_type,
//                         default: reg.val.clone(),
//                     });
//                 Ok(())
//
//                 //let mut comp_type_model = ComponentTypeModel {
//                 //id: signature.get_comp_type_safe().unwrap().to_string(),
//                 //entity_type: signature.get_ent_type_safe().unwrap().to_string(),
//                 //};
//                 //sim.model.component_types.push(comp_type_model);
//             }
//             Register::Trigger(reg) => {
//                 debug!("registering trigger: {:?}", reg);
//
//                 central
//                     .model
//                     .get_component_mut(&reg.comp)
//                     .unwrap()
//                     .triggers
//                     .push(reg.name);
//
//                 Ok(())
//             }
//             _ => Ok(()),
//         }
//     }
// }
