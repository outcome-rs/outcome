//! Local simulation abstraction.

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
use crate::{arraystring, model, EntityName, Result, SimModel, Var, VarType};
use crate::{EntityId, StringId};
use fnv::FnvHashMap;

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
    pub event_queue: Vec<StringId>,

    /// All entities that exist within the simulation are stored here
    pub entities: FnvHashMap<EntityId, Entity>,
    /// Map of string indexes for entities (string indexes are optional)
    pub entities_idx: FnvHashMap<StringId, EntityId>,
    /// Pool of integer identifiers for entities
    entity_idpool: id_pool::IdPool,
}

/// Snapshot functionality.
impl Sim {
    /// Serialize simulation to a vector of bytes.
    ///
    /// # Compression
    ///
    /// Optional compression using LZ4 algorithm can be performed.
    pub fn to_snapshot(&self, compress: bool) -> Result<Vec<u8>> {
        let mut data: Vec<u8> = bincode::serialize(&self).unwrap();
        #[cfg(feature = "lz4")]
        {
            if compress {
                data = lz4::block::compress(&data, None, true)?;
            }
        }
        Ok(data)
    }

    /// Create simulation instance from a vector of bytes representing a snapshot.
    pub fn from_snapshot(mut buf: &Vec<u8>, compressed: bool) -> Result<Self> {
        if compressed {
            #[cfg(feature = "lz4")]
            let data = lz4::block::decompress(&buf, None)?;
            #[cfg(feature = "lz4")]
            let mut sim = match bincode::deserialize::<Sim>(&data) {
                Ok(ms) => ms,
                Err(e) => return Err(Error::FailedReadingSnapshot(format!("{}", e))),
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
            // let mut sim = match bincode::deserialize::<Sim>(&buf) {
            //     Ok(ms) => ms,
            //     Err(e) => return Err(Error::FailedReadingSnapshot(format!("{}", e))),
            // };

            let mut sim = bincode::deserialize::<Sim>(&buf).unwrap();
            // sim.setup_lua_state(&model);
            // sim.setup_lua_state_ent();
            return Ok(sim);
        }
    }

    /// Create simulation instance using a path to snapshot file.
    pub fn from_snapshot_at(path: &str) -> Result<Self> {
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
        if let Ok(s) = Self::from_snapshot(&buf, true) {
            return Ok(s);
        } else {
            return Self::from_snapshot(&buf, false);
        }
    }
}

impl Sim {
    /// Gets the sim clock value.
    pub fn get_clock(&self) -> usize {
        self.clock
    }

    /// Creates new simulation instance from a path to scenario directory.
    pub fn from_scenario_at_path(path: PathBuf) -> Result<Sim> {
        let scenario = Scenario::from_path(path.clone())?;
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
                Some(&arraystring::new_unchecked("_mod_init")),
                Some(arraystring::new_unchecked("_mod_init")),
            )?;
            sim.event_queue
                .push(arraystring::new_unchecked("_scr_init"));
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
    ) -> Result<()> {
        trace!("starting spawn_entity");

        trace!("creating new entity");
        let mut ent = match prefab {
            Some(p) => Entity::from_prefab_name(p, &self.model)?,
            None => Entity::empty(),
        };
        trace!("done");

        trace!("getting new_uid from pool");
        let new_uid = self.entity_idpool.request_id().unwrap();
        trace!("done");

        trace!("inserting entity");
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
        trace!("done");

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
    /// Get any var using absolute address and coerce it to string.
    pub fn get_as_string(&self, addr: &Address) -> Result<String> {
        Ok(self.get_var(addr)?.to_string())
    }

    /// Get any var by absolute address and coerce it to integer.
    pub fn get_as_int(&self, addr: &Address) -> Result<crate::Int> {
        Ok(self.get_var(addr)?.to_int())
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
            out_map.extend(ent.storage.map.iter().map(|((comp_id, var_id), v)| {
                (
                    format!(":{}:{}:{}", ent_str, comp_id, var_id),
                    v.to_string(),
                )
            }));
        }
        out_map
    }

    /// Get a `Var` from the sim using an absolute address.
    pub fn get_var(&self, addr: &Address) -> Result<&Var> {
        if let Some(ent_uid) = self.entities_idx.get(&addr.entity) {
            if let Some(ent) = self.entities.get(ent_uid) {
                return ent.storage.get_var(&addr.storage_index());
            }
        }
        if let Some(ent) = self.entities.get(
            &addr
                .entity
                .parse::<u32>()
                .map_err(|e| Error::ParsingError(e.to_string()))?,
        ) {
            return ent.storage.get_var(&addr.storage_index());
        }
        Err(Error::FailedGettingVariable(addr.to_string()))
    }

    /// Get a variable from the sim using an absolute address.
    pub fn get_var_mut(&mut self, addr: &Address) -> Result<&mut Var> {
        if let Some(ent_uid) = self.entities_idx.get(&addr.entity) {
            if let Some(ent) = self.entities.get_mut(ent_uid) {
                return ent.storage.get_var_mut(&addr.storage_index());
            }
        } else {
            if let Some(ent) = self.entities.get_mut(
                &addr
                    .entity
                    .parse::<u32>()
                    .map_err(|e| Error::ParsingError(e.to_string()))?,
            ) {
                return ent.storage.get_var_mut(&addr.storage_index());
            }
        }
        Err(Error::FailedGettingVariable(addr.to_string()))
    }

    /// Set a var at address using a string value as input.
    pub fn set_from_string(&mut self, addr: &Address, val: &String) -> Result<()> {
        match addr.var_type {
            VarType::Str => {
                *self.get_var_mut(&addr)?.as_str_mut()? = val.clone();
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
            VarType::StrList => {
                *self.get_var_mut(&addr)?.as_str_list_mut()? = vec.clone();
            }
            VarType::IntList => {
                *self.get_var_mut(&addr)?.as_int_list_mut()? = vec
                    .iter()
                    .map(|is| is.parse::<crate::Int>().unwrap())
                    .collect();
            }
            VarType::FloatList => {
                *self.get_var_mut(&addr)?.as_float_list_mut()? = vec
                    .iter()
                    .map(|fs| fs.parse::<crate::Float>().unwrap())
                    .collect();
            }
            VarType::BoolList => {
                *self.get_var_mut(&addr)?.as_bool_list_mut()? =
                    vec.iter().map(|bs| bs.parse::<bool>().unwrap()).collect();
            }
            _ => error!(
                "set_from_string_list not yet implemented for var type {:?}",
                addr.var_type
            ),
        }
        Ok(())
    }
}

#[cfg(feature = "grids")]
impl Sim {
    /// Set a var of any type using a string grid as input.
    pub fn set_from_string_grid(&mut self, addr: &Address, vec2d: &Vec<Vec<String>>) -> Result<()> {
        match addr.var_type {
            VarType::StrGrid => {
                *self.get_var_mut(&addr)?.as_str_grid_mut()? = vec2d.clone();
            }
            VarType::IntGrid => {
                *self.get_var_mut(&addr)?.as_int_grid_mut()? = vec2d
                    .iter()
                    .map(|v| {
                        v.iter()
                            .map(|is| is.parse::<crate::Int>().unwrap())
                            .collect()
                    })
                    .collect();
            }
            VarType::FloatGrid => {
                *self.get_var_mut(&addr)?.as_float_grid_mut()? = vec2d
                    .iter()
                    .map(|v| {
                        v.iter()
                            .map(|fs| fs.parse::<crate::Float>().unwrap())
                            .collect()
                    })
                    .collect();
            }
            VarType::BoolGrid => {
                *self.get_var_mut(&addr)?.as_bool_grid_mut()? = vec2d
                    .iter()
                    .map(|v| v.iter().map(|bs| bs.parse::<bool>().unwrap()).collect())
                    .collect();
            }
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
                        .get_var_mut(&Address::from_str(&addr)?)?
                        .as_int_grid_mut()?;
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
                    let ig = self.get_var_mut(&deal)?.as_int_grid_mut()?;
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
                        .get_var_mut(&Address::from_str(&addr)?)?
                        .as_int_grid_mut()?;
                    *ig = out_grid;
                }
                _ => (),
            }
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
                VarType::Str | VarType::Int | VarType::Float | VarType::Bool => {
                    //                    println!("{}", &addr.to_string());
                    self.set_from_string(&addr, &val);
                }
                //                    self.set_from_string(&addr, &val.as_str().to_string()),
                VarType::StrList | VarType::IntList | VarType::FloatList | VarType::BoolList => {
                    unimplemented!()
                    // self.set_from_string_list(
                    //     &addr,
                    //     &val.as_array()
                    //         .unwrap()
                    //         .iter()
                    //         .map(|v| coerce_toml_val_to_string(&v))
                    //         .collect(),
                    // );
                }
                #[cfg(feature = "grids")]
                VarType::StrGrid | VarType::IntGrid | VarType::FloatGrid | VarType::BoolGrid => {
                    unimplemented!()
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
                }
            };
        }
    }
}

/// Entity and component handling functions.
impl Sim {
    /// Gets reference to entity using a valid integer id
    pub fn get_entity(&self, uid: &EntityId) -> Result<&Entity> {
        self.entities.get(uid).ok_or(Error::NoEntity(*uid))
    }

    /// Gets mutable reference to entity using an integer id
    pub fn get_entity_mut(&mut self, uid: &EntityId) -> Result<&mut Entity> {
        self.entities.get_mut(uid).ok_or(Error::NoEntity(*uid))
    }

    /// Gets reference to entity using a string id
    pub fn get_entity_str(&self, name: &StringId) -> Result<&Entity> {
        let entity_uid = self
            .entities_idx
            .get(name)
            .ok_or(Error::NoEntityIndexed(name.to_string()))?;
        self.get_entity(entity_uid)
    }

    /// Gets mutable reference to entity using a string id
    pub fn get_entity_str_mut(&mut self, name: &StringId) -> Result<&mut Entity> {
        let entity_uid = *self
            .entities_idx
            .get(name)
            .ok_or(Error::NoEntityIndexed(name.to_string()))?;
        self.get_entity_mut(&entity_uid)
    }

    pub fn get_entities(&self) -> Vec<&Entity> {
        self.entities.values().collect()
    }

    pub fn get_entities_mut(&mut self) -> Vec<&mut Entity> {
        self.entities.values_mut().collect()
    }

    /// Gets entities that have the same set of components
    pub fn get_entities_of_type(&self, type_: &Vec<StringId>) -> Vec<&Entity> {
        unimplemented!()
    }
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
