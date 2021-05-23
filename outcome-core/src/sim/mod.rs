//! Local simulation abstraction.

pub mod step;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Stdout, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(feature = "load_img")]
use image;
#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

use fnv::FnvHashMap;
use id_pool::IdPool;

use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::error::Error;
use crate::model::{DataEntry, DataImageEntry, EventModel, Scenario};
use crate::snapshot::{Snap, Snapshot};
use crate::{
    model, string, CompName, EntityId, EntityName, EventName, Result, SimModel, SimStarter,
    StringId, Var, VarType, FEATURE_NAME_SHORT_STRINGID, FEATURE_NAME_STACK_STRINGID,
    FEATURE_SHORT_STRINGID, FEATURE_STACK_STRINGID,
};

/// Local (non-distributed) simulation instance object.
///
/// One of the main abstractions provided by the library. It allows for quick
/// assembly of a full-fledged simulation instance from either a scenario or
/// a snapshot.
///
/// # Local single machine context
///
/// While it's capable of utilizing multiple processor cores, `Sim` is still
/// a construct that only functions within the confines of a single machine.
/// For the distributed variants see [`distr::SimCentral`] and
/// [`distr::SimNode`].
///
/// # Example
///
/// ```ignore
/// use std::env;
/// let path = env::current_dir().unwrap();
/// let sim = outcome_core::Sim::from_scenario_at(path).expect("failed");
/// ```
///
/// [`distr::SimCentral`]: ../distr/central/struct.SimCentral.html
/// [`distr::SimNode`]: ../distr/node/struct.SimNode.html
#[derive(Serialize, Deserialize)]
pub struct Sim {
    /// Serves as the base for creation and runtime processing of the
    /// simulation
    pub model: SimModel,

    /// Number of steps that have been processed so far
    pub(crate) clock: usize,
    /// Global queue of events waiting for execution
    pub event_queue: Vec<EventName>,

    /// All entities that exist within the simulation are stored here
    pub entities: FnvHashMap<EntityId, Entity>,
    /// Map of string indexes for entities (string indexes are optional)
    pub entity_idx: FnvHashMap<EntityName, EntityId>,
    /// Pool of integer identifiers for entities
    pub entity_pool: IdPool,

    /// Lua state for selected entities
    #[cfg(feature = "machine_lua")]
    #[serde(skip)]
    pub entity_lua_state: FnvHashMap<EntityId, Arc<Mutex<Lua>>>,
    /// Loaded dynamic libraries by name
    #[cfg(feature = "machine_dynlib")]
    #[serde(skip)]
    pub libs: BTreeMap<String, libloading::Library>,
}

/// Snapshot functionality.
impl Sim {
    /// Serialize simulation to a vector of bytes.
    ///
    /// # Compression
    ///
    /// Optional compression using LZ4 algorithm can be performed.
    pub fn save_snapshot(&self, name: &str, compress: bool) -> Result<()> {
        let mut data = self.to_snapshot()?;
        // TODO store project path on Sim struct?
        let project_path = crate::util::find_project_root(self.model.scenario.path.clone(), 3)?;
        let snapshot_path = project_path.join(crate::SNAPSHOTS_DIR_NAME).join(name);

        #[cfg(feature = "lz4")]
        {
            if compress {
                data = lz4::block::compress(&data, None, true)?;
            }
        }

        let mut file = File::create(snapshot_path)?;
        file.write_all(&data);

        Ok(())
    }

    /// Creates new `Sim` from snapshot, using
    pub fn load_snapshot(name: &str, compressed: Option<bool>) -> Result<Self> {
        let project_path = crate::util::find_project_root(std::env::current_dir()?, 3)?;
        let snapshot_path = project_path.join(crate::SNAPSHOTS_DIR_NAME).join(name);
        let mut file = File::open(snapshot_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes);
        if let Some(compressed) = compressed {
            if compressed {
                #[cfg(feature = "lz4")]
                {
                    bytes = lz4::block::decompress(&bytes, None)?;
                }
            };
            let sim = Sim::from_snapshot(&mut bytes)?;
            Ok(sim)
        } else {
            // first try reading compressed
            #[cfg(feature = "lz4")]
            {
                // bytes = lz4::block::decompress(&bytes, None)?;
                match lz4::block::decompress(&bytes, None) {
                    Ok(mut bytes) => {
                        let sim = Sim::from_snapshot(&mut bytes)?;
                        return Ok(sim);
                    }
                    Err(_) => {
                        let sim = Sim::from_snapshot(&mut bytes)?;
                        return Ok(sim);
                    }
                }
            }
            #[cfg(not(feature = "lz4"))]
            {
                let sim = Sim::from_snapshot(&mut bytes)?;
                return Ok(sim);
            }
        }
    }

    // /// Create simulation instance from a vector of bytes representing a snapshot.
    // pub fn from_snapshot(mut buf: &Vec<u8>, compressed: bool) -> Result<Self> {
    //     if compressed {
    //         #[cfg(feature = "lz4")]
    //         let data = lz4::block::decompress(&buf, None)?;
    //         #[cfg(feature = "lz4")]
    //         let mut sim = match bincode::deserialize::<Sim>(&data) {
    //             Ok(ms) => ms,
    //             Err(e) => return Err(Error::FailedReadingSnapshot(format!("{}", e))),
    //         };
    //         #[cfg(not(feature = "lz4"))]
    //         let mut sim: Self = match bincode::deserialize(&buf) {
    //             Ok(ms) => ms,
    //             Err(e) => return Err(Error::FailedReadingSnapshot("".to_string())),
    //         };
    //         // TODO handle additional initialization here or create an init function on `Sim`
    //         // sim.setup_lua_state(&model);
    //         // sim.setup_lua_state_ent();
    //         return Ok(sim);
    //     } else {
    //         let sim = Sim::from_snapshot()
    //         // sim.setup_lua_state(&model);
    //         // sim.setup_lua_state_ent();
    //         return Ok(sim);
    //     }
    // }

    /// Create simulation instance using a path to snapshot file.
    pub fn from_snapshot_at(path: &str) -> Result<Self> {
        println!("sim from_snapshot_at: {}", path);
        let pathbuf = PathBuf::from(path);
        let path = pathbuf.canonicalize().unwrap();
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                // error!("{}", e);
                return Err(Error::FailedReadingSnapshot(format!("{}", e)));
            }
        };
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf);

        // first try deserializing as compressed, otherwise it must be uncompressed
        if let Ok(s) = Self::from_snapshot(&mut buf) {
            // if let Ok(s) = Self::from_snapshot(&buf, true) {
            return Ok(s);
        } else {
            return Self::from_snapshot(&mut buf);
            // return Self::from_snapshot(&buf, false);
        }
    }
}

impl Sim {
    /// Gets the sim clock value.
    pub fn get_clock(&self) -> usize {
        self.clock
    }

    /// Creates a new bare-bones simulation instance.
    pub fn new() -> Self {
        Self {
            model: SimModel::default(),
            clock: 0,
            event_queue: Vec::new(),
            entities: FnvHashMap::default(),
            entity_idx: FnvHashMap::default(),
            entity_pool: id_pool::IdPool::new(),
            #[cfg(feature = "machine_lua")]
            entity_lua_state: Default::default(),
            #[cfg(feature = "machine_dynlib")]
            libs: Default::default(),
        }
    }

    pub fn from_project_starter(project_path: PathBuf, starter: SimStarter) -> Result<Self> {
        match starter {
            SimStarter::Scenario(scenario) => {
                Self::from_scenario_at_path(project_path.join(scenario))
            }
            SimStarter::Snapshot(snapshot) => {
                Self::from_snapshot_at(project_path.join(snapshot).to_str().unwrap())
            }
            SimStarter::Experiment(_) => unimplemented!(),
        }
    }

    /// Creates new simulation instance from a path to scenario directory.
    pub fn from_scenario_at_path(path: PathBuf) -> Result<Self> {
        let scenario = Scenario::from_path(path.clone())?;
        Sim::from_scenario(scenario)
    }

    /// Creates new simulation instance from a &str path to scenario directory.
    pub fn from_scenario_at(path: &str) -> Result<Self> {
        let path = PathBuf::from(path).canonicalize()?;
        Sim::from_scenario_at_path(path)
    }

    /// Creates new simulation instance from a scenario struct.
    pub fn from_scenario(scenario: Scenario) -> Result<Self> {
        // first create a model using the given scenario
        let model = SimModel::from_scenario(scenario)?;
        // then create a sim struct using that model
        let mut sim = Sim::from_model(model)?;
        Ok(sim)
    }

    /// Creates a new simulation instance from a model struct.
    pub fn from_model(model: model::SimModel) -> Result<Self> {
        // create a new sim object
        let mut sim: Sim = Sim {
            model,
            clock: 0,
            event_queue: Vec::new(),
            entities: FnvHashMap::default(),
            entity_idx: FnvHashMap::default(),
            entity_pool: id_pool::IdPool::new(),
            #[cfg(feature = "machine_lua")]
            entity_lua_state: Default::default(),
            #[cfg(feature = "machine_dynlib")]
            libs: Default::default(),
        };

        #[cfg(feature = "machine_dynlib")]
        {
            for module in &sim.model.scenario.modules {
                for module_lib in &module.manifest.libraries {
                    // use paths to existing shared library files
                    if let Some(lib_path) = &module_lib.path {
                        let mut full_path = module.path.join(lib_path);
                        // set extension based on detected system
                        if full_path.extension().is_none() {
                            #[cfg(target_os = "windows")]
                            full_path.set_extension("dll");
                            #[cfg(target_os = "linux")]
                            full_path.set_extension("so");
                        }
                        let lib = Library::new(full_path).unwrap();
                        sim.libs.insert(module_lib.name.clone(), lib);
                    }
                    // build rust projects as library using cargo
                    else if let Some(lib_project_path) = &module_lib.project_path {
                        let lib_project_path = PathBuf::from(lib_project_path);
                        let lib_project_path_full = module.path.join(lib_project_path.clone());

                        let mut cmd = std::process::Command::new("cargo");
                        cmd.current_dir(lib_project_path_full.clone()).arg("build");

                        if let Some(mode) = &module_lib.project_mode {
                            if mode.as_str() == "release" {
                                cmd.arg("--release");
                            }
                        } else {
                            cmd.arg("--release");
                        }

                        // pass relevant features to the command
                        let mut features = vec![];

                        // add explicitly selected features
                        if let Some(project_features) = &module_lib.project_features {
                            let features_str = project_features.split(",").collect::<Vec<&str>>();
                            for feature_str in features_str {
                                features.push(feature_str.to_string());
                            }
                        }

                        // inherit features from the current program
                        if module_lib.project_inherit_features {
                            if FEATURE_STACK_STRINGID {
                                features
                                    .push(format!("outcome-core/{}", FEATURE_NAME_STACK_STRINGID));
                            }
                            if FEATURE_SHORT_STRINGID {
                                features
                                    .push(format!("outcome-core/{}", FEATURE_NAME_SHORT_STRINGID));
                            }
                            // TODO add the rest of the features
                        }

                        cmd.arg(format!(
                            "--features={}",
                            features.iter().as_slice().join(",")
                        ));

                        info!(
                            "building library from rust project: {}, mode: {:?} (cmd: {:?})",
                            lib_project_path_full.to_str().unwrap(),
                            module_lib.project_mode,
                            cmd
                        );

                        // execute the command, building the project
                        let status = cmd.status()?;

                        let mut lib_path_full = lib_project_path_full.join(format!(
                            // TODO does DLL output also include 'lib{}' prefix by default?
                            "target/{}/lib{}",
                            module_lib
                                .project_mode
                                .as_ref()
                                .unwrap_or(&"debug".to_string()),
                            lib_project_path.file_name().unwrap().to_str().unwrap()
                        ));
                        // set extension based on detected system
                        if lib_path_full.extension().is_none() {
                            #[cfg(target_os = "windows")]
                            lib_path_full.set_extension("dll");
                            #[cfg(target_os = "linux")]
                            lib_path_full.set_extension("so");
                        }
                        let lib = Library::new(lib_path_full).unwrap();
                        sim.libs.insert(module_lib.name.clone(), lib);
                    }
                }
            }
        }
        // let mut arc_libs = Arc::new(Mutex::new(libs));
        // TODO setup lua state

        // module script init
        #[cfg(feature = "machine_script")]
        {
            sim.spawn_entity(
                Some(&string::new_truncate("_mod_init")),
                Some(string::new_truncate("_mod_init")),
            )?;
            sim.event_queue.push(string::new_truncate("_scr_init"));
        }

        // add entities
        // sim.apply_model_entities();
        // sim.apply_model();

        // setup entities' lua_state
        // sim.setup_lua_state_ent();
        // apply data as found in user files
        sim.apply_data_reg();
        #[cfg(all(feature = "grids", feature = "load_img"))]
        sim.apply_data_img();

        // apply settings from scenario manifest
        sim.apply_settings();

        // apply single step to setup the model
        #[cfg(feature = "machine_script")]
        sim.step();

        Ok(sim)
    }

    /// Applies the model on already existing sim instance, spawning objects
    /// and applying data based on what has been registered.
    pub fn apply_model(&mut self) -> Result<()> {
        unimplemented!()
        // println!("applying model");
        // self.apply_model_entities()
        // self.apply_model_entities_par();
    }

    /// Spawns a new entity based on the given prefab.
    ///
    /// If prefab is `None` then an empty entity is spawned.
    pub fn spawn_entity(
        &mut self,
        prefab: Option<&StringId>,
        name: Option<StringId>,
    ) -> Result<EntityId> {
        trace!("starting spawn_entity");

        trace!("creating new entity");
        // let now = Instant::now();
        let mut ent = match prefab {
            Some(p) => Entity::from_prefab_name(p, &self.model)?,
            None => Entity::empty(),
        };
        // trace!(
        //     "creating ent from prefab took: {}ns",
        //     now.elapsed().as_nanos()
        // );
        trace!("done");

        trace!("getting new_uid from pool");
        let new_uid = self.entity_pool.request_id().unwrap();
        trace!("done");

        trace!("inserting entity");
        if let Some(n) = &name {
            if !self.entity_idx.contains_key(n) {
                self.entity_idx.insert(n.clone(), new_uid);
                self.entities.insert(new_uid, ent);
            } else {
                return Err(Error::Other(format!(
                    "Failed to add entity: entity named \"{}\" already exists",
                    n,
                )));
            }
        } else {
            self.entities.insert(new_uid, ent);
        }
        trace!("done");

        Ok(new_uid)
    }

    pub fn add_event(&mut self, name: EventName) -> Result<()> {
        self.model.events.push(EventModel { id: name.clone() });
        self.event_queue.push(name);
        Ok(())
    }
}

/// Functionality related to handling lua.
#[cfg(feature = "machine_lua")]
impl Sim {
    /// Setup lua states for the individual entities.
    /// This is used during initialization from snapshot.
    pub fn setup_lua_state_ent(&mut self) {
        let mut map: HashMap<StringId, Vec<PathBuf>> = HashMap::new();
        for ent_type in &self.model.entity_types {
            let mut scripts_vec = Vec::new();
            for comp in self
                .model
                .components
                .iter()
                .filter(|c| &c.entity_type == &ent_type.id)
                .collect::<Vec<&model::ComponentModel>>()
            {
                for script_file in &comp.script_files {
                    if !scripts_vec.contains(script_file) {
                        scripts_vec.push(script_file.clone());
                    }
                }
                //                println!("ent_type: {}",
                // ent_type.id);
                // println!("script files: {:?}",
                // comp.script_files);
            }
            map.insert(StringId::from_truncate(&ent_type.id), scripts_vec);
        }

        for ((mut ent_type, mut ent_id), mut entity) in &mut self.entities {
            if entity.insta.lua_state.is_some() {
                continue;
            }
            if let Some(scripts) = map.get(&ent_type) {
                let mut lua_state = Lua::new();
                lua_state.context(|ctx| {
                    for script_path in scripts {
                        let mut file = match File::open(script_path) {
                            Ok(f) => f,
                            Err(e) => {
                                warn!("{}", e);
                                continue;
                            }
                        };
                        let mut contents = String::new();
                        file.read_to_string(&mut contents);
                        let script_path_str =
                            script_path.to_str().expect("failed to_str on pathbuf");
                        let last_three = script_path_str.rsplitn(4, "/").collect::<Vec<&str>>();
                        let last_three =
                            format!("{}/{}/{}", last_three[2], last_three[1], last_three[0]);
                        ctx.load(&contents)
                            .set_name(&last_three)
                            .expect(
                                "failed set_name() on lua chunk while setting entity's lua_state",
                            )
                            .exec()
                            .expect("failed exec() on lua chunk while setting entity's lua_state");
                    }
                });
                entity.insta.lua_state = Some(Arc::new(Mutex::new(lua_state)));
            }
        }
    }
}

/// Data access helpers.
impl Sim {
    /// Get all vars, coerce each to string.
    pub fn get_all_as_strings(&self) -> HashMap<String, String> {
        let mut out_map = HashMap::new();
        // for (ent_str, ent_uid) in &self.entities_idx {
        for (ent_uid, ent) in &self.entities {
            // if let Some(ent) = self.entities.get(ent_uid) {
            let mut ent_str = ent_uid.to_string();
            if let Some((ent_id, _)) = &self.entity_idx.iter().find(|(id, uid)| uid == &ent_uid) {
                ent_str = ent_id.to_string();
            }
            out_map.extend(ent.storage.map.iter().map(|((comp_id, var_id), v)| {
                (
                    format!(":{}:{}:{}", ent_str, comp_id, var_id),
                    v.to_string(),
                )
            }));
        }
        out_map
    }

    pub fn get_vars(&self, find_entity_names: bool) -> Result<Vec<(String, &Var)>> {
        let mut out = Vec::new();
        for (ent_id, entity) in &self.entities {
            let mut ent_name: EntityName = EntityName::from(ent_id.to_string());
            if find_entity_names {
                if let Some((_ent_name, _)) = self.entity_idx.iter().find(|(_, id)| id == &ent_id) {
                    ent_name = _ent_name.clone();
                }
            }
            out.extend(
                entity
                    .storage
                    .map
                    .iter()
                    .map(|((comp_name, var_name), var)| {
                        (format!("{}:{}:{}", ent_name, comp_name, var_name), var)
                    })
                    .collect::<Vec<(String, &Var)>>(),
            );
        }
        Ok(out)
    }

    /// Get a `Var` from the sim using an absolute address.
    pub fn get_var(&self, addr: &Address) -> Result<&Var> {
        if let Some(ent_uid) = self.entity_idx.get(&addr.entity) {
            if let Some(ent) = self.entities.get(ent_uid) {
                return ent.storage.get_var(&addr.storage_index());
            }
        } else if addr.entity.chars().all(char::is_numeric) {
            if let Some(ent) = self.entities.get(
                &addr
                    .entity
                    .parse::<u32>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?,
            ) {
                return ent.storage.get_var(&addr.storage_index());
            }
        }
        Err(Error::FailedGettingVarFromSim(addr.clone()))
    }

    /// Get a variable from the sim using an absolute address.
    pub fn get_var_mut(&mut self, addr: &Address) -> Result<&mut Var> {
        if let Some(ent_uid) = self.entity_idx.get(&addr.entity) {
            if let Some(ent) = self.entities.get_mut(ent_uid) {
                return ent.storage.get_var_mut(&addr.storage_index());
            }
        } else if addr.entity.chars().all(char::is_numeric) {
            if let Some(ent) = self.entities.get_mut(
                &addr
                    .entity
                    .parse::<u32>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?,
            ) {
                return ent.storage.get_var_mut(&addr.storage_index());
            }
        }
        Err(Error::FailedGettingVarFromSim(addr.clone()))
    }

    /// Set a var at address using a string value as input.
    pub fn set_from_string(&mut self, addr: &Address, val: &String) -> Result<()> {
        match addr.var_type {
            VarType::String => {
                *self.get_var_mut(&addr)?.as_string_mut()? = val.clone();
            }
            VarType::Int => {
                *self.get_var_mut(&addr)?.as_int_mut()? = val
                    .parse::<crate::Int>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?;
            }
            VarType::Float => {
                *self.get_var_mut(&addr)?.as_float_mut()? = val
                    .parse::<crate::Float>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?;
            }
            VarType::Bool => {
                *self.get_var_mut(&addr)?.as_bool_mut()? = val
                    .parse::<bool>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?;
            }
            _ => debug!(
                "set_from_string not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }

    /// Set a var of any type using a string list as input.
    pub fn set_from_string_list(&mut self, addr: &Address, vec: &Vec<String>) -> Result<()> {
        match addr.var_type {
            // VarType::StringList => {
            //     *self.get_var_mut(&addr)?.as_str_list_mut()? = vec.clone();
            // }
            // VarType::IntList => {
            //     *self.get_var_mut(&addr)?.as_int_list_mut()? = vec
            //         .iter()
            //         .map(|is| is.parse::<crate::Int>().unwrap())
            //         .collect();
            // }
            // VarType::FloatList => {
            //     *self.get_var_mut(&addr)?.as_float_list_mut()? = vec
            //         .iter()
            //         .map(|fs| fs.parse::<crate::Float>().unwrap())
            //         .collect();
            // }
            // VarType::BoolList => {
            //     *self.get_var_mut(&addr)?.as_bool_list_mut()? =
            //         vec.iter().map(|bs| bs.parse::<bool>().unwrap()).collect();
            // }
            _ => error!(
                "set_from_string_list not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }
}

#[cfg(feature = "grids")]
/// Grids related functions.
impl Sim {
    /// Set a var of any type using a string grid as input.
    pub fn set_from_string_grid(&mut self, addr: &Address, vec2d: &Vec<Vec<String>>) -> Result<()> {
        match addr.var_type {
            // VarType::StringGrid => {
            //     *self.get_var_mut(&addr)?.as_str_grid_mut()? = vec2d.clone();
            // }
            // VarType::IntGrid => {
            //     *self.get_var_mut(&addr)?.as_int_grid_mut()? = vec2d
            //         .iter()
            //         .map(|v| {
            //             v.iter()
            //                 .map(|is| is.parse::<crate::Int>().unwrap())
            //                 .collect()
            //         })
            //         .collect();
            // }
            // VarType::FloatGrid => {
            //     *self.get_var_mut(&addr)?.as_float_grid_mut()? = vec2d
            //         .iter()
            //         .map(|v| {
            //             v.iter()
            //                 .map(|fs| fs.parse::<crate::Float>().unwrap())
            //                 .collect()
            //         })
            //         .collect();
            // }
            // VarType::BoolGrid => {
            //     *self.get_var_mut(&addr)?.as_bool_grid_mut()? = vec2d
            //         .iter()
            //         .map(|v| v.iter().map(|bs| bs.parse::<bool>().unwrap()).collect())
            //         .collect();
            // }
            _ => error!(
                "set_from_string_grid not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }

    // TODO support more image types
    /// Apply image data as found in the model.
    #[cfg(feature = "load_img")]
    fn apply_data_img(&mut self) -> Result<()> {
        use self::image::GenericImageView;
        for die in &self.model.data_imgs.clone() {
            //            debug!("loading image: {:?}", die);
            match die {
                DataImageEntry::BmpU8(addr, path) => {
                    let img = image::open(path).unwrap();
                    debug!(
                        "loading image <BmpU8> ({},{}) from path: {}, destination: {}",
                        img.width(),
                        img.height(),
                        path,
                        addr
                    );
                    //                    println!("{}", img.);
                    let img = img.to_luma8();
                    let mut out_grid: Vec<Vec<crate::Var>> = Vec::new();
                    let mut row: Vec<Var> = Vec::new();
                    for (w, h, luma) in img.enumerate_pixels() {
                        row.push(Var::Int(luma[0] as crate::Int));
                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let ig = self.get_var_mut(&addr.parse()?)?.as_grid_mut()?;
                    *ig = out_grid;
                }
                DataImageEntry::PngU8U8U8Concat(addr, path) => {
                    let img = image::open(path).unwrap();
                    debug!(
                        "loading image <PngU8U8U8Concat> ({},{}) from path: {}, destination: {}",
                        img.width(),
                        img.height(),
                        path,
                        addr
                    );
                    let width = img.width();
                    let img = img.to_rgb8();

                    let mut out_grid: Vec<Vec<Var>> = Vec::new();
                    let mut row: Vec<Var> = Vec::new();
                    for (w, h, rgb) in img.enumerate_pixels() {
                        let combined = (rgb.0[0] as u32 * 10_u32.pow(3) + rgb.0[1] as u32)
                            * 10_u32.pow(3)
                            + rgb.0[2] as u32;
                        row.push(Var::Int(combined as crate::Int));

                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let deal = addr.parse().expect("failed creating addr from str");
                    let ig = self.get_var_mut(&deal)?.as_grid_mut()?;
                    *ig = out_grid;
                }
                DataImageEntry::PngU8U8U8(addr, path) => {
                    let img = image::open(path).unwrap();
                    debug!(
                        "loading image ({},{}) from path: {}, destination: {}",
                        img.width(),
                        img.height(),
                        path,
                        addr
                    );
                    let width = img.width();
                    let img = img.to_rgb8();

                    let mut out_grid: Vec<Vec<Var>> = Vec::new();
                    let mut row: Vec<Var> = Vec::new();
                    for (w, h, rgb) in img.enumerate_pixels() {
                        let c = 65536 * rgb.0[0] as u32 + 256 * rgb.0[1] as u32 + rgb.0[2] as u32;
                        row.push(Var::Int(c as crate::Int));
                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let ig = self.get_var_mut(&Address::from_str(addr)?)?.as_grid_mut()?;
                    *ig = out_grid;
                }
                _ => (),
            }
        }
        Ok(())
    }
}

// TODO revise data applying
/// Data applying functions.
impl Sim {
    /// Apply regular data as found in data declarations in user files.
    fn apply_data_reg(&mut self) {
        for de in &self.model.data.clone() {
            match de {
                DataEntry::Simple((addr, val)) => {
                    let addr = match Address::from_str(&addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    self.set_from_string(&addr, &val);
                }
                DataEntry::List((addr, vec)) => {
                    let addr = match Address::from_str(&addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    self.set_from_string_list(&addr, &vec);
                }
                #[cfg(feature = "grids")]
                DataEntry::Grid((addr, vec2d)) => {
                    let addr = match Address::from_str(&addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    self.set_from_string_grid(&addr, vec2d);
                }
            }
        }
    }

    /// Apply settings as found in scenario manifest.
    fn apply_settings(&mut self) {
        for (addr, val) in &self.model.scenario.manifest.settings.clone() {
            let addr = match Address::from_str(&addr) {
                Ok(a) => a,
                Err(_) => continue,
            };
            use crate::util::coerce_toml_val_to_string;
            match addr.var_type {
                VarType::String | VarType::Int | VarType::Float | VarType::Bool | VarType::Byte => {
                    //                    println!("{}", &addr.to_string());
                    self.set_from_string(&addr, &val);
                }
                _ => unimplemented!(), //                    self.set_from_string(&addr, &val.as_str().to_string()),
                                       // VarType::StringList
                                       // | VarType::IntList
                                       // | VarType::FloatList
                                       // | VarType::BoolList
                                       // | VarType::ByteList => {
                                       //     unimplemented!()
                                       // self.set_from_string_list(
                                       //     &addr,
                                       //     &val.as_array()
                                       //         .unwrap()
                                       //         .iter()
                                       //         .map(|v| coerce_toml_val_to_string(&v))
                                       //         .collect(),
                                       // );
                                       // }
                                       // #[cfg(feature = "grids")]
                                       // VarType::StringGrid
                                       // | VarType::IntGrid
                                       // | VarType::FloatGrid
                                       // | VarType::BoolGrid
                                       // | VarType::ByteGrid => {
                                       //     unimplemented!()

                                       // self.set_from_string_grid(
                                       //     &addr,
                                       //     &val.as_array()
                                       //         .unwrap()
                                       //         .iter()
                                       //         .map(|v| {
                                       //             v.as_array()
                                       //                 .unwrap()
                                       //                 .iter()
                                       //                 .map(|vv| coerce_toml_val_to_string(&vv))
                                       //                 .collect()
                                       //         })
                                       //         .collect(),
                                       // );
                                       // }
            };
        }
    }
}

/// Entity and component handling functions.
impl Sim {
    /// Gets reference to entity using a valid integer id
    pub fn get_entity(&self, id: &EntityId) -> Result<&Entity> {
        self.entities
            .get(id)
            .ok_or(Error::FailedGettingEntityById(*id))
    }

    /// Gets mutable reference to entity using an integer id
    pub fn get_entity_mut(&mut self, id: &EntityId) -> Result<&mut Entity> {
        self.entities
            .get_mut(id)
            .ok_or(Error::FailedGettingEntityById(*id))
    }

    /// Gets reference to entity using a string id
    pub fn get_entity_by_name(&self, name: &EntityName) -> Result<&Entity> {
        let entity_id = self
            .entity_idx
            .get(name)
            .ok_or(Error::FailedGettingEntityByName(name.to_string()))?;
        self.get_entity(entity_id)
    }

    /// Gets mutable reference to entity using a string id
    pub fn get_entity_by_name_mut(&mut self, name: &EntityName) -> Result<&mut Entity> {
        let entity_id = self
            .entity_idx
            .get(name)
            .ok_or(Error::FailedGettingEntityByName(name.to_string()))?;
        self.entities
            .get_mut(entity_id)
            .ok_or(Error::FailedGettingEntityById(*entity_id))
    }

    /// Gets references to all entity objects
    pub fn get_entities(&self) -> Vec<&Entity> {
        self.entities.values().collect()
    }

    /// Gets mutable references to all entity objects
    pub fn get_entities_mut(&mut self) -> Vec<&mut Entity> {
        self.entities.values_mut().collect()
    }

    /// Gets references to all entities that have the same set of components
    pub fn get_entities_of_type(&self, type_: &Vec<CompName>) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|(_, e)| type_.iter().all(|c| e.components.contains(c)))
            .map(|(_, e)| e)
            .collect()
    }

    /// Gets references to component variable collections from all entities,
    /// content of each collection being same as specified in the component
    /// model definition.
    ///
    /// NOTE: If a variable exists within the context of a component but wasn't
    /// included in the component model then it will not be retrieved.
    pub fn get_components(&self, comp_name: &CompName) -> Result<Vec<Vec<&Var>>> {
        let mut out = Vec::new();
        let comp_model = self.model.get_component(comp_name)?;
        for (_, entity) in &self.entities {
            let mut _out = Vec::new();
            for model_var in &comp_model.vars {
                _out.push(
                    entity
                        .storage
                        .get_var(&(comp_name.clone(), model_var.name.clone()))?,
                );
            }
            out.push(_out);
        }
        Ok(out)
    }
}

// TODO use some other (more basic?) scenario
const TEST_SCENARIO_PATH: &str = "../examples/simulation/scenarios/hello_world.toml";

#[test]
fn sim_from_scenario_path() {
    Sim::from_scenario_at(TEST_SCENARIO_PATH).expect("failed starting sim from path to scenario");
}

#[test]
fn sim_from_scenario_struct() {
    let scenario = Scenario::default();
    Sim::from_scenario(scenario).expect("failed starting sim from empty scenario");
}

#[test]
fn sim_step() {
    let mut sim = Sim::from_scenario_at(TEST_SCENARIO_PATH)
        .expect("failed starting sim from path to scenario");
    assert!(sim.step().is_ok());
    assert!(sim.step().is_ok());
    assert!(sim.step().is_ok());
}
