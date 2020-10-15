//! Defines entity structure related functionality.

mod storage;
pub use self::storage::Storage;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use crate::component::Component;
use crate::model::{ComponentModel, EntityModel};
use crate::SimModel;
use crate::{model, CompId, StringId};

use fnv::FnvHashMap;

#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

pub type StorageIndex = (StringId, StringId);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompCollection {
    pub map: FnvHashMap<CompId, Component>,
    /// Queue of components scheduled for execution,
    /// keys are event ids, values are lists of component uids
    pub queue: FnvHashMap<StringId, Vec<CompId>>,
}
impl CompCollection {
    pub fn get(&self, key: &CompId) -> Option<&Component> {
        self.map.get(key)
    }
    pub fn get_mut(&mut self, key: &CompId) -> Option<&mut Component> {
        self.map.get_mut(key)
    }
    pub fn attach(&mut self, model: &SimModel, storage: &mut Storage, comp_name: &StringId) {
        let comp_model = model.get_component(comp_name).unwrap();
        let new_comp = Component::from_model(comp_model);
        for var_model in &comp_model.vars {
            storage.insert(
                &comp_name,
                &var_model.id,
                &var_model.type_,
                &var_model.default,
            );
        }

        //// ignore components that don't have any states
        //// (besides the built-in 'none' state)
        //// TODO
        // if comp_model.logic.states.len() >= 0 {
        // let comp_uid = (IndexString::from(comp_name).unwrap(),);

        if !self.map.contains_key(comp_name) {
            self.map.insert(*comp_name, new_comp);
            for trigger in &comp_model.triggers {
                //                println!("trigger: {}", trigger);
                let t = StringId::from(trigger).unwrap();
                self.queue.get_mut(&t).unwrap().push(*comp_name);
            }
        }
        //}
    }
    pub fn detach(
        &mut self,
        storage: &mut Storage,
        comp_name: &CompId,
        comp_model: &ComponentModel,
    ) {
        storage.remove_comp_vars(comp_name, comp_model);

        // find and remove references to component from all the
        // queues for different events
        for (q, v) in &mut self.queue {
            let n = match v.iter().position(|c| c == comp_name) {
                Some(p) => p,
                None => continue,
            };
            v.remove(n);
            //            println!("removed");
        }
        //        self.queue.iter_mut().map(|(_,v)|
        // v.remove_item(&comp_uid));
        self.map.remove(comp_name);
    }
}

/// Basic building block of the simulation state.
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    /// All data associated with the entity is stored here
    pub storage: Storage,
    /// Component store
    pub components: CompCollection,

    // /// Map of events with lists of components
    // pub comp_queue: FnvHashMap<EventIndex, Vec<CompUid>>,
    /// Non-serializable aspects of an entity
    // TODO use cfg_if to include this only if related features are enabled
    #[serde(skip)]
    pub insta: EntityNonSer,
}

/// Contains all the non-serializable constructs stored on an entity instance.
#[derive(Debug, Default)]
pub struct EntityNonSer {
    #[cfg(feature = "machine_lua")]
    pub lua_state: Option<Arc<Mutex<Lua>>>,
    #[cfg(feature = "machine_dynlib")]
    pub libs: Option<BTreeMap<String, libloading::Library>>,
}

impl Entity {
    //TODO
    pub fn from_prefab(ent_model: &EntityModel, sim_model: &SimModel) -> Option<Entity> {
        let mut ent = Entity {
            // name: ent_model.name,
            storage: Storage::new(),
            components: CompCollection {
                map: FnvHashMap::default(),
                queue: FnvHashMap::default(),
            },
            insta: EntityNonSer::default(),
        };
        ent.components.queue.insert(
            StringId::from(crate::DEFAULT_TRIGGER_EVENT).unwrap(),
            Vec::new(),
        );
        ent.components
            .queue
            .insert(StringId::from("init").unwrap(), Vec::new());
        for event in &sim_model.events {
            ent.components
                .queue
                .insert(StringId::from(&event.id).unwrap(), Vec::new());
        }

        // add meta vars
        //        ent.storage.insert_parse("_meta", "_meta", "id",
        // &VarType::Str, &ent.model_id);
        // ent.storage.insert_parse("_meta", "_meta",
        // "type", &VarType::Str, &ent.model_type);

        // unimplemented!();

        // let mut comp_models = Vec::new();

        // let mut comps = Vec::new();
        // for comp_model in comp_models {
        for comp_model in &sim_model.components {
            // create a new component
            let mut comp = Component::from_model(&comp_model);
            // add component vars to the entity
            for var_model in &comp_model.vars {
                ent.storage.insert(
                    &comp_model.name,
                    &var_model.id,
                    &var_model.type_,
                    &var_model.default,
                );
            }

            // ignore components that don't have any states
            // (besides the built-in 'none' state)
            // unimplemented!();
            // if comp_model.script.states.len() > 1 {
            // comps.push(comp);
            //}
            // comps.push(comp);
            // }

            // push comp refs to ent event queues based on the model
            // triggers unimplemented!();
            // for comp in comps {
            //     let comp_model = sim_model
            //         .get_component(&ent_model_type, &comp.model_type, &comp.model_id)
            //         .unwrap();
            for trigger in &comp_model.triggers {
                let t = StringId::from(trigger).unwrap();
                if let Some(q) = ent.components.queue.get_mut(&t) {
                    q.push(comp_model.name);
                }
            }
            ent.components.map.insert(comp_model.name, comp);
        }

        // setup dyn libs object for this entity
        // let mut libs = HashMap::new();
        // for comp_model in comp_models {
        //     for lib_path in &comp_model.lib_files {
        //         let lib = Library::new(lib_path.clone()).unwrap();
        //         libs.insert(
        //             format!("{}", lib_path.file_stem().unwrap().to_str().unwrap()),
        //             lib,
        //         );
        //     }
        // }
        // ent.libs = libs;

        Some(ent)
    }
    /// Create a new entity from model.
    pub fn from_model_ref(
        prefab: &StringId,
        sim_model: &model::SimModel,
        // comp_models: &Vec<&ComponentModel>,
    ) -> Option<Entity> {
        // let ent_model = &sim_model.entities[ent_model_n];
        let ent_model = sim_model.get_entity(prefab).unwrap();
        Entity::from_prefab(ent_model, sim_model)
    }
    // pub fn get_model<'a>(&'a self, sim_model: &'a model::SimModel) -> Option<&model::EntityModel> {
    //     sim_model.get_entity(&self.model_type, &self.model_id)
    // }
}
