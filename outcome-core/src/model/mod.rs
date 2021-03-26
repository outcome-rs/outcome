//! Model content definitions, logic for turning deserialized data into
//! model objects.

#![allow(unused)]

mod deser;

use std::collections::HashMap;
use std::fs::{read, read_dir, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use fnv::FnvHashMap;
use semver::{Version, VersionReq};
use toml::Value;

use crate::address::{Address, LocalAddress, ShortLocalAddress};
use crate::error::Error;
use crate::util;
use crate::{arraystring, MedString, ShortString, StringId};
use crate::{CompName, EntityName, EventName, Result, Var, VarName, VarType};
use crate::{
    MODULES_DIR_NAME, MODULE_ENTRY_FILE_NAME, MODULE_MANIFEST_FILE, SCENARIOS_DIR_NAME,
    SCENARIO_MANIFEST_FILE, VERSION,
};

#[cfg(feature = "machine_script")]
use crate::machine::script::{parser, preprocessor, InstructionType};
use crate::machine::START_STATE_NAME;

/// Collection of all the model data needed for running a simulation.
///
/// # Instantiating simulation from model
///
/// Creating a simulation instance requires passing an existing `SimModel`.
///
/// # Dynamic model
///
/// As `SimModel` is stored within, and used for runtime processing of, the
/// simulation instance, it itself can also be mutated at runtime. This allows
/// for dynamic changes to the underlying simulation rules at any point during
/// simulation processing.
///
/// # Role of the model in a distributed setting
///
/// In a situation where there are multiple nodes, each holding and processing
/// locally stored entities, the model serves as the collection of common rules
/// shared by the whole system. As such it needs to always stay synchronized
/// across the whole system.
///
/// In a distributed setting, any changes to the model are handled centrally
/// and propagated to all the nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimModel {
    pub scenario: Scenario,
    pub events: Vec<EventModel>,
    pub scripts: Vec<String>,
    pub entities: Vec<EntityPrefab>,
    pub components: Vec<ComponentModel>,
    pub data: Vec<DataEntry>,
    pub data_files: Vec<DataFileEntry>,
    pub data_imgs: Vec<DataImageEntry>,
    pub services: Vec<ServiceModel>,
}

impl SimModel {
    /// Creates a new simulation model from a scenario structure.
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
            services: Vec::new(),
        };

        // add hardcoded content
        #[cfg(feature = "machine")]
        model.events.push(crate::model::EventModel {
            id: ShortString::from(crate::DEFAULT_TRIGGER_EVENT).unwrap(),
        });

        let mut mod_init_prefab = EntityPrefab {
            name: StringId::from("_mod_init").unwrap(),
            // name: StringId::from(&format!("_mod_init_{}", module.manifest.name)).unwrap(),
            ..EntityPrefab::default()
        };

        // iterate over scenario modules
        for module in &scenario.modules {
            // services
            for module_service in &module.manifest.services {
                model.services.push(module_service.clone());
            }

            // load from structured data
            #[cfg(feature = "yaml")]
            {
                let files = util::find_files_with_extension(
                    module.path.clone(),
                    vec!["yaml", "yml"],
                    true,
                    None,
                );
                debug!("yaml files: {:?}", files);
                for file in files {
                    if let Ok(file_struct) = util::deser_struct_from_path(file.clone()) {
                        trace!("yaml file struct: {:?}", file_struct);
                        model.apply_from_structured_file(file_struct)?;
                    } else {
                        warn!("unable to parse file: {}", file.to_string_lossy());
                    }
                }
            }

            // load from scripts
            #[cfg(feature = "machine_script")]
            {
                model.events.push(EventModel {
                    id: ShortString::from("_scr_init").unwrap(),
                });

                let scr_init_mod_template = ComponentModel {
                    name: StringId::from("_init_mod_").unwrap(),
                    triggers: vec![StringId::from("_scr_init").unwrap()],
                    logic: LogicModel {
                        start_state: StringId::from("main").unwrap(),
                        ..Default::default()
                    },
                    ..ComponentModel::default()
                };

                // let scr_init_mod_template = ComponentModel {
                //     name: StringId::from_unchecked("_init_mod_"),
                //     vars: vec![],
                //     start_state: StringId::from_unchecked("main"),
                //     triggers: vec![StringId::from_unchecked("_scr_init")],
                //     logic: LogicModel {
                //         commands: Vec::new(),
                //         states: FnvHashMap::default(),
                //         procedures: FnvHashMap::default(),
                //         cmd_location_map: FnvHashMap::default(),
                //         pre_commands: FnvHashMap::default(),
                //     },
                //     source_files: Vec::new(),
                //     script_files: Vec::new(),
                //     lib_files: Vec::new(),
                // };
                // #[cfg(feature = "machine")]
                use crate::machine::{cmd::Command, CommandPrototype, LocationInfo};

                // use script processor to handle scripts
                let program_data = crate::machine::script::util::get_program_metadata();

                // create path to entry script
                let mod_entry_file_path = PathBuf::new()
                    .join(crate::MODULES_DIR_NAME)
                    .join(&module.manifest.name)
                    .join(format!(
                        "{}{}",
                        crate::MODULE_ENTRY_FILE_NAME,
                        crate::machine::script::SCRIPT_FILE_EXTENSION
                    ));

                // parse the module entry script
                let mut instructions = parser::parse_script_at(
                    &mod_entry_file_path.to_string_lossy(),
                    &scenario.path.to_string_lossy(),
                )?;

                // preprocess entry script
                preprocessor::run(&mut instructions, &mut model, &program_data)?;

                // turn instructions into proper commands
                let mut commands: Vec<Command> = Vec::new();
                let mut cmd_prototypes: Vec<CommandPrototype> = Vec::new();
                let mut cmd_locations: Vec<LocationInfo> = Vec::new();
                // first get a list of commands from the main instruction list
                for instruction in instructions {
                    let cmd_prototype = match instruction.type_ {
                        InstructionType::Command(c) => c,
                        _ => continue,
                    };
                    cmd_prototypes.push(cmd_prototype);
                    cmd_locations.push(instruction.location.clone());
                }

                let mut comp_model = scr_init_mod_template.clone();
                comp_model.name =
                    arraystring::new_truncate(&format!("init_{}", module.manifest.name));

                for (n, cmd_prototype) in cmd_prototypes.iter().enumerate() {
                    cmd_locations[n].comp_name = Some(comp_model.name.into());
                    cmd_locations[n].line = Some(n);

                    // create command struct from prototype
                    let command =
                        Command::from_prototype(cmd_prototype, &cmd_locations[n], &cmd_prototypes)?;
                    commands.push(command.clone());

                    // insert the commands into the component's logic model
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

                comp_model
                    .logic
                    .states
                    .insert(StringId::from("main").unwrap(), (0, commands.len()));
                mod_init_prefab.components.push(comp_model.name);
                model.components.push(comp_model);
            }
        }
        model.entities.push(mod_init_prefab);

        Ok(model)
    }
}

impl SimModel {
    pub fn apply_from_structured_file(&mut self, file_struct: deser::DataFile) -> Result<()> {
        for component in file_struct.components {
            trace!("file struct component: {:?}", component);
            if let Some(comp_struct) = component.1 {
                let comp_model = ComponentModel::from_deser(&component.0, comp_struct)?;
                self.components.push(comp_model);
            }
        }

        Ok(())
    }

    /// Get reference to entity prefab using `type_` and `id` str args.
    pub fn get_entity(&self, name: &StringId) -> Option<&EntityPrefab> {
        self.entities
            .iter()
            .find(|entity| &entity.name.as_ref() == &name.as_ref())
    }

    /// Get mutable reference to entity prefab using `type_` and `id` args.
    pub fn get_entity_mut(&mut self, name: &StringId) -> Option<&mut EntityPrefab> {
        self.entities.iter_mut().find(|entity| &entity.name == name)
    }

    /// Get reference to component model using `type_` and `id` args.
    pub fn get_component(&self, name: &CompName) -> Result<&ComponentModel> {
        self.components
            .iter()
            .find(|comp| &comp.name == name)
            .ok_or(Error::NoComponentModel(*name))
    }

    /// Get mutable reference to component model using `type_` and `id` args.
    pub fn get_component_mut(&mut self, name: &StringId) -> Option<&mut ComponentModel> {
        self.components.iter_mut().find(|comp| &comp.name == name)
    }
}

/// Scenario manifest model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioManifest {
    /// Name of, and unique reference to, the scenario
    pub name: String,
    /// Semver specifier for the scenario version
    pub version: String,
    /// Semver specifier for the engine version
    pub engine: String,

    /// List of the module dependencies for the scenario
    pub mods: Vec<ScenarioModuleDep>,
    /// Map of settings, each being essentially an arbitrary data setter
    pub settings: HashMap<String, String>,

    /// More free-form than the name
    pub title: Option<String>,
    /// Short description of the scenario
    pub desc: Option<String>,
    /// Long description of the scenario
    pub desc_long: Option<String>,
    /// Author information
    pub author: Option<String>,
    /// Source website information
    pub website: Option<String>,
}

impl ScenarioManifest {
    /// Creates new scenario manifest object from path reference.
    pub fn from_path(path: PathBuf) -> Result<ScenarioManifest> {
        // let manifest_path = path.join(SCENARIO_MANIFEST_FILE);
        let manifest_path = path;
        let deser_manifest: deser::ScenarioManifest = util::deser_struct_from_path(manifest_path)?;
        let mut mods = Vec::new();
        for module in deser_manifest.mods {
            let (name, value) = module;

            // TODO better errors
            mods.push(ScenarioModuleDep::from_toml_value(&name, &value).unwrap());
        }

        Ok(ScenarioManifest {
            name: deser_manifest.scenario.name,
            version: deser_manifest.scenario.version,
            engine: deser_manifest.scenario.engine,

            settings: deser_manifest
                .settings
                .iter()
                .map(|(s, v)| (s.to_string(), v.to_string()))
                .collect(),
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
    /// Create scenario module dependency object from a serde value
    /// representation.
    pub fn from_toml_value(scenario_name: &String, value: &Value) -> Option<ScenarioModuleDep> {
        // field names
        let version_field = "version";
        let git_field = "git";

        let mut version_req = "*".to_string();
        let mut git_address = None;

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

/// Scenario model consisting of the manifest and list of modules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scenario {
    /// Full path to scenario root directory
    pub path: PathBuf,
    /// Scenario manifest
    pub manifest: ScenarioManifest,
    /// List of modules
    pub modules: Vec<Module>,
}

impl Scenario {
    /// Create a scenario model from a path reference to scenario manifest.
    pub fn from_path(path: PathBuf) -> Result<Scenario> {
        // resolve project root path
        let mut dir_path = path.parent().ok_or(Error::Other(format!(
            "unable to get parent of path: {}",
            path.to_string_lossy()
        )))?;
        let stem = dir_path.file_stem().ok_or(Error::Other(format!(
            "unable to get stem of path: {}",
            path.to_string_lossy()
        )))?;
        if dir_path.is_dir() && stem == SCENARIOS_DIR_NAME {
            dir_path = dir_path.parent().ok_or(Error::Other(format!(
                "unable to get parent of path: {}",
                path.to_string_lossy()
            )))?;
        } else {
            warn!(
                "scenarios are expected to be kept inside a dedicated \"{}\" directory",
                SCENARIOS_DIR_NAME
            )
        }
        info!("project root directory: {:?}", dir_path);

        // get the scenario manifest
        let scenario_manifest = ScenarioManifest::from_path(path.clone())?;

        // if the version requirement for the engine specified in
        // the scenario manifest is not met return an error
        if !VersionReq::from_str(&scenario_manifest.engine)?.matches(&Version::from_str(VERSION)?) {
            error!(
                "engine version does not meet the requirement specified in scenario manifest, \
                current engine version: \"{}\", version requirement: \"{}\"",
                VERSION, &scenario_manifest.engine
            );
            return Err(Error::Other(
                "engine version does not match module requirement".to_string(),
            ));
        }
        // get the map of mods to load from the manifest (only mods
        // listed there will be loaded)
        let mods_to_load = &scenario_manifest.mods;
        info!(
            "there are {} mods listed in the scenario manifest",
            &mods_to_load.len()
        );
        // get the path to scenario mods directory
        let scenario_mods_path = dir_path.join(MODULES_DIR_NAME);
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
        for mod_to_load in mods_to_load {
            let mod_to_load_name = mod_to_load.name.clone();
            let mod_version_req = mod_to_load.version_req.clone();
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
            path: dir_path.to_path_buf(),
            manifest: scenario_manifest,
            modules: matching_mods,
        })
    }
}

/// Module manifest model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    // required
    /// Module name
    pub name: String,
    /// Module version
    pub version: String,
    /// Required engine version
    pub engine_version_req: String,
    /// Required engine features
    pub engine_features: Vec<String>,
    /// List of other module dependencies for this module
    pub dependencies: HashMap<String, ModuleDep>,
    /// List of required target addrs
    pub reqs: Vec<String>,

    pub libs: Vec<ModuleLib>,
    pub services: Vec<ServiceModel>,

    // optional
    /// Free-form module name
    pub title: Option<String>,
    /// Module description
    pub desc: Option<String>,
    /// Longer module description
    pub desc_long: Option<String>,
    /// Author information
    pub author: Option<String>,
    /// Website information
    pub website: Option<String>,
}

impl ModuleManifest {
    /// Create module manifest from path to module directory
    pub fn from_dir_at(path: PathBuf) -> Result<ModuleManifest> {
        let manifest_path = path.join(MODULE_MANIFEST_FILE);
        let deser_manifest: deser::ModuleManifest =
            util::deser_struct_from_path(manifest_path.clone())?;
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
        if let Some(table) = deser_manifest._mod.engine.as_table() {
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
        let mut libs = Vec::new();
        for (lib_name, lib_value) in deser_manifest.libs {
            let mut library_path = None;
            let mut project_path = None;
            if let Some(table) = lib_value.as_table() {
                for (name, value) in table {
                    match name.as_str() {
                        "path" | "library" => library_path = Some(value.to_string()),
                        "project" => project_path = Some(value.to_string()),
                        _ => (),
                    }
                }
            } else if let Some(s) = lib_value.as_str() {
                library_path = Some(s.to_string());
            }
            let lib = ModuleLib {
                path: library_path,
                project: project_path,
            };
            libs.push(lib);
        }

        let mut services = Vec::new();
        for (service_name, service_value) in deser_manifest.services {
            let mut executable_path = None;
            let mut project_path = None;
            let mut managed = true;
            let mut args = Vec::new();

            if let Some(table) = service_value.as_table() {
                for (name, value) in table {
                    match name.as_str() {
                        "executable" | "path" => {
                            executable_path =
                                Some(value.to_string()[1..value.to_string().len() - 1].to_string())
                        }
                        "project" => project_path = Some(value.to_string()),
                        "args" => {
                            if let Some(arr) = value.as_array() {
                                args = arr.iter().map(|v| v.to_string()).collect();
                            }
                        }
                        _ => (),
                    }
                }
            } else if let Some(s) = service_value.as_str() {
                executable_path = Some(s[1..s.len() - 1].to_string());
            }

            let service = ServiceModel {
                name: service_name,
                executable: Some(
                    path.join(PathBuf::from_str(executable_path.unwrap().as_str()).unwrap()),
                ),
                project: project_path,
                managed,
                args,
            };
            services.push(service);
        }

        Ok(ModuleManifest {
            name: deser_manifest._mod.name,
            engine_version_req,
            engine_features,
            version: deser_manifest._mod.version,
            dependencies: dep_map,
            reqs: req_vec,
            libs,
            services,
            title: match deser_manifest._mod.title.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc: match deser_manifest._mod.desc.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            desc_long: match deser_manifest._mod.desc_long.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            author: match deser_manifest._mod.author.as_str() {
                "" => None,
                s => Some(s.to_owned()),
            },
            website: match deser_manifest._mod.website.as_str() {
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

/// Library declared by a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLib {
    /// Path to dynamic library file relative to module root
    path: Option<String>,
    /// Path to buildable project
    project: Option<String>,
}

/// Service declared by a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceModel {
    /// Unique name for the service
    pub name: String,
    /// Path to executable relative to module root
    pub executable: Option<PathBuf>,
    /// Path to buildable project
    pub project: Option<String>,
    /// Defines the nature of the service
    pub managed: bool,
    /// Arguments string passed to the executable
    pub args: Vec<String>,
}

/// Module model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub manifest: ModuleManifest,
    pub path: PathBuf,
}

impl Module {
    pub fn from_dir_at(path: PathBuf) -> Result<Module> {
        let module_manifest = ModuleManifest::from_dir_at(path.clone())?;

        Ok(Module {
            manifest: module_manifest,
            path,
        })
    }
}

/// Trigger event model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModel {
    pub id: EventName,
}

/// Entity prefab model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityPrefab {
    pub name: EntityName,
    pub components: Vec<CompName>,
}

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
/// Component model.
///
/// Components are primarily referenced by their name. Other than that
/// each component defines a list of variables and a list of event triggers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentModel {
    /// String identifier of the component
    pub name: CompName,
    /// List of variables that define the component's interface
    pub vars: Vec<VarModel>,
    /// List of events that serve as triggers for the component
    pub triggers: Vec<StringId>,

    /// Logic attached to the component
    #[cfg(feature = "machine")]
    pub logic: LogicModel,
}

impl ComponentModel {
    pub fn from_deser(key: &String, val: deser::ComponentEntry) -> Result<Self> {
        Ok(ComponentModel {
            name: arraystring::new_truncate(key),
            vars: val
                .vars
                .into_iter()
                .filter(|(k, v)| v.is_some())
                .map(|(k, v)| VarModel::from_deser(&k, v).unwrap())
                .collect(),
            triggers: Vec::new(),
            logic: LogicModel {
                start_state: arraystring::new_unchecked(START_STATE_NAME),
                ..Default::default()
            },
        })
    }
}

/// Component-bound state machine logic model.
#[cfg(feature = "machine")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogicModel {
    /// Name of the starting state
    pub start_state: StringId,
    /// List of local phase commands
    pub commands: Vec<crate::machine::cmd::Command>,
    /// List of pre phase commands
    pub pre_commands: FnvHashMap<ShortString, Vec<crate::machine::cmd::ExtCommand>>,
    /// Mapping of state procedure names to their start and end lines
    pub states: FnvHashMap<StringId, (usize, usize)>,
    /// Mapping of non-state procedure names to their start and end lines
    pub procedures: FnvHashMap<ShortString, (usize, usize)>,
    /// Location info mapped for each command on the list by index
    pub cmd_location_map: Vec<crate::machine::LocationInfo>,
}

#[cfg(feature = "machine")]
impl LogicModel {
    pub fn empty() -> LogicModel {
        LogicModel {
            start_state: arraystring::new_unchecked(crate::machine::START_STATE_NAME),
            commands: Vec::new(),
            states: FnvHashMap::default(),
            procedures: FnvHashMap::default(),
            cmd_location_map: Vec::new(),
            pre_commands: FnvHashMap::default(),
        }
    }

    pub fn get_subset(&self, start_line: usize, last_line: usize) -> LogicModel {
        let mut new_logic = LogicModel::empty();
        new_logic.commands = self.commands[start_line..last_line].to_vec();
        new_logic.cmd_location_map = self.cmd_location_map[start_line..last_line].to_vec();
        // warn!("{:?}", new_logic);
        new_logic
    }
}

/// Variable model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarModel {
    pub id: VarName,
    pub type_: VarType,
    pub default: Option<Var>,
}

impl VarModel {
    pub fn from_deser(key: &str, val: Option<deser::VarEntry>) -> Result<VarModel> {
        let addr = ShortLocalAddress::from_str(key)?;

        Ok(VarModel {
            id: arraystring::new_truncate(&addr.var_id),
            type_: addr.var_type,
            default: val.map(|v| Var::from(v)),
        })
    }
}

/// Data entry model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataEntry {
    Simple((String, String)),
    List((String, Vec<String>)),
    #[cfg(feature = "grids")]
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

/// Data image entry model. Used specifically for importing grid data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataImageEntry {
    BmpU8(String, String),
    BmpU8U8U8(String, String),
    // BmpCombineU8U8U8U8Int(String, String),
    // TODO
    PngU8(String, String),
    PngU8U8U8(String, String),
    PngU8U8U8Concat(String, String),
    // PngCombineU8U8U8U8(String, String),
}
