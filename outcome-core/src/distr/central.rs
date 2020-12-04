use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rand::prelude::SliceRandom;

use crate::distr::{
    CentralCommunication, DistrError, EntityAssignMethod, NodeCommunication, Signal,
};
use crate::error::{Error, Result};
use crate::model::Scenario;
use crate::sim::interface::{SimInterface, SimInterfaceStorage};
use crate::{Address, EntityId, EntityUid, ShortString, SimModel, StringId, Var};

#[cfg(feature = "machine")]
use crate::machine::{cmd::CentralExtCommand, cmd::Command, cmd::ExtCommand, ExecutionContext};
#[cfg(feature = "machine")]
use rayon::prelude::*;

use crate::entity::Entity;
use fnv::FnvHashMap;
use id_pool::IdPool;

/// Distributed simulation main authority. Does the necessary
/// coordination work for distributed sim instances.
///
/// It holds the main simulation model object, the current clock
/// and the current event queue. It doesn't hold any entity data.
///
/// Some of its tasks include:
/// - executing central commands that require global authority, for
///   example those mutating the sim model
/// - load balancing, division of entities between nodes
#[derive(Serialize, Deserialize)]
pub struct SimCentral {
    pub model: SimModel,
    pub clock: usize,
    pub event_queue: Vec<StringId>,

    pub node_entities: FnvHashMap<u32, Vec<EntityUid>>,
    // pub entity_node_routes: FnvHashMap<>
    pub entities_idx: FnvHashMap<EntityId, EntityUid>,
    entity_idpool: IdPool,

    ent_spawn_queue: FnvHashMap<u32, Vec<(EntityUid, Option<EntityId>, Option<EntityId>)>>,
    pub model_changes_queue: SimModel,
}

impl SimCentral {
    pub fn get_clock(&self) -> usize {
        self.clock
    }
    pub fn flush_queue<N: CentralCommunication>(&mut self, net: &mut N) -> Result<()> {
        if !self.ent_spawn_queue.is_empty() {
            for (k, v) in &self.ent_spawn_queue {
                net.sig_send_to_node(*k, Signal::SpawnEntities(v.clone()))?;
            }
            self.ent_spawn_queue.clear();
        }

        Ok(())
    }
    pub fn from_model(model: SimModel) -> Result<SimCentral> {
        let mut event_queue = vec![StringId::from_truncate("step")];
        let mut sim_central = SimCentral {
            model: model.clone(),
            clock: 0,
            event_queue,
            node_entities: Default::default(),
            entities_idx: Default::default(),
            entity_idpool: IdPool::new(),
            ent_spawn_queue: Default::default(),
            model_changes_queue: SimModel::default(),
        };
        // module script init
        // #[cfg(feature = "machine_script")]
        // {
        //     sim_central.spawn_entity(
        //         Some(&StringId::from("_mod_init").unwrap()),
        //         StringId::from("_mod_init").unwrap(),
        //         net,
        //     )?;
        //     sim_central
        //         .event_queue
        //         .push(StringId::from("_scr_init").unwrap());
        // }

        Ok(sim_central)
    }
    pub fn apply_model(&mut self) -> Result<()> {
        unimplemented!()
    }
    pub fn get_entity_names(&self) -> Vec<StringId> {
        unimplemented!()
    }
    pub fn add_entity(&mut self, model_name: &str, name: &str) -> Result<()> {
        unimplemented!()
    }
    #[cfg(feature = "machine_lua")]
    pub fn setup_lua_state_ent(&mut self) {
        unimplemented!()
    }
}
pub enum SpawnPolicy {
    Direct(u32),
    Random,
    EqualQuantity,
    EqualTotalSize,
}
impl SimCentral {
    /// Spawns a new entity.
    pub fn spawn_entity(
        &mut self,
        prefab: Option<StringId>,
        name: Option<StringId>,
        policy: SpawnPolicy,
    ) -> Result<()> {
        trace!("spawning entity from central");

        let new_uid = self.entity_idpool.request_id().unwrap();

        if let Some(n) = name {
            if self.entities_idx.contains_key(&n) {
                return Err(Error::Other(format!(
                    "Failed to add entity: entity named \"{}\" already exists",
                    n,
                )));
            }
            self.entities_idx.insert(n, new_uid);
        }

        match policy {
            SpawnPolicy::Direct(node_id) => {
                if !self.ent_spawn_queue.contains_key(&node_id) {
                    self.ent_spawn_queue.insert(node_id, Vec::new());
                }
                self.ent_spawn_queue
                    .get_mut(&node_id)
                    .unwrap()
                    .push((new_uid, prefab, name));
            }
            // TODO
            SpawnPolicy::Random => {
                if self.node_entities.is_empty() {
                    return Err(Error::Other("no nodes available".to_string()));
                }

                // shuffle the existing node ids and draw one
                let mut nums: Vec<&u32> = self.node_entities.keys().collect::<Vec<&u32>>();
                nums.shuffle(&mut rand::thread_rng());
                let node_id = *nums.first().unwrap();

                // create place in the queue for that node
                if !self.ent_spawn_queue.contains_key(node_id) {
                    self.ent_spawn_queue.insert(*node_id, Vec::new());
                }

                // push to the queue
                self.ent_spawn_queue
                    .get_mut(&node_id)
                    .unwrap()
                    .push((new_uid, prefab, name));
            }
            _ => unimplemented!(),
        }

        // self.ent_spawn_queue.push((new_uid, prefab, name));

        // net.sig_send_to_node(0, Signal::SpawnEntities(vec![(new_uid, prefab, name)]));

        Ok(())
    }
    pub fn assign_entities(
        &self,
        node_count: usize,
        method: EntityAssignMethod,
    ) -> Vec<Vec<EntityUid>> {
        match method {
            // EntityAssignMethod::Random => {
            //     let mut ent_models = self.model.entities.clone();
            //     let mut thread_rng = rand::thread_rng();
            //     ent_models.shuffle(&mut thread_rng);
            //
            //     let mut out_vec = Vec::new();
            //     let chunk_size = ent_models.len() / node_count;
            //     for n in 0..node_count {
            //         let mut ent_vec = Vec::new();
            //         if ent_models.len() >= chunk_size {
            //             for cn in 0..chunk_size {
            //                 let em = ent_models.pop().unwrap();
            //                 ent_vec.push(StringId::from(&em.name).unwrap());
            //             }
            //         } else {
            //             for em in &ent_models {
            //                 ent_vec.push(StringId::from(&em.name).unwrap());
            //             }
            //             ent_models.clear();
            //         }
            //         out_vec.push(ent_vec);
            //         //                    let div =
            //         // ent_models
            //     }
            //     return out_vec;
            // }
            _ => unimplemented!(),
        }
    }

    //pub fn execute_remote<E: Sized + DistrError, C: Connection<E> + Sized + Sync + Send>(
    //&mut self,
    //commands: &Vec<Command>,
    //entity_node_map: &HashMap<EntityUid, String>,
    //mut addr_book: &mut HashMap<String, C>,
    //) {
    //// let ent =
    //}

    pub fn step_network<N: CentralCommunication>(
        &mut self,
        network: &mut N,
        event_queue: Vec<StringId>,
    ) -> Result<()> {
        debug!("starting processing step");
        // tell nodes to start processing step
        network.sig_broadcast(Signal::StartProcessStep(event_queue))?;
        debug!("sent `StartProcessStep` signal to all nodes");

        debug!("starting reading incoming signals");
        #[cfg(feature = "machine")]
        let mut cext_cmds: Arc<Mutex<Vec<(ExecutionContext, CentralExtCommand)>>> =
            Arc::new(Mutex::new(Vec::new()));
        loop {
            match network.sig_read() {
                Ok((node_id, signal)) => match signal {
                    #[cfg(feature = "machine")]
                    Signal::ExecuteCentralExtCmd(cmd) => cext_cmds.lock().unwrap().push(cmd),
                    Signal::EndOfMessages => break,
                    Signal::ProcessStepFinished => break,
                    _ => unimplemented!(),
                },
                Err(e) => match e {
                    Error::WouldBlock => continue,
                    _ => break,
                    // DistrError::WouldBlock => continue,
                    // _ => {
                    //     println!("{:?}", e);
                    //     break;
                    // }
                },
            };
        }
        debug!("finished reading incoming signals");

        debug!("starting processing cext commands");
        #[cfg(feature = "machine")]
        for (context, cext_cmd) in cext_cmds.lock().unwrap().iter() {
            // println!("{:?}", cext_cmd);
            //TODO
            cext_cmd.execute_distr(self, network, &context.ent, &context.comp);
        }
        network.sig_broadcast(Signal::UpdateModel(self.model.clone()));
        self.flush_queue(network)?;

        network.sig_broadcast(Signal::EndOfMessages)?;
        loop {
            if let Ok((_, s)) = network.sig_read() {
                match s {
                    Signal::ProcessStepFinished => break,
                    _ => (),
                }
            }
        }
        debug!("finished executing cext commands");

        // self.clock += 1;
        Ok(())
    }

    ///// TODO
    //pub fn step<E: Sized + DistrError, C: Connection<E> + Sized + Sync + Send>(
    //&mut self,
    //entity_node_map: &HashMap<EntityUid, String>,
    //mut addr_book: &mut HashMap<String, C>,
    //) {
    //println!("sim_central start processing step");
    //// `pre` phase

    //// tell nodes to start processing step
    //for (node, mut conn) in addr_book.iter_mut() {
    //conn.send_signal(Signal::StartProcessStep(self.event_queue.clone()));
    //}
    //println!("sim_central finished tell nodes to start processing step");
    //// nodes start their processing routines]
    //// nodes start exchanging data

    //// `loc` phase
    //// entities on different nodes get into processing on their own
    //// entities start sending central_ext commands our way

    //// `post` phase
    //let mut cext_cmds = Arc::new(Mutex::new(Vec::new()));
    //// let mut cext_cmds = Vec::new();
    ////        for (node, (ci, co)) in addr_book {
    //addr_book
    //.par_iter_mut()
    //.for_each(|(node, conn): (&String, &mut C)| {
    ////            thread::spawn(|| {
    //// println!("start loop");
    //let mut msg_count = 0;
    //loop {
    //let msg = match conn.read_signal() {
    //Ok(m) => m,
    //Err(e) => return,
    //// Err(e) => match &e {
    ////     DistrError::WouldBlock => {
    ////         println!("{:?}", e);
    ////         return;
    ////     }
    ////     DistrError::Other(s) => {
    ////         println!("{:?}", e);
    ////         return;
    ////     }
    ////     _ => return,
    //// },
    //};
    //msg_count += 1;
    //// println!("{}: {:?}", msg_count, msg);
    //match msg {
    //Signal::ProcessStepFinished => return,
    //Signal::EndOfMessages => {
    //println!("end of messages");
    //return;
    //}
    //Signal::ExecuteCentralExtCmd(cmd) => cext_cmds.lock().unwrap().push(cmd),
    //// Signal::ExecuteCentralExtCmd(cmd) => {
    ////     cext_cmds.push(cmd);
    ////     continue;
    //// }
    //_ => println!("unimplemented distrmsg"),
    //}
    //}
    //});
    //println!("sim_central finished reading cext cmds");
    //// let cc = cext_cmds.lock().unwrap();
    //for (context, cext_cmd) in cext_cmds.lock().unwrap().iter() {
    //// println!("{:?}", cext_cmd);
    ////TODO
    //// cext_cmd.execute(self, &context.ent_uid, &context.comp_uid);
    //}
    //println!("sim_central finished executing cext cmds");

    //self.clock += 1;
    //}
}

impl SimInterfaceStorage for SimCentral {
    fn get_as_string(&self, addr: &Address) -> Option<String> {
        unimplemented!()
    }
    fn get_as_int(&self, addr: &Address) -> Option<i32> {
        unimplemented!()
    }
    fn get_all_as_strings(&self) -> HashMap<String, String, RandomState> {
        unimplemented!()
    }

    fn get_var(&self, addr: &Address) -> Option<Var> {
        unimplemented!()
    }

    fn get_str(&self, addr: &Address) -> Option<&String> {
        unimplemented!()
    }
    fn get_str_mut(&mut self, addr: &Address) -> Option<&mut String> {
        unimplemented!()
    }
    fn get_int(&self, addr: &Address) -> Option<&i32> {
        unimplemented!()
    }
    fn get_int_mut(&mut self, addr: &Address) -> Option<&mut i32> {
        unimplemented!()
    }
    fn get_float(&self, addr: &Address) -> Option<&f32> {
        unimplemented!()
    }
    fn get_float_mut(&mut self, addr: &Address) -> Option<&mut f32> {
        unimplemented!()
    }
    fn get_bool(&self, addr: &Address) -> Option<&bool> {
        unimplemented!()
    }
    fn get_bool_mut(&mut self, addr: &Address) -> Option<&mut bool> {
        unimplemented!()
    }
    fn get_str_list(&self, addr: &Address) -> Option<&Vec<String>> {
        unimplemented!()
    }
    fn get_str_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<String>> {
        unimplemented!()
    }
    fn get_int_list(&self, addr: &Address) -> Option<&Vec<i32>> {
        unimplemented!()
    }
    fn get_int_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<i32>> {
        unimplemented!()
    }
    fn get_float_list(&self, addr: &Address) -> Option<&Vec<f32>> {
        unimplemented!()
    }
    fn get_float_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<f32>> {
        unimplemented!()
    }
    fn get_bool_list(&self, addr: &Address) -> Option<&Vec<bool>> {
        unimplemented!()
    }
    fn get_bool_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<bool>> {
        unimplemented!()
    }
    fn get_str_grid(&self, addr: &Address) -> Option<&Vec<Vec<String>>> {
        unimplemented!()
    }
    fn get_str_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<String>>> {
        unimplemented!()
    }
    fn get_int_grid(&self, addr: &Address) -> Option<&Vec<Vec<i32>>> {
        unimplemented!()
    }
    fn get_int_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<i32>>> {
        unimplemented!()
    }
    fn get_float_grid(&self, addr: &Address) -> Option<&Vec<Vec<f32>>> {
        unimplemented!()
    }
    fn get_float_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<f32>>> {
        unimplemented!()
    }
    fn get_bool_grid(&self, addr: &Address) -> Option<&Vec<Vec<bool>>> {
        unimplemented!()
    }
    fn get_bool_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<bool>>> {
        unimplemented!()
    }

    fn set_from_string(&mut self, addr: &Address, val: &String) -> Result<()> {
        unimplemented!()
    }
    fn set_from_string_list(&mut self, addr: &Address, vec: &Vec<String>) -> Result<()> {
        unimplemented!()
    }
    fn set_from_string_grid(&mut self, addr: &Address, vec2d: &Vec<Vec<String>>) -> Result<()> {
        unimplemented!()
    }
}
