//! Entity structure related definitions.

mod storage;

pub use self::storage::Storage;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use fnv::FnvHashMap;

use crate::error::{Error, Result};
use crate::model::{ComponentModel, EntityPrefab};
use crate::{arraystring, EntityName, EventName, SimModel};
use crate::{model, CompName, StringId};

#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

pub use storage::StorageIndex;

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

    /// Current state of each component-tied state machine
    #[cfg(feature = "machine")]
    pub comp_state: FnvHashMap<CompName, StringId>,

    /// Queue of scheduled component-tied machines for each event
    #[cfg(feature = "machine")]
    pub comp_queue: FnvHashMap<EventName, Vec<CompName>>,

    /// Non-serializable aspects of an entity
    // TODO use cfg_if to include this only if related features are enabled
    // #[serde(skip)]
    pub insta: EntityNonSer,
}

/// Contains all the non-serializable constructs stored on an entity instance.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EntityNonSer {
    #[cfg(feature = "machine_lua")]
    pub lua_state: Option<Arc<Mutex<Lua>>>,
    #[cfg(feature = "machine_dynlib")]
    pub libs: Option<BTreeMap<String, libloading::Library>>,
}

impl Entity {
    /// Creates a new entity using the prefab model.
    fn from_prefab(prefab: &EntityPrefab, model: &SimModel) -> Result<Entity> {
        trace!("creating new entity from prefab");
        let mut ent = Entity::empty();

        #[cfg(feature = "machine")]
        {
            // ent.comp_queue.insert(
            //     arraystring::new_unchecked(crate::DEFAULT_TRIGGER_EVENT),
            //     Vec::new(),
            // );
            ent.comp_queue.insert(
                arraystring::new_unchecked(crate::DEFAULT_INIT_EVENT),
                Vec::new(),
            );

            for event in &model.events {
                ent.comp_queue
                    .insert(arraystring::new_truncate(&event.id), Vec::new());
            }
        }

        for comp in &prefab.components {
            ent.attach(*comp, model)?;
        }

        // TODO setup dyn libs
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
    pub fn from_prefab_name(prefab: &EntityName, sim_model: &model::SimModel) -> Result<Entity> {
        trace!("creating entity from prefab name: {}", prefab);
        let ent_model = sim_model
            .get_entity(prefab)
            .ok_or(Error::NoEntityPrefab(*prefab))?;
        Entity::from_prefab(ent_model, sim_model)
    }

    /// Creates a new empty entity.
    pub fn empty() -> Self {
        Entity {
            storage: Storage::default(),
            #[cfg(feature = "machine")]
            comp_state: Default::default(),
            #[cfg(feature = "machine")]
            comp_queue: Default::default(),
            insta: EntityNonSer::default(),
        }
    }

    pub fn attach(&mut self, component: CompName, model: &SimModel) -> Result<()> {
        let comp_model = model.get_component(&component)?;
        debug!("attaching component: {:?}", comp_model);

        for var_model in &comp_model.vars {
            self.storage.insert(
                (component, var_model.id),
                var_model
                    .default
                    .to_owned()
                    .unwrap_or(var_model.type_.default_value()),
            );
        }

        #[cfg(feature = "machine")]
        {
            trace!("triggers: {:?}", comp_model.triggers);
            for trigger in &comp_model.triggers {
                let t = arraystring::new_truncate(trigger);
                if let Some(q) = self.comp_queue.get_mut(&t) {
                    trace!("pushing to comp_queue: {}", comp_model.name);
                    q.push(comp_model.name);
                }
            }
            self.comp_state
                .insert(comp_model.name, comp_model.logic.start_state);
        }

        // debug!("start_state: {}", comp_model.start_state);

        //// ignore components that don't have any states
        //// (besides the built-in 'none' state)
        //// TODO
        // if comp_model.logic.states.len() >= 0 {
        // let comp_uid = (IndexString::from(comp_name).unwrap(),);

        #[cfg(feature = "machine")]
        {
            if !self.comp_state.contains_key(&component) {
                for trigger in &comp_model.triggers {
                    //                println!("trigger: {}", trigger);
                    let t = StringId::from(trigger).unwrap();
                    #[cfg(feature = "machine")]
                    self.comp_queue.get_mut(&t).unwrap().push(component);
                }
            }
        }

        Ok(())
    }

    pub fn detach(&mut self, comp_name: &CompName, sim_model: &SimModel) -> Result<()> {
        self.storage
            .remove_comp_vars(comp_name, sim_model.get_component(comp_name)?);

        #[cfg(feature = "machine")]
        {
            self.comp_state.remove(comp_name);
            // find and remove references to component from all the queues
            // for different events
            for (q, v) in &mut self.comp_queue {
                let n = match v.iter().position(|c| c == comp_name) {
                    Some(p) => p,
                    None => continue,
                };
                v.remove(n);
            }
        }

        Ok(())
    }
}
