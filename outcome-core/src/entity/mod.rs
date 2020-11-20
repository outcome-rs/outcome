//! Entity structure related definitions.

mod storage;
pub use self::storage::Storage;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};
use crate::model::{ComponentModel, EntityPrefabModel};
use crate::SimModel;
use crate::{model, CompId, StringId};

use fnv::FnvHashMap;

#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

pub type StorageIndex = (StringId, StringId);

// impl CompCollection {
//     pub fn get(&self, key: &CompId) -> Option<&Component> {
//         self.map.get(key)
//     }
//     pub fn get_mut(&mut self, key: &CompId) -> Option<&mut Component> {
//         self.map.get_mut(key)
//     }
//     pub fn attach(
//         &mut self,
//         model: &SimModel,
//         storage: &mut Storage,
//         comp_name: &StringId,
//     ) -> Result<()> {
//         let comp_model = model
//             .get_component(comp_name)
//             .ok_or(Error::NoComponentModel(comp_name.to_string()))?;
//         let new_comp = Component::from_model(comp_model)?;
//         for var_model in &comp_model.vars {
//             storage.insert(
//                 &comp_name,
//                 &var_model.id,
//                 &var_model.type_,
//                 &var_model.default,
//             );
//         }
//
//         //// ignore components that don't have any states
//         //// (besides the built-in 'none' state)
//         //// TODO
//         // if comp_model.logic.states.len() >= 0 {
//         // let comp_uid = (IndexString::from(comp_name).unwrap(),);
//
//         if !self.map.contains_key(comp_name) {
//             self.map.insert(*comp_name, new_comp);
//             for trigger in &comp_model.triggers {
//                 //                println!("trigger: {}", trigger);
//                 let t = StringId::from(trigger).unwrap();
//                 #[cfg(feature = "machine")]
//                 self.queue.get_mut(&t).unwrap().push(*comp_name);
//             }
//         }
//
//         Ok(())
//     }
//     pub fn detach(
//         &mut self,
//         storage: &mut Storage,
//         comp_name: &CompId,
//         comp_model: &ComponentModel,
//     ) {
//         storage.remove_comp_vars(comp_name, comp_model);
//
//         // find and remove references to component from all the
//         // queues for different events
//         #[cfg(feature = "machine")]
//         for (q, v) in &mut self.queue {
//             let n = match v.iter().position(|c| c == comp_name) {
//                 Some(p) => p,
//                 None => continue,
//             };
//             v.remove(n);
//         }
//
//         //        self.queue.iter_mut().map(|(_,v)|
//         // v.remove_item(&comp_uid));
//         self.map.remove(comp_name);
//     }
// }

/// Basic building block of the simulation state.
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    /// All data associated with the entity is stored here
    pub storage: Storage,
    // /// Component store
    // pub components: CompCollection,
    /// asd
    #[cfg(feature = "machine")]
    pub comp_state: FnvHashMap<CompId, StringId>,

    /// Queue of components scheduled for execution,
    /// keys are event ids, values are lists of component uids
    #[cfg(feature = "machine")]
    pub comp_queue: FnvHashMap<StringId, Vec<CompId>>,

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
    /// Creates a new entity using the prefab model.
    fn from_prefab_model(ent_model: &EntityPrefabModel, sim_model: &SimModel) -> Result<Entity> {
        let mut ent = Entity::empty();

        #[cfg(feature = "machine")]
        {
            ent.comp_queue.insert(
                StringId::from_unchecked(crate::DEFAULT_TRIGGER_EVENT),
                Vec::new(),
            );

            ent.comp_queue
                .insert(StringId::from_unchecked("init"), Vec::new());

            for event in &sim_model.events {
                ent.comp_queue
                    .insert(StringId::from_truncate(&event.id), Vec::new());
            }
        }

        for comp_model in &sim_model.components {
            // create a new component
            // let mut comp = Component::from_model(&comp_model)?;
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
            #[cfg(feature = "machine")]
            for trigger in &comp_model.triggers {
                let t = StringId::from_truncate(trigger);
                if let Some(q) = ent.comp_queue.get_mut(&t) {
                    q.push(comp_model.name);
                }
            }
            #[cfg(feature = "machine")]
            ent.comp_state
                .insert(comp_model.name, comp_model.start_state);
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

        Ok(ent)
    }

    /// Creates a new entity from model.
    pub fn from_prefab(prefab: &StringId, sim_model: &model::SimModel) -> Result<Entity> {
        let ent_model = sim_model
            .get_entity(prefab)
            .ok_or(Error::NoEntityPrefab(prefab.to_string()))?;
        Entity::from_prefab_model(ent_model, sim_model)
    }

    /// Creates a new empty entity.
    pub fn empty() -> Self {
        Entity {
            storage: Storage::new(),
            #[cfg(feature = "machine")]
            comp_state: Default::default(),
            #[cfg(feature = "machine")]
            comp_queue: Default::default(),
            insta: EntityNonSer::default(),
        }
    }
}
