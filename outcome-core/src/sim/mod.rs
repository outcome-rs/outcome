//! Local simulation abstraction.

pub mod interface;
pub mod step;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(feature = "load_img")]
use image;
#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::error::Error;
use crate::model::{DataEntry, DataImageEntry, Scenario};
use crate::sim::interface::{SimInterface, SimInterfaceStorage};
use crate::{model, EntityId, Result, SimModel, Var, VarType};
use crate::{EntityUid, StringId};
use fnv::FnvHashMap;

/// Local simulation instance object.
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
    /// Serves as the base for creation and potentially also runtime processing
    /// of the simulation
    pub model: SimModel,

    /// Number of steps that have been processed so far
    pub(crate) clock: usize,
    /// Global queue of events waiting for execution
    pub(crate) event_queue: Vec<StringId>,

    /// All entities that exist within the simulation world are stored here
    pub entities: FnvHashMap<EntityUid, Entity>,
    /// Map of string indexes for entities (string indexes are optional)
    pub entities_idx: FnvHashMap<StringId, EntityUid>,
    /// Pool of integer identifiers for entities
    entity_idpool: id_pool::IdPool,
}

/// Transformations.
impl Sim {
    /// Serialize simulation to a vector of bytes.
    ///
    /// # Compression
    ///
    /// Optional compression using LZ4 algorithm can be performed.
    pub fn to_snapshot(&self, compress: bool) -> Result<Vec<u8>> {
        let mut data = bincode::serialize(&self).unwrap();
        #[cfg(feature = "lz4")]
        if compress {
            data = lz4::block::compress(&data, None, true)?;
        }
        Ok(data)
    }

    /// Create simulation instance from a vector of bytes representing a snapshot.
    pub fn from_snapshot(mut buf: Vec<u8>, compressed: bool) -> Result<Self> {
        if compressed {
            #[cfg(feature = "lz4")]
            let data = lz4::block::decompress(&buf, None)?;
            #[cfg(feature = "lz4")]
            let mut sim: Self = match bincode::deserialize(&data) {
                Ok(ms) => ms,
                Err(e) => return Err(Error::FailedReadingSnapshot("".to_string())),
            };
            #[cfg(not(feature = "lz4"))]
            let mut sim: Self = match bincode::deserialize(&buf) {
                Ok(ms) => ms,
                Err(e) => return Err(Error::FailedReadingSnapshot("".to_string())),
            };
            // TODO handle additional initialization here or create an init function on `Sim`
            // sim.setup_lua_state(&model);
            // sim.setup_lua_state_ent();
            return Ok(sim);
        } else {
            let mut sim: Self = match bincode::deserialize(&buf) {
                Ok(ms) => ms,
                Err(e) => return Err(Error::FailedReadingSnapshot("".to_string())),
            };
            // sim.setup_lua_state(&model);
            // sim.setup_lua_state_ent();
            return Ok(sim);
        }
    }

    /// Create simulation instance using a path to snapshot file.
    ///
    /// # Decompression
    ///
    /// If the snapshot was compressed before saved to disk, reading it
    /// successfully will require setting the `compressed` argument to true.
    pub fn from_snapshot_at(path: &PathBuf, compressed: bool) -> Result<Self> {
        let path = path.canonicalize().unwrap();
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                error!("{}", e);
                return Err(Error::FailedReadingSnapshot("".to_string()));
            }
        };
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf);
        Self::from_snapshot(buf, compressed)
    }
}

impl Sim {
    /// Gets the sim clock value.
    pub fn get_clock(&self) -> usize {
        self.clock
    }

    /// Creates new simulation instance from a path to scenario directory.
    pub fn from_scenario_at_path(path: PathBuf) -> Result<Sim> {
        let scenario = Scenario::from_dir_at(path.clone())?;
        Sim::from_scenario(scenario)
    }

    /// Creates new simulation instance from a &str path to scenario directory.
    pub fn from_scenario_at(path: &str) -> Result<Sim> {
        let path = PathBuf::from(path).canonicalize()?;
        Sim::from_scenario_at_path(path)
    }

    /// Creates new simulation instance from a scenario struct.
    pub fn from_scenario(scenario: Scenario) -> Result<Sim> {
        // first create a model using the given scenario
        let model = SimModel::from_scenario(scenario)?;
        // then create a sim struct using that model
        let mut sim = Sim::from_model(model)?;
        Ok(sim)
    }

    /// Creates a new simulation instance from a model struct.
    pub fn from_model(model: model::SimModel) -> Result<Sim> {
        // create a new sim object
        let mut sim: Sim = Sim {
            model,
            clock: 0,
            event_queue: Vec::new(),
            entities: FnvHashMap::default(),
            entities_idx: FnvHashMap::default(),
            entity_idpool: id_pool::IdPool::new(),
        };

        // TODO load dynlibs
        // load dynamic libraries as seen in component models
        // let mut libs = HashMap::new();
        // for comp_model in &sim.model.components {
        //     for lib_path in &comp_model.lib_files {
        //         let lib = Library::new(lib_path.clone()).unwrap();
        //         libs.insert(
        //             format!("{}", lib_path.file_stem().unwrap().to_str().unwrap()),
        //             lib,
        //         );
        //     }
        // }
        // let mut arc_libs = Arc::new(Mutex::new(libs));
        // TODO setup lua state

        // module script init
        #[cfg(feature = "machine_script")]
        {
            sim.spawn_entity(
                Some(&StringId::from_unchecked("_mod_init")),
                Some(StringId::from_unchecked("_mod_init")),
            )?;
            sim.event_queue.push(StringId::from_unchecked("_scr_init"));
        }

        // add entities
        // sim.apply_model_entities();
        // sim.apply_model();

        // setup entities' lua_state
        // sim.setup_lua_state_ent();
        // apply data as found in user files
        sim.apply_data_reg();
        #[cfg(feature = "load_img")]
        sim.apply_data_img();

        // apply settings from scenario manifest
        sim.apply_settings();

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
    ) -> Result<()> {
        let mut ent = match prefab {
            Some(p) => Entity::from_prefab(p, &self.model)?,
            None => Entity::empty(),
        };

        let new_uid = self.entity_idpool.request_id().unwrap();

        if let Some(n) = &name {
            if !self.entities_idx.contains_key(n) {
                self.entities_idx.insert(*n, new_uid);
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
        Ok(())
    }
}

#[cfg(feature = "machine_lua")]
/// Functionality related to handling lua.
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
    /// Get any var using absolute address and coerce it to string.
    pub fn get_as_string(&self, addr: &Address) -> Option<String> {
        if let Some(var) = self.get_var(addr) {
            return Some(var.to_string());
        }
        None
    }

    /// Get any var by absolute address and coerce it to integer.
    pub fn get_as_int(&self, addr: &Address) -> Option<crate::Int> {
        if let Some(var) = self.get_var(addr) {
            return Some(var.to_int());
        }
        None
    }
    /// Get all vars, coerce each to string.
    pub fn get_all_as_strings(&self) -> HashMap<String, String> {
        let mut out_map = HashMap::new();
        // for (ent_str, ent_uid) in &self.entities_idx {
        for (ent_uid, ent) in &self.entities {
            // if let Some(ent) = self.entities.get(ent_uid) {
            let mut ent_str = ent_uid.to_string();
            if let Some((ent_id, _)) = &self.entities_idx.iter().find(|(id, uid)| uid == &ent_uid) {
                ent_str = ent_id.to_string();
            }
            out_map.extend(
                ent.storage
                    .get_all_coerce_to_string()
                    .into_iter()
                    .map(|(k, v)| (format!(":{}:{}", ent_str, k), v)),
            );
        }
        out_map
    }
    /// Get a `Var` from the sim using an absolute address.
    pub fn get_var(&self, addr: &Address) -> Option<Var> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_var_from_addr(addr, None);
        }
        None
    }
    /// Get a reference to `Str` variable from the sim using
    /// an absolute address.
    pub fn get_str(&self, addr: &Address) -> Option<&String> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `Str` variable from the sim
    /// using an absolute address.
    pub fn get_str_mut(&mut self, addr: &Address) -> Option<&mut String> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `Int` variable from the sim using
    /// an absolute address.
    pub fn get_int(&self, addr: &Address) -> Option<&crate::Int> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `Int` variable from the sim
    /// using an absolute address.
    pub fn get_int_mut(&mut self, addr: &Address) -> Option<&mut crate::Int> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `Float` variable from the sim
    /// using an absolute address.
    pub fn get_float(&self, addr: &Address) -> Option<&crate::Float> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_float(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `Float` variable from the sim
    /// using an absolute address.
    pub fn get_float_mut(&mut self, addr: &Address) -> Option<&mut crate::Float> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_float_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `Bool` variable from the sim
    /// using an absolute address.
    pub fn get_bool(&self, addr: &Address) -> Option<&bool> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_bool(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `Bool` variable from the sim
    /// using an absolute address.
    pub fn get_bool_mut(&mut self, addr: &Address) -> Option<&mut bool> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_bool_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `StrList` variable from the sim
    /// using an absolute address.
    pub fn get_str_list(&self, addr: &Address) -> Option<&Vec<String>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str_list(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `StrList` variable from the
    /// sim using an absolute address.
    pub fn get_str_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<String>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str_list_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `IntList` variable from the sim
    /// using an absolute address.
    pub fn get_int_list(&self, addr: &Address) -> Option<&Vec<crate::Int>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int_list(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `IntList` variable from the
    /// sim using an absolute address.
    pub fn get_int_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<crate::Int>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int_list_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `FloatList` variable from the sim
    /// using an absolute address.
    pub fn get_float_list(&self, addr: &Address) -> Option<&Vec<crate::Float>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_float_list(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `FloatList` variable from the
    /// sim using an absolute address.
    pub fn get_float_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<crate::Float>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity
                .storage
                .get_float_list_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `BoolList` variable from the sim
    /// using an absolute address.
    pub fn get_bool_list(&self, addr: &Address) -> Option<&Vec<bool>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_bool_list(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `BoolList` variable from the
    /// sim using an absolute address.
    pub fn get_bool_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<bool>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity
                .storage
                .get_bool_list_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `StrGrid` variable from the sim
    /// using an absolute address.
    pub fn get_str_grid(&self, addr: &Address) -> Option<&Vec<Vec<String>>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str_grid(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `StrGrid` variable from the
    /// sim using an absolute address.
    pub fn get_str_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<String>>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_str_grid_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `IntGrid` variable from the sim
    /// using an absolute address.
    pub fn get_int_grid(&self, addr: &Address) -> Option<&Vec<Vec<crate::Int>>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int_grid(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `IntGrid` variable from the
    /// sim using an absolute address.
    pub fn get_int_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<crate::Int>>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_int_grid_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `FloatGrid` variable from the sim
    /// using an absolute address.
    pub fn get_float_grid(&self, addr: &Address) -> Option<&Vec<Vec<crate::Float>>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_float_grid(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `FloatGrid` variable from the
    /// sim using an absolute address.
    pub fn get_float_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<crate::Float>>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity
                .storage
                .get_float_grid_mut(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a reference to `BoolGrid` variable from the sim
    /// using an absolute address.
    pub fn get_bool_grid(&self, addr: &Address) -> Option<&Vec<Vec<bool>>> {
        if let Some(entity) = self
            .entities
            .get(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity.storage.get_bool_grid(&(addr.get_storage_index()));
        }
        None
    }
    /// Get a mut reference to `BoolGrid` variable from the
    /// sim using an absolute address.
    pub fn get_bool_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<bool>>> {
        if let Some(entity) = self
            .entities
            .get_mut(self.entities_idx.get(&addr.entity).unwrap())
        {
            return entity
                .storage
                .get_bool_grid_mut(&(addr.get_storage_index()));
        }
        None
    }

    /// Set a var at address using a string value as input.
    pub fn set_from_string(&mut self, addr: &Address, val: &String) -> Result<()> {
        match addr.var_type {
            //            VarType::Str => *self.get_str_mut(addr).unwrap() = val,
            //            VarType::Int => *self.get_int_mut(addr).unwrap() =
            // val.parse::<crate::Int>().unwrap(),            VarType::Float =>
            // *self.get_float_mut(addr).unwrap() = val.parse::<crate::Float>().unwrap(),
            //            VarType::Bool => *self.get_bool_mut(addr).unwrap() =
            // val.parse::<bool>().unwrap(),            _ =>
            // unimplemented!("set_from_string not yet implemented for var type {:?}",
            // addr.var_type),
            VarType::Str => {
                if let Some(v) = self.get_str_mut(&addr) {
                    *v = val.clone();
                } else {
                    debug!(
                        "failed setting str from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::Int => {
                if let Some(v) = self.get_int_mut(&addr) {
                    *v = val.parse::<crate::Int>().unwrap();
                } else {
                    debug!(
                        "failed setting int from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::Float => {
                *self.get_float_mut(&addr).unwrap() = val.parse::<crate::Float>().unwrap()
            }
            VarType::Bool => *self.get_bool_mut(&addr).unwrap() = val.parse::<bool>().unwrap(),
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
            VarType::StrList => {
                if let Some(v) = self.get_str_list_mut(&addr) {
                    *v = vec.clone();
                } else {
                    error!(
                        "failed setting str_list from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::IntList => {
                if let Some(v) = self.get_int_list_mut(&addr) {
                    *v = vec
                        .iter()
                        .map(|is| is.parse::<crate::Int>().unwrap())
                        .collect();
                } else {
                    error!(
                        "failed setting int_list from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::FloatList => {
                if let Some(v) = self.get_float_list_mut(&addr) {
                    *v = vec
                        .iter()
                        .map(|fs| fs.parse::<crate::Float>().unwrap())
                        .collect();
                } else {
                    error!(
                        "failed setting float_list from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::BoolList => {
                if let Some(v) = self.get_bool_list_mut(&addr) {
                    *v = vec.iter().map(|bs| bs.parse::<bool>().unwrap()).collect();
                } else {
                    error!(
                        "failed setting bool_list from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            _ => error!(
                "set_from_string_list not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }
    /// Set a var of any type using a string grid as input.
    pub fn set_from_string_grid(&mut self, addr: &Address, vec2d: &Vec<Vec<String>>) -> Result<()> {
        match addr.var_type {
            VarType::StrGrid => {
                if let Some(v) = self.get_str_grid_mut(&addr) {
                    *v = vec2d.clone();
                } else {
                    error!(
                        "failed setting str_grid from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::IntGrid => {
                if let Some(v) = self.get_int_grid_mut(&addr) {
                    *v = vec2d
                        .iter()
                        .map(|v| {
                            v.iter()
                                .map(|is| is.parse::<crate::Int>().unwrap())
                                .collect()
                        })
                        .collect();
                } else {
                    error!(
                        "failed setting int_grid from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::FloatGrid => {
                if let Some(v) = self.get_float_grid_mut(&addr) {
                    *v = vec2d
                        .iter()
                        .map(|v| {
                            v.iter()
                                .map(|fs| fs.parse::<crate::Float>().unwrap())
                                .collect()
                        })
                        .collect();
                } else {
                    error!(
                        "failed setting float_grid from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            VarType::BoolGrid => {
                if let Some(v) = self.get_bool_grid_mut(&addr) {
                    *v = vec2d
                        .iter()
                        .map(|v| v.iter().map(|bs| bs.parse::<bool>().unwrap()).collect())
                        .collect();
                } else {
                    error!(
                        "failed setting bool_grid from data at addr: {}",
                        &addr.to_string()
                    );
                }
            }
            _ => error!(
                "set_from_string_grid not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }
}

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

    // TODO support more image types
    /// Apply image data as found in the model.
    #[cfg(feature = "load_img")]
    fn apply_data_img(&mut self) {
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
                    let img = img.to_luma();
                    let mut out_grid: Vec<Vec<crate::Int>> = Vec::new();
                    let mut row: Vec<crate::Int> = Vec::new();
                    for (w, h, luma) in img.enumerate_pixels() {
                        row.push(luma[0] as crate::Int);
                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let ig = self
                        .get_int_grid_mut(&Address::from_str(&addr).unwrap())
                        .unwrap();
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
                    let img = img.to_rgb();

                    let mut out_grid: Vec<Vec<crate::Int>> = Vec::new();
                    let mut row: Vec<crate::Int> = Vec::new();
                    for (w, h, rgb) in img.enumerate_pixels() {
                        let combined = (rgb.0[0] as u32 * 10_u32.pow(3) + rgb.0[1] as u32)
                            * 10_u32.pow(3)
                            + rgb.0[2] as u32;
                        row.push(combined as crate::Int);

                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let deal = Address::from_str(&addr).expect("failed creating addr from str");
                    let ig = match self.get_int_grid_mut(&deal) {
                        Some(i) => i,
                        None => {
                            error!("get_int_grid_mut failed while loading image");
                            continue;
                        }
                    };
                    //                    let ig = self.get_int_grid_mut(
                    //                        &Address::from_str(&addr,
                    // Some(&model)).expect("failed creating
                    // addr from str"))
                    // .expect("get_int_grid_mut failed");
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
                    let img = img.to_rgb();

                    let mut out_grid: Vec<Vec<crate::Int>> = Vec::new();
                    let mut row: Vec<crate::Int> = Vec::new();
                    for (w, h, rgb) in img.enumerate_pixels() {
                        let c = 65536 * rgb.0[0] as u32 + 256 * rgb.0[1] as u32 + rgb.0[2] as u32;
                        row.push(c as crate::Int);
                        if (w + 1) % img.width() == 0 {
                            out_grid.push(row);
                            row = Vec::new();
                        }
                    }
                    let ig = self
                        .get_int_grid_mut(&Address::from_str(&addr).unwrap())
                        .unwrap();
                    *ig = out_grid;
                }
                _ => (),
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
                VarType::Str | VarType::Int | VarType::Float | VarType::Bool => {
                    //                    println!("{}", &addr.to_string());
                    self.set_from_string(&addr, &coerce_toml_val_to_string(&val));
                }
                //                    self.set_from_string(&addr, &val.as_str().to_string()),
                VarType::StrList | VarType::IntList | VarType::FloatList | VarType::BoolList => {
                    self.set_from_string_list(
                        &addr,
                        &val.as_array()
                            .unwrap()
                            .iter()
                            .map(|v| coerce_toml_val_to_string(&v))
                            .collect(),
                    );
                }
                VarType::StrGrid | VarType::IntGrid | VarType::FloatGrid | VarType::BoolGrid => {
                    self.set_from_string_grid(
                        &addr,
                        &val.as_array()
                            .unwrap()
                            .iter()
                            .map(|v| {
                                v.as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|vv| coerce_toml_val_to_string(&vv))
                                    .collect()
                            })
                            .collect(),
                    );
                }
            };
        }
    }
}

/// Entity and component handling functions.
impl Sim {
    fn attach_component(
        &mut self,
        comp_model: model::ComponentModel,
        entity_uid: String,
    ) -> Result<()> {
        unimplemented!();
        //        let comp =
        // Component::from_model(&comp_model);
        //        let comp_id = format!("{}/{}",
        // &comp_model.type_, &comp_model.id);
        // self.model.components. push(comp_model);
        // let mut ent = self.get_entity_mut(&
        // entity_uid).unwrap();        if !ent.
        // components.contains_key(&comp_id) {
        // ent.components.insert(comp_id, comp);
        // Ok(())        } else {
        //            Err(format!("couldn't attach
        // component: \"{}\" already
        // exists on entity \"{}\"", comp_id, entity_uid))
        // }
    }
    fn detach_component(&mut self, entity_uid: String, comp_id: String) -> Result<()> {
        unimplemented!();
        //        let ent =
        // self.get_entity_mut(&entity_uid).unwrap();
        //        if ent.components.contains_key(&comp_id) {
        //            ent.components.remove(&comp_id);
        //            Ok(())
        //        } else {
        //            Err(format!("couldn't detach
        // component: \"{}\" doesn't exist on entity
        // \"{}\"", comp_id, entity_uid))        }
    }

    /// Add a new entity.
    pub fn add_entity_old(&mut self, model_type: &str, model_id: &str, new_id: &str) -> Result<()> {
        unimplemented!()
        //     let ent_suid = (
        //         IndexString::from_str(model_type).unwrap(),
        //         IndexString::from_str(new_id).unwrap(),
        //     );
        //     let mut ent = Entity::from_model_ref(&ent_suid, &self.model).unwrap();
        //     //        ent.id = new_id;
        //
        //     if !self.entities_idx.contains_key(&ent_suid) {
        //         let new_uid = self.entity_idpool.request_id().unwrap();
        //         self.entities_idx.insert(ent_suid, new_uid);
        //         self.entities.insert(new_uid, ent);
        //         Ok(())
        //     } else {
        //         let (a, b) = ent_suid;
        //         //            debug!("{}", format!("Failed to add entity:
        //         // entity with id \"{}/{}\" already exists", a,
        //         // b));
        //         Err(format!(
        //             "Failed to add entity: entity with id \"{}/{}\" already exists",
        //             a, b
        //         ))
        //     }
    }
    //    /// Remove an entity.
    //    pub fn remove_entity(&mut self, ent_uid: &str) ->
    // Result<(), String> {        unimplemented!();
    //        if self.entities.contains_key(ent_uid) {
    //            self.entities.remove(ent_uid);
    //            Ok(())
    //        } else {
    //            Err(format!("Failed to remove entity: entity
    // with key \"{}\" doesn't exist", ent_uid))        }
    //    }

    /// Get reference to entity using a valid integer id.
    pub fn get_entity(&self, uid: &EntityUid) -> Option<&Entity> {
        self.entities.get(uid)
    }
    /// Get mutable reference to entity using a valid integer id.
    pub fn get_entity_mut(&mut self, uid: &EntityUid) -> Option<&mut Entity> {
        self.entities.get_mut(uid)
    }
    /// Get reference to entity using a str literal.
    pub fn get_entity_str(&self, name: &StringId) -> Option<&Entity> {
        self.entities.get(self.entities_idx.get(name).unwrap())
    }
    /// Get mutable reference to entity using a str literal.
    pub fn get_entity_str_mut(&mut self, name: &StringId) -> Option<&mut Entity> {
        self.entities.get_mut(self.entities_idx.get(name).unwrap())
    }
    // pub fn get_component(&self, entity: &StringId, comp: &StringId) -> Option<&Component> {
    //     self.get_entity_str(entity).unwrap().components.get(comp)
    // }
    // pub fn get_component_mut(
    //     &mut self,
    //     entity: &StringId,
    //     component: &StringId,
    // ) -> Option<&mut Component> {
    //     self.get_entity_str_mut(entity)
    //         .unwrap()
    //         .components
    //         .get_mut(component)
    // }
    pub fn get_entities(&self) -> Vec<&Entity> {
        self.entities.values().collect()
    }
    pub fn get_entities_mut(&mut self) -> Vec<&mut Entity> {
        self.entities.values_mut().collect()
    }
    pub fn get_entities_of_type(&self, type_: &Vec<StringId>) -> Vec<&Entity> {
        unimplemented!()
    }
    //    pub fn get_entity_model(&self, type_: &str, id: &str)
    // -> Option<&model::Entity> {
    // self.model.get_entity(type_, id)    }
    //    pub fn get_component_model(&self, type_: &str, id:
    // &str) -> Option<&model::Component> {
    // self.model.get_component(type_, id)    }
}

#[test]
fn sim_from_scenario_path() {
    let mut sim = Sim::from_scenario_at("../scenarios/barebones");
    assert!(sim.is_ok());
}

// #[test]
// fn sim_from_scenario_struct() {
//     let scenario_manifest = crate::model::ScenarioManifest {};
//     let scenario = crate::model::Scenario {};
//     let (model, sim_instance) = match outcome::Sim::from_scenario(scenario) {};
// }

#[test]
fn sim_step() {
    let mut sim = Sim::from_scenario_at("../scenarios/barebones").unwrap();
    assert!(sim.step().is_ok());
    assert!(sim.step().is_ok());
    assert!(sim.step().is_ok());
}
