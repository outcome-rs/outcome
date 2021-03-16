//! Node definition.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fnv::FnvHashMap;

use crate::distr::{NodeCommunication, Signal};
use crate::entity::Entity;
use crate::sim::step;
use crate::{CompName, Result};
use crate::{EntityId, EntityName, SimModel, StringId};

#[cfg(feature = "machine")]
use rayon::prelude::*;

/// Distributed simulation node.
///
/// It holds the current clock value, a full copy of the sim model, and
/// a subset of all the sim entities.
///
/// Implementation of `SimNode` itself doesn't provide a mechanism for
/// communication between different nodes. It includes custom processing
/// functions that can be used by a higher level coordinator which will
/// provide it's own connection functionality.
#[derive(Serialize, Deserialize)]
pub struct SimNode {
    pub clock: usize,
    pub model: SimModel,
    pub event_queue: Vec<StringId>,
    pub entities: FnvHashMap<EntityId, Entity>,
    pub entities_idx: FnvHashMap<EntityName, EntityId>,
}

impl SimNode {
    /// Creates a new node using the sim model and a list of entities.
    pub fn from_model(model: &SimModel) -> Result<SimNode> {
        let mut sim_node = SimNode {
            clock: 0,
            model: model.clone(),
            entities: FnvHashMap::default(),
            entities_idx: FnvHashMap::default(),
            event_queue: vec![StringId::from("_scr_init").unwrap()],
        };

        // sim_node.apply_model_entities(entities);

        // let ent_uid = (
        //     StringId::from("singleton").unwrap(),
        //     StringId::from("0").unwrap(),
        // );
        // let comp_uid = (
        //     StringId::from("mod_init").unwrap(),
        //     StringId::from("0").unwrap(),
        // );
        // let commands = sim
        //     .model
        //     .get_component(
        //         &IndexString::from_str_truncate("singleton"),
        //         &IndexString::from_str_truncate("mod_init"),
        //         &IndexString::from_str_truncate("0"),
        //     )
        //     .unwrap()
        //     .logic
        //     .commands
        //     .clone();
        // exec::execute(&commands, &ent_uid, &comp_uid, &mut sim, None, None);

        Ok(sim_node)
    }

    pub fn add_entity(
        &mut self,
        uid: EntityId,
        prefab_id: Option<EntityName>,
        target_id: Option<EntityName>,
    ) -> Result<()> {
        let entity = match &prefab_id {
            Some(p) => Entity::from_prefab_name(p, &self.model)?,
            None => Entity::empty(),
        };

        warn!("{:?}", entity);

        self.entities.insert(uid, entity);

        if let Some(t) = target_id {
            self.entities_idx.insert(t, uid);
        }

        Ok(())
    }

    /// Apply registered model entities by instantiating them.
    /// None of the existing entities are removed. Only entities
    /// registered with the `spawn` flag are instantiated.
    pub fn apply_model_entities(&mut self, selection: &Vec<EntityId>) {
        trace!("start adding entities");
        unimplemented!();
        // let mut counter = 0;
        // for ent_uid in selection {
        //     let entity = self.model.get_entity(ent_uid).unwrap();
        //     // don't instantiate already existing entities that match the key
        //     if self.entities_idx.contains_key(ent_uid) {
        //         continue;
        //     }
        //     if entity.spawn == true {
        //         // create a new entity
        //         let mut ent = match Entity::from_prefab(&entity, &self.model) {
        //             Some(e) => e,
        //             None => break,
        //         };
        //         self.entities.insert(*ent_uid, ent);
        //         counter = counter + 1;
        //     }
        // }
        // trace!("finished adding entities ({})", counter);
    }

    #[cfg(not(feature = "machine"))]
    pub fn step<N: NodeCommunication>(
        &mut self,
        mut network: &mut N,
        event_queue: &Vec<StringId>,
    ) -> Result<()> {
        Ok(())
    }

    /// Process single step.
    ///
    /// ### Arguments
    ///
    /// `entity_node_map` is a map of all the sim entities as keys, each with
    /// value containing the id of the target node.
    ///
    /// `addr_book` is a map of nodes and their connections
    #[cfg(feature = "machine")]
    pub fn step<N: NodeCommunication>(
        &mut self,
        mut network: &mut N,
        event_queue: &Vec<StringId>,
    ) -> Result<()> {
        use crate::machine::cmd::{CentralRemoteCommand, ExtCommand};
        use crate::machine::{cmd, ExecutionContext};
        trace!(
            "sim_node start processing step, event queue: {:?}",
            event_queue
        );

        // // clone event queue into a local variable
        // let mut event_queue = self.event_queue.clone();
        //
        // let arrstr_step = StringId::from_unchecked("step");
        // if !event_queue.contains(&arrstr_step) {
        //     event_queue.push(arrstr_step);
        // }
        // self.event_queue.clear();

        let model = &self.model;
        // let event_queue = &self.event_queue;

        // declare sync vecs for external and central-external
        let ext_cmds: Arc<Mutex<Vec<(ExecutionContext, ExtCommand)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let central_ext_cmds: Arc<Mutex<Vec<(ExecutionContext, CentralRemoteCommand)>>> =
            Arc::new(Mutex::new(Vec::new()));

        // loc phase
        self.entities
            .par_iter_mut()
            .for_each(|(ent_uid, mut entity): (&EntityId, &mut Entity)| {
                trace!("processing entity: {:?}", entity);
                step::step_entity_local(
                    model,
                    &event_queue,
                    ent_uid,
                    entity,
                    &ext_cmds,
                    &central_ext_cmds,
                );
            });
        trace!("sim_node finished local phase");

        // // send ext cmd requests
        // for (exec_context, ext_cmd) in ext_cmds.lock().unwrap().iter() {
        //     println!("sending ext_cmd: {:?}", ext_cmd);
        //     // let ent_uid_string = format!("{}/{}", ent_type, ent_id);
        //     let target_ent = match ext_cmd {
        //         ExtCommand::SetVar(esv) => esv.target.entity,
        //         _ => continue,
        //     };
        //     // let target_node_id = entity_node_map.get(&target_ent).unwrap();
        //     // unimplemented!();
        //     //addr_book.get(target_node_id).unwrap().send_request(ext_cmd);
        // }
        //
        // println!("sim_node finished send ext cmd requests");

        // `post` phase
        // process queued external msgs from `loc`
        // regular ext msgs go again to peer nodes
        // central ext msgs go to the main auth
        //        addr_book.iter().for_each(|(node,c)| {
        //
        //        });

        // addr_book
        //     .par_iter_mut()
        //     .for_each(|(node, c): (&u32, &mut C)| {
        //         //            thread::spawn(|| {
        //         loop {
        //             println!("enter loop, wait for read_ext_cmd...");
        //             // TODO there should be protocol for ending of stream of these messages
        //             //
        //             // match c.read_message()
        //             // match c.read_ext_cmd() {
        //             //     Ok((context, cmd)) => {
        //             //         println!("exec ext command received from {:?}", context)
        //             //     }
        //             //     _ => return,
        //             // }
        //             return;
        //         }
        //     });
        // println!("sim_node finished read ext cmd responses");

        let mut cexts = central_ext_cmds.lock().unwrap().clone();
        cexts.reverse();
        let mut counter = 0;
        let mut cexts_part = Vec::new();
        loop {
            if let Some(cmd) = cexts.pop() {
                counter += 1;
                cexts_part.push(cmd);
            } else {
                if !cexts_part.is_empty() {
                    network.sig_send_central(Signal::ExecuteCentralExtCmds(cexts_part.clone()));
                }
                break;
            }

            if counter >= 1000 {
                counter = 0;
                network.sig_send_central(Signal::ExecuteCentralExtCmds(cexts_part.clone()));
                cexts_part.clear();
            }
        }
        // for cext in cexts {
        //     // println!("sending cext cmd: {:?}", cext);
        //     network.sig_send_central(Signal::ExecuteCentralExtCmd(cext));
        // }
        // network.sig_send_central(Signal::ExecuteCentralExtCmds(cexts));
        network.sig_send_central(Signal::EndOfMessages);
        loop {
            // std::thread::sleep(std::time::Duration::from_millis(8));
            match network.sig_read_central()? {
                Signal::SpawnEntities(e) => {
                    warn!("signal: spawn entities: {:?}", e);
                    for (a, b, c) in e {
                        self.add_entity(a, b, c)?;
                    }
                    info!("spawn entities finished");
                }
                // TODO currently rewrites the whole model with the received data
                Signal::UpdateModel(model) => {
                    info!("signal: update model");
                    self.model = model;
                    info!("update model finished");
                }
                Signal::EndOfMessages => {
                    info!("signal: end of messages, breaking loop");
                    break;
                }
                _ => (),
            }
        }
        info!("sending signal process step finished");
        network.sig_send_central(Signal::ProcessStepFinished);
        trace!("sim_node finished send central ext cmd requests");

        // println!("{:?}", self.entities);
        Ok(())
    }

    //fn exec_ext_get(&self, get: cmd::get_set::Get) {}

    /// Serialize, send over and locally remove selected
    /// entities.
    pub fn transfer_entities() {}
    /// Receive and deserialize selected entities, then push
    /// them to the main entity list.
    pub fn receive_entities() {}
}
