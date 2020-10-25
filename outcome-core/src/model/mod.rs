//! Contains definitions for model objects, as well as logic for turning
//! deserialized data into model objects.
//!
//! `SimModel` object contains a collection of all the model objects as found
//! in user files. It can be used to spawn a simulation instance. Spawning an
//! object at runtime requires a reference to a model.

#![allow(unused)]

extern crate semver;
extern crate serde;
// extern crate serde_yaml;
extern crate toml;

// mod dyn_deser;
mod deser;

// pub use model::dyn_deser::*;

use std::collections::HashMap;
use std::fs::{read, read_dir, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use fnv::FnvHashMap;

use crate::Result;
use crate::{util, ShortString};
use crate::{MedString, StringId};
use crate::{Var, VarType};
use crate::{
    MODULE_ENTRY_SCRIPT_NAME, MODULE_MANIFEST_FILE, SCENARIO_MANIFEST_FILE, SCENARIO_MODS_DIR_NAME,
    VERSION,
};

use crate::address::Address;
use crate::error::Error;
//use crate::script::bridge::FILE_EXTENSION;

#[cfg(feature = "machine_script")]
use crate::machine::script::{parser, preprocessor, InstructionKind};

//use crate::machine::cmd::Command;
//use crate::machine::{cmd, CommandPrototype, LocationInfo};

use self::semver::{Version, VersionReq};
use self::toml::Value;

/// Simulation model is a basis for creating simulation instance. It contains
/// all the relevant elements found in the user-files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimModel {
    pub scenario: Scenario,
    pub events: Vec<EventModel>,
    pub scripts: Vec<String>,
    pub entities: Vec<EntityPrefabModel>,
    pub components: Vec<ComponentModel>,
    pub data: Vec<DataEntry>,
    pub data_files: Vec<DataFileEntry>,
    pub data_imgs: Vec<DataImageEntry>,
}

impl SimModel {
    pub fn from_scenario(scenario: Scenario) -> Result<SimModel> {
        // first create an empty sim model
        let mut model = SimModel {
            scenario: scenario.clone(),
            events: Vec::new(),
            scripts: Vec::new(),
            entities: Vec::new(),
            components: Vec::new(),
            data: Vec::new(),
            data_files: Vec::new(),
            data_imgs: Vec::new(),
        };

        let singleton_model = EntityPrefabModel {
            name: ShortString::from("singleton").unwrap(),
            components: vec![ShortString::from("mod_init").unwrap()],
        };
        model.entities.push(singleton_model);

        let mut mod_init_comp_model = ComponentModel {
            name: ShortString::from("mod_init").unwrap(),
            // optionally add some vars
            vars: vec![
                VarModel {
                    id: "main".to_string(),
                    type_: VarType::Int,
                    default: Var::Int(666),
                    internal: false,
                },
                VarModel {
                    id: "main".to_string(),
                    type_: VarType::Float,
                    default: Var::Float(6.666),
                    internal: false,
                },
                VarModel {
                    id: "main".to_string(),
                    type_: VarType::IntList,
                    default: Var::IntList(vec![0; 10]),
                    internal: false,
                },
            ],
            start_state: ShortString::from("init").unwrap(),
            triggers: vec![ShortString::from("init").unwrap()],
            // triggers: vec![],
            #[cfg(feature = "machine")]
            logic: LogicModel {
                commands: Vec::new(),
                states: FnvHashMap::default(),
                procedures: FnvHashMap::default(),
                cmd_location_map: FnvHashMap::default(),
                pre_commands: FnvHashMap::default(),
            },
            source_files: Vec::new(),
            script_files: Vec::new(),
            lib_files: Vec::new(),
            // model_uid: 0,
        };
        model.components.push(mod_init_comp_model);

        model.events.push(EventModel {
            id: ShortString::from("init").unwrap(),
        });

        // add hardcoded content
        model.events.push(crate::model::EventModel {
            id: ShortString::from(crate::DEFAULT_TRIGGER_EVENT).unwrap(),
        });

        #[cfg(feature = "machine_script")]
        {
            #[cfg(feature = "machine")]
            use crate::machine::{cmd::Command, CommandPrototype, LocationInfo};

            // use script processor to handle scripts
            let program_data = crate::machine::script::util::get_program_metadata();

            // iterate over scenario modules
            for module in &scenario.modules {
                // create path to entry script
                // TODO build the file name from available static vars
                let mod_entry_file_path = scenario
                    .path
                    .join(crate::SCENARIO_MODS_DIR_NAME)
                    .join(&module.manifest.name)
                    .join(format!("{}{}", crate::MODULE_ENTRY_SCRIPT_NAME, ".os"));

                // TODO remove unwrap
                // parse the module entry script
                let mut instructions =
                    parser::parse_script_at(mod_entry_file_path.to_str().unwrap())?;

                // preprocess entry script
                preprocessor::run(&mut instructions, &mut model, &program_data)?;

                // turn instructions into proper commands
                let mut commands: Vec<Command> = Vec::new();
                // first get a list of commands from the main instruction list
                let mut cmd_prototypes: Vec<CommandPrototype> = Vec::new();
                let mut cmd_locations: Vec<LocationInfo> = Vec::new();
                for instruction in instructions {
                    let cmd_prototype = match instruction.kind {
                        InstructionKind::Command(c) => c,
                        _ => continue,
                    };
                    cmd_prototypes.push(cmd_prototype);
                    cmd_locations.push(instruction.location.clone());
                }

                let ent_uid = (
                    StringId::from("singleton").unwrap(),
                    StringId::from("0").unwrap(),
                );
                let (ent_model_type, _) = ent_uid;
                let comp_name = StringId::from("mod_init").unwrap();
                let mut comp_model = model.get_component_mut(&comp_name).unwrap();

                for (n, cmd_prototype) in cmd_prototypes.iter().enumerate() {
                    cmd_locations[n].line = Some(n);
                    let command =
                        Command::from_prototype(cmd_prototype, &cmd_locations[n], &cmd_prototypes)?;
                    commands.push(command.clone());
                    //// insert the commands into the component's logic model
                    if let Command::Procedure(proc) = &command {
                        comp_model
                            .logic
                            .procedures
                            .insert(proc.name.clone(), (proc.start_line, proc.end_line));
                    }
                    comp_model.logic.commands.push(command);
                    comp_model
                        .logic
                        .cmd_location_map
                        //.insert(comp_model.logic.commands.len() - 1, location.clone());
                        .insert(n, cmd_locations[n].clone());
                }

                // crate::machine::exec::execute(
                //     &commands,
                //     &ent_model_type,
                //     &comp_name,
                //     &mut sim,
                //     None,
                //     None,
                // );
            }
            let ent_uid = (
                StringId::from("singleton").unwrap(),
                StringId::from("0").unwrap(),
            );
            let comp_uid = (
                StringId::from("mod_init").unwrap(),
                StringId::from("0").unwrap(),
            );
            let mut comp_model = model
                .get_component_mut(&StringId::from("mod_init").unwrap())
                .unwrap();
            let commands = comp_model.logic.commands.clone();
            comp_model
                .logic
                .states
                .insert(ShortString::from("init").unwrap(), (0, commands.len()));
        }

        Ok(model)
    }
}

impl SimModel {
    /// Get reference to entity prefab using `type_` and `id` str args.
    pub fn get_entity(&self, name: &StringId) -> Option<&EntityPrefabModel> {
        self.entities
            .iter()
            .find(|entity| &entity.name.as_ref() == &name.as_str())
    }
    /// Get mutable reference to entity prefab using `type_` and `id` args.
    pub fn get_entity_mut(&mut self, name: &StringId) -> Option<&mut EntityPrefabModel> {
        self.entities.iter_mut().find(|entity| &entity.name == name)
    }
    /// Get reference to component model using `type_` and `id` args.
    pub fn get_component(&self, name: &StringId) -> Option<&ComponentModel> {
        self.components.iter().find(|comp| &comp.name == name)
    }
    /// Get mutable reference to component model using `type_` and `id` args.
    pub fn get_component_mut(&mut self, name: &StringId) -> Option<&mut ComponentModel> {
        self.components.iter_mut().find(|comp| &comp.name == name)
    }
}

/// Scenario manifest model. Slightly different from the raw static deser form.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioManifest {
    // required
    pub name: String,
    pub version: String,
    pub engine: String,
    // optional
    pub mods: Vec<ScenarioModuleDep>,
    pub settings: HashMap<String, Value>,
    pub title: Option<String>,
    pub desc: Option<String>,
    pub desc_long: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
}
impl ScenarioManifest {
    /// Create new scenario manifest object from path
    /// reference to scenario directory
    pub fn from_dir_at(path: PathBuf) -> Result<ScenarioManifest> {
        let manifest_path = path.join(SCENARIO_MANIFEST_FILE);
        let deser_manifest: deser::ScenarioManifest =
            util::static_deser_obj_from_path(manifest_path)?;
        // let deser_manifest: deser::ScenarioManifest =
        //     match util::static_deser_obj_from_path(manifest_path) {
        //         // TODO print error?
        //         Err(e) => {
        //             error!("Failed deserializing scenario manifest");
        //             return None;
        //         }
        //         Ok(man) => match man {
        //             Some(m) => m,
        //             None => return None,
        //         },
        //     };
        let mut mods = Vec::new();
        for module in deser_manifest.mods {
            let (name, value) = module;

            // TODO
            mods.push(ScenarioModuleDep::from_toml_value(&name, &value).unwrap());
        }

        Ok(ScenarioManifest {
            // required
            name: deser_manifest.scenario.name,
            version: deser_manifest.scenario.version,
            engine: deser_manifest.scenario.engine,
            // optional
            settings: deser_manifest.settings,
            title: match deser_manifest.scenario.title.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc: match deser_manifest.scenario.desc.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc_long: match deser_manifest.scenario.desc_long.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            author: match deser_manifest.scenario.author.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            website: match deser_manifest.scenario.website.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            mods,
        })
    }
}
/// Scenario module dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioModuleDep {
    pub name: String,
    pub version_req: String,
    pub git_address: Option<String>,
}
impl ScenarioModuleDep {
    /// Create scenario module dependency object from a
    /// serde value representation.
    pub fn from_toml_value(scenario_name: &String, value: &Value) -> Option<ScenarioModuleDep> {
        // str field names
        let version_field = "version";
        let git_field = "git";

        let mut version_req = String::from("*");
        let mut git_address = None;

        // simplest is a str version
        if let Some(s) = value.as_str() {
            match VersionReq::parse(s) {
                Ok(vr) => version_req = vr.to_string(),
                Err(e) => {
                    warn!(
                        "failed parsing scenario module dep version req \"{}\" ({}), \
                         using default \"*\" (any)",
                        s, e
                    );
                    version_req = VersionReq::any().to_string();
                }
            }
        }
        // otherwise it's a mapping with different kinds of entries
        else if let Some(mapping) = value.as_table() {
            unimplemented!();
            if let Ok(vr) = VersionReq::parse(value.as_str().unwrap()) {
                version_req = vr.to_string();
            } else {
                // TODO print warning about the version_req
            }
            // `git_address` is optional, default is `None`
            git_address = match value.get(git_field) {
                Some(v) => Some(String::from(v.as_str().unwrap())),
                None => None,
            };
        } else {
            error!(
                "module dep has to be either a string (version specifier)\
                 or a mapping"
            );
            return None;
        }
        Some(ScenarioModuleDep {
            name: scenario_name.clone(),
            version_req,
            git_address,
        })
    }
}

/// Scenario model, consisting of the manifest and list of modules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scenario {
    /// Full path to scenario root directory
    pub path: PathBuf,
    pub manifest: ScenarioManifest,
    pub modules: Vec<Module>,
}
impl Scenario {
    /// Create a scenario model from a path reference to scenario directory.
    pub fn from_dir_at(path: PathBuf) -> Result<Scenario> {
        // get the scenario manifest
        // let scenario_manifest = ScenarioManifest::from_dir_at(path.clone()).expect(&format!(
        //     "failed making scenario manifest from dir path: \"{}\"",
        //     path.to_str().unwrap()
        // ));
        let scenario_manifest = ScenarioManifest::from_dir_at(path.clone())?;

        // if the version requirement for the engine specified in
        // the scenario manifest is not met throw a warning
        if !VersionReq::from_str(&scenario_manifest.engine)?.matches(&Version::from_str(VERSION)?) {
            error!("`outcome` version used by this program does not meet the version requirement \
            specified in scenario manifest (\"engine\" entry), \
            this is unacceptable (version of `outcome` used is: \"{}\", version requirement: \"{}\")",
                  VERSION, &scenario_manifest.engine);
            return Err(Error::Other("".to_string()));
        }
        // get the map of mods to load from the manifest (only mods
        // listed there will be loaded)
        let mods_to_load = scenario_manifest.mods.clone();
        info!(
            "there are {} mods listed in the scenario manifest",
            &mods_to_load.len()
        );
        // get the path to scenario mods directory
        let scenario_mods_path = path.join(SCENARIO_MODS_DIR_NAME);
        // found matching mods will be added to this vec
        let mut matching_mods: Vec<Module> = Vec::new();
        // this vec is for storing mod_not_found messages to print
        // them after the loop
        let mut mod_not_found_msgs: Vec<String> = Vec::new();
        // this bool will turn false if any of the mods from the
        // manifest wasn't found based on it the process of
        // creating the scenario can be halted
        let mut all_mods_found = true;
        // try to find all the mods specified in the scenario
        // manifest
        for mod_to_load in mods_to_load.to_owned() {
            let mod_to_load_name = mod_to_load.name;
            let mod_version_req = mod_to_load.version_req;
            let mut found_mod_match = false;
            // only the top directories within the mods directory are
            // considered
            for mod_dir in util::get_top_dirs_at(scenario_mods_path.clone()) {
                let mod_dir_name = mod_dir.file_name().unwrap().to_str().unwrap();
                // we only want matching dir names
                if mod_dir_name != mod_to_load_name {
                    continue;
                };
                // path of the mod manifest we need to look for
                let mod_manifest_path = mod_dir.join(MODULE_MANIFEST_FILE);
                if mod_manifest_path.exists() {
                    let module_manifest: ModuleManifest =
                        ModuleManifest::from_dir_at(mod_dir.clone())?;
                    // is the engine version requirement met?
                    if !VersionReq::parse(&module_manifest.engine_version_req)?
                        .matches(&Version::parse(VERSION)?)
                    {
                        return Err(Error::Other(format!("mod \"{}\" specifies a version requirement for `outcome` (\"engine\" entry) that does not match \
                        the version this program is using (version of `outcome` used: \"{}\", version requirement: \"{}\")",
                               module_manifest.name, VERSION, module_manifest.engine_version_req)));
                    }
                    // are the engine feature requirements met?
                    for feature_req in &module_manifest.engine_features {
                        match feature_req.as_str() {
                            crate::FEATURE_NAME_MACHINE_SYSINFO => {
                                if !crate::FEATURE_MACHINE_SYSINFO {
                                    return Err(Error::Other(format!(
                                        "required feature \"system_info\" not available"
                                    )));
                                }
                            }
                            _ => (),
                        }
                    }

                    // found mod that matches the name and version from scenario manifest
                    if module_manifest.name == mod_to_load_name
                        && VersionReq::parse(&mod_version_req)
                            .unwrap_or(VersionReq::any())
                            .matches(
                                &Version::parse(&module_manifest.version)
                                    .unwrap_or(Version::new(0, 1, 0)),
                            )
                    {
                        info!(
                            "mod found: \"{}\" version: \"{}\" (\"{}\")",
                            mod_to_load_name,
                            module_manifest.version.to_string(),
                            mod_version_req.to_string()
                        );
                        let module = match Module::from_dir_at(mod_dir.clone()) {
                            Ok(m) => m,
                            Err(_) => {
                                error!("failed creating module from path: {:?}", mod_dir.clone());
                                continue;
                            }
                        };
                        matching_mods.push(module);
                        found_mod_match = true;
                        break;
                    }
                }
            }
            // if no matching mod was found
            if !found_mod_match {
                all_mods_found = false;
                mod_not_found_msgs.push(format!(
                    "mod not found: name:\"{}\" version:\"{}\" specified in scenario manifest was not \
                        found",
                    mod_to_load_name, mod_version_req.to_string()));
            }
        }

        // check if mod dependencies are present
        if matching_mods.len() > 0 {
            for n in 0..matching_mods.len() - 1 {
                let module = &matching_mods[n].clone();
                let mut missing_deps: Vec<ModuleDep> = Vec::new();
                for (dep_name, dep) in &module.manifest.dependencies {
                    // is the dependency mod present?
                    if matching_mods.iter().any(|m| {
                        &m.manifest.name == dep_name
                            && VersionReq::parse(&dep.version_req)
                                .unwrap_or(VersionReq::any())
                                .matches(
                                    &Version::parse(&m.manifest.version)
                                        .unwrap_or(Version::new(0, 1, 0)),
                                )
                    }) {
                        // we're fine
                    } else {
                        // dependency not present, throw an error
                        error!(
                            "dependency not available: \"{}\" (\"{}\"), \
                             required by \"{}\" (\"{}\")",
                            dep_name.clone(),
                            dep.version_req.to_string(),
                            module.manifest.name,
                            module.manifest.version.to_string()
                        );
                        missing_deps.push(dep.clone());
                        all_mods_found = false;
                    }
                }
                if !missing_deps.is_empty() {
                    matching_mods.remove(n);
                }
            }
        }

        // show errors about mods not found, they are shown after
        // the mod found messages
        for err_msg in mod_not_found_msgs {
            error!("{}", err_msg);
        }

        // break if not all the mods were found
        if !all_mods_found {
            error!(
                "failed to load all mods listed in the scenario manifest ({}/{})",
                matching_mods.len(),
                mods_to_load.len()
            );
            // error!("scenario creation process halted: missing modules");
            return Err(Error::ScenarioMissingModules);
        } else {
            info!(
                "found all mods listed in the scenario manifest ({})",
                mods_to_load.len()
            );
        }

        Ok(Scenario {
            path,
            manifest: scenario_manifest,
            modules: matching_mods,
        })
    }
}

/// Module manifest model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    // required
    pub name: String,
    pub version: String,
    pub engine_version_req: String,
    pub engine_features: Vec<String>,
    pub dependencies: HashMap<String, ModuleDep>,
    pub reqs: Vec<String>,
    // optional
    pub title: Option<String>,
    pub desc: Option<String>,
    pub desc_long: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
}
impl ModuleManifest {
    /// Create module manifest from path to module directory
    pub fn from_dir_at(path: PathBuf) -> Result<ModuleManifest> {
        let manifest_path = path.join(MODULE_MANIFEST_FILE);
        let deser_manifest: deser::ModuleManifest =
            util::static_deser_obj_from_path(manifest_path.clone())?;
        let mut dep_map: HashMap<String, ModuleDep> = HashMap::new();
        for (name, value) in deser_manifest.dependencies {
            // TODO
            // dep_map.insert(name.clone(),
            // ModuleDep::from_toml_value(&name,
            // &value));
        }
        let mut req_vec: Vec<String> = Vec::new();
        for req in deser_manifest.reqs {
            req_vec.push(req);
        }
        let mut engine_version_req = String::new();
        let mut engine_features = Vec::new();
        if let Some(table) = deser_manifest.mod_.engine.as_table() {
            for (name, value) in table {
                match name.as_str() {
                    "version" => engine_version_req = value.as_str().unwrap().to_string(),
                    "features" => {
                        engine_features = value
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect()
                    }
                    _ => (),
                }
            }
        }

        Ok(ModuleManifest {
            name: deser_manifest.mod_.name,
            engine_version_req,
            engine_features,
            version: deser_manifest.mod_.version,
            dependencies: dep_map,
            reqs: req_vec,
            title: match deser_manifest.mod_.title.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc: match deser_manifest.mod_.desc.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc_long: match deser_manifest.mod_.desc_long.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            author: match deser_manifest.mod_.author.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            website: match deser_manifest.mod_.website.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
        })
    }
}

/// Module dependency on another module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDep {
    pub name: String,
    pub version_req: String,
    pub git_address: Option<String>,
}

/// Module model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub manifest: ModuleManifest,
}
impl Module {
    pub fn from_dir_at(path: PathBuf) -> Result<Module> {
        let module_manifest = ModuleManifest::from_dir_at(path.clone())?;

        Ok(Module {
            manifest: module_manifest,
        })
    }
}

/// Trigger event model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModel {
    pub id: ShortString,
}

/// Entity prefab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPrefabModel {
    pub name: ShortString,
    pub components: Vec<ShortString>,
}

// /// Component prefab.
// cfg_if! {
//     if #[cfg(feature = "machine")] {
//         #[derive(Debug, Clone, Serialize, Deserialize)]
//         pub struct ComponentPrefab {
//             pub name: ShortString,
//             pub vars: Vec<VarModel>,
//             pub start_state: ShortString,
//             pub triggers: Vec<ShortString>,
//
//             pub logic: LogicModel,
//
//             pub source_files: Vec<PathBuf>,
//             pub script_files: Vec<PathBuf>,
//             pub lib_files: Vec<PathBuf>,
//         }
//     } else {
//         #[derive(Debug, Clone, Serialize, Deserialize)]
//         pub struct ComponentPrefab {
//             pub name: ShortString,
//             pub vars: Vec<VarModel>,
//         }
//     }
// }
//
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentModel {
    pub name: ShortString,
    pub vars: Vec<VarModel>,
    pub start_state: ShortString,
    pub triggers: Vec<ShortString>,

    #[cfg(feature = "machine")]
    pub logic: LogicModel,

    pub source_files: Vec<PathBuf>,
    pub script_files: Vec<PathBuf>,
    pub lib_files: Vec<PathBuf>,
}

#[cfg(feature = "machine")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicModel {
    /// List of loc phase commands
    pub commands: Vec<crate::machine::cmd::Command>,
    /// List of pre phase commands
    pub pre_commands: FnvHashMap<ShortString, Vec<crate::machine::cmd::ExtCommand>>,
    /// Mapping of state procedure names to their start and end lines
    pub states: FnvHashMap<ShortString, (usize, usize)>,
    /// Mapping of non-state procedure names to their start and end lines
    pub procedures: FnvHashMap<ShortString, (usize, usize)>,
    /// Location info mapped for each command on the list by vec index
    pub cmd_location_map: FnvHashMap<usize, crate::machine::LocationInfo>,
}
#[cfg(feature = "machine")]
impl LogicModel {
    pub fn empty() -> LogicModel {
        LogicModel {
            commands: Vec::new(),
            states: FnvHashMap::default(),
            procedures: FnvHashMap::default(),
            cmd_location_map: FnvHashMap::default(),
            pre_commands: FnvHashMap::default(),
        }
    }
}

/// Variable model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarModel {
    pub id: String,
    pub type_: VarType,
    pub default: Var,
    pub internal: bool,
}

/// Data entry model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataEntry {
    Simple((String, String)),
    List((String, Vec<String>)),
    Grid((String, Vec<Vec<String>>)),
}

/// Data file entry model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFileEntry {
    Json(String),
    JsonList(String),
    JsonGrid(String),
    Yaml(String),
    YamlList(String),
    YamlGrid(String),
    CsvList(String),
    CsvGrid(String),
}

/// Data image entry model. Used specifically for importing
/// grid data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataImageEntry {
    BmpU8(String, String),
    BmpU8U8U8(String, String),
    //    BmpCombineU8U8U8U8Int(String, String),
    // TODO
    PngU8(String, String),
    PngU8U8U8(String, String),
    PngU8U8U8Concat(String, String),
    //    PngCombineU8U8U8U8(String, String),
}
