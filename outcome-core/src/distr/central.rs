//! Central authority definition.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rand::prelude::SliceRandom;

use crate::distr::{
    CentralCommunication, DistributionPolicy, NodeCommunication, NodeId, Signal, TaskId,
};
use crate::error::{Error, Result};
use crate::model::Scenario;
use crate::{
    arraystring, Address, EntityId, EntityName, EventName, PrefabName, ShortString, SimModel,
    StringId, Var,
};

#[cfg(feature = "machine")]
use crate::machine::{cmd::CentralRemoteCommand, cmd::Command, cmd::ExtCommand, ExecutionContext};
#[cfg(feature = "machine")]
use rayon::prelude::*;

use crate::entity::Entity;
use crate::snapshot::Snapshot;
use fnv::FnvHashMap;
use id_pool::IdPool;

/// Distributed simulation central authority. Does the necessary coordination
/// work for distributed sim instances.
///
/// It holds the main simulation model object, the current clock and the
/// current event queue, as well as a list of entities.
/// It doesn't hold any entity data.
///
/// Some of its tasks include:
/// - executing central commands that require global authority, for example
/// related to mutating the sim model or invoking events
/// - load balancing, distribution of entities between nodes
#[derive(Serialize, Deserialize)]
pub struct SimCentral {
    pub model: SimModel,
    pub clock: usize,
    pub event_queue: Vec<EventName>,

    /// Default distribution policy for entities. Note that entities can be
    /// assigned custom individual policies that override it.
    pub distribution_policy: DistributionPolicy,

    pub node_entities: FnvHashMap<NodeId, Vec<EntityId>>,
    // pub entity_node_routes: FnvHashMap<>
    pub entities_idx: FnvHashMap<EntityName, EntityId>,
    entity_idpool: IdPool,

    ent_spawn_queue: FnvHashMap<NodeId, Vec<(EntityId, Option<PrefabName>, Option<EntityName>)>>,
    pub model_changes_queue: SimModel,
}

impl SimCentral {
    /// Gets the current value of the globally synchronized simulation clock.
    pub fn get_clock(&self) -> usize {
        self.clock
    }

    /// Flushes the communication queue, lumping requests of the same type
    /// together if possible.
    pub fn flush_queue<C: CentralCommunication>(&mut self, comms: &mut C) -> Result<()> {
        if !self.ent_spawn_queue.is_empty() {
            for (k, v) in &self.ent_spawn_queue {
                warn!("node: {:?}, spawn: {:?}", k, v);
                comms.send_sig_to_node(*k, 0, Signal::SpawnEntities(v.clone()))?;
            }
            self.ent_spawn_queue.clear();
        }

        Ok(())
    }

    /// Creates a new `SimCentral` using a model object.
    pub fn from_model(model: SimModel) -> Result<SimCentral> {
        let mut event_queue = vec![arraystring::new_truncate("step")];
        let mut sim_central = SimCentral {
            model: model.clone(),
            clock: 0,
            event_queue,
            distribution_policy: DistributionPolicy::Random,
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

impl SimCentral {
    /// Spawns a new entity.
    pub fn spawn_entity(
        &mut self,
        prefab: Option<StringId>,
        name: Option<StringId>,
        policy: DistributionPolicy,
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
            DistributionPolicy::BindToNode(node_id) => {
                if !self.ent_spawn_queue.contains_key(&node_id) {
                    self.ent_spawn_queue.insert(node_id, Vec::new());
                }
                self.ent_spawn_queue
                    .get_mut(&node_id)
                    .unwrap()
                    .push((new_uid, prefab, name));
            }
            // TODO
            DistributionPolicy::Random => {
                if self.node_entities.is_empty() {
                    return Err(Error::Other("no nodes available".to_string()));
                }

                // shuffle the existing node ids and draw one
                let mut nums: Vec<&u32> = self.node_entities.keys().collect::<Vec<&u32>>();
                warn!("nodes: {:?}", nums);
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
        // while self.ent_spawn_queue
        // for (n, v) in &self.ent_spawn_queue {
        //     net.send_sig_to_node(*n, Signal::SpawnEntities(v.clone()));
        // }

        Ok(())
    }

    pub fn assign_entities(
        &self,
        node_count: usize,
        policy: DistributionPolicy,
    ) -> Vec<Vec<EntityId>> {
        match policy {
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

    /// Processes a single simulation step.
    ///
    /// Uses a reference to a network object that implements
    /// `CentralCommunication` for all the network communication needs.
    ///
    /// # Protocol overview
    ///
    /// 1. All nodes are signalled to start processing next step.
    /// 2. Nodes send back central remote commands that came up during their
    /// local processing, if any.
    /// 3. Incoming central remote commands are executed and results are sent
    /// back. Any model changes are also sent to the nodes.
    /// 4. Nodes signal their readiness to move on to the next step.
    pub fn step_network<N: CentralCommunication>(
        &mut self,
        network: &mut N,
        event_queue: Vec<StringId>,
    ) -> Result<()> {
        debug!("starting processing step");

        // tell nodes to start processing next step
        network.broadcast_sig(0, Signal::StartProcessStep(event_queue))?;
        debug!("sent `StartProcessStep` signal to all nodes");

        debug!("starting reading incoming signals");
        #[cfg(feature = "machine")]
        let mut cext_cmds: Arc<Mutex<Vec<(ExecutionContext, CentralRemoteCommand)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let mut do_nodes = network.get_node_ids()?;
        let mut node_counter = 0;
        while !do_nodes.is_empty() {
            let node = do_nodes.get(node_counter).unwrap();
            match network.try_recv_sig_from(*node) {
                Ok((task_id, signal)) => match signal {
                    #[cfg(feature = "machine")]
                    Signal::ExecuteCentralExtCmd(cmd) => cext_cmds.lock().unwrap().push(cmd),
                    #[cfg(feature = "machine")]
                    Signal::ExecuteCentralExtCmds(cmds) => cext_cmds.lock().unwrap().extend(cmds),
                    Signal::EndOfMessages | Signal::ProcessStepFinished => {
                        do_nodes.remove(node_counter);
                    }
                    _ => debug!("unimplemented: received signal: {:?}", signal),
                },
                Err(e) => match e {
                    Error::WouldBlock => continue,
                    _ => error!("{:?}", e),
                },
            };
            node_counter += 1;
            if node_counter >= do_nodes.len() {
                node_counter = 0;
            }
        }
        debug!("finished reading incoming signals");

        debug!("starting processing cext commands");
        #[cfg(feature = "machine")]
        for (context, cext_cmd) in cext_cmds.lock().unwrap().iter() {
            warn!("{:?}", cext_cmd);
            //TODO
            cext_cmd.execute_distr(self, &context.ent, &context.comp);
        }
        network.broadcast_sig(0, Signal::UpdateModel(self.model.clone()));
        self.flush_queue(network)?;

        network.broadcast_sig(0, Signal::EndOfMessages)?;
        // network.sig_broadcast(Signal::EndOfMessages)?;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(8));
            if let Ok((_, _, s)) = network.try_recv_sig() {
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

    pub fn init_snapshot_download<N: CentralCommunication>(
        &mut self,
        network: &mut N,
    ) -> Result<TaskId> {
        debug!("starting downloading snapshot");

        let task_id = network.request_task_id()?;
        // tell nodes to send their snapshots
        network.broadcast_sig(task_id, Signal::SnapshotRequest)?;
        debug!("sent `StartProcessStep` signal to all nodes");

        Ok(task_id)
    }
}
