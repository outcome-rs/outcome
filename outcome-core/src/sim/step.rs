//! Step processing functions for the `Sim` struct.

use std::sync::{Arc, Mutex};

use crate::entity::Entity;
use crate::error::Error;
use crate::{EntityId, EntityUid, SimModel, StringId};

#[cfg(feature = "machine")]
use crate::machine::{cmd::CentralExtCommand, cmd::ExtCommand, exec, ExecutionContext};
#[cfg(feature = "machine")]
use rayon::prelude::*;

use super::Sim;

/// Single step processing functions.
impl Sim {
    /// Performs single simulation step, utilizing multi-threading.
    ///
    /// # Process description
    ///
    /// This function uses a parallel iterator to iterate over all entities.
    /// Each entity-owning thread then makes a list of components to process
    /// using entity's component queue to find matches based on the triggered
    /// events.
    ///
    /// For each processed component, current state value is found.
    /// Logic processing utility function is used to process component
    /// commands. Once parallel iteration over entities is done, last thing
    /// to do is executing external and central-external commands that have
    /// been accumulated during parallel iteration stage.
    pub fn step(&mut self) -> Result<(), Error> {
        // clone event queue into a local variable
        let mut event_queue = self.event_queue.clone();

        let arrstr_step = StringId::from_unchecked("step");
        if !event_queue.contains(&arrstr_step) {
            event_queue.push(arrstr_step);
        }
        self.event_queue.clear();

        #[cfg(feature = "machine")]
        {
            let model = &self.model;

            // declare atomic vecs for ext and central-ext commands
            let ext_cmds: Arc<Mutex<Vec<(ExecutionContext, ExtCommand)>>> =
                Arc::new(Mutex::new(Vec::new()));
            let central_ext_cmds: Arc<Mutex<Vec<(ExecutionContext, CentralExtCommand)>>> =
                Arc::new(Mutex::new(Vec::new()));

            // loc phase
            self.entities.par_iter_mut().for_each(
                |(ent_uid, mut entity): (&EntityUid, &mut Entity)| {
                    step_entity_local(
                        model,
                        &event_queue,
                        ent_uid,
                        entity,
                        &ext_cmds,
                        &central_ext_cmds,
                    );
                },
            );

            // post phase
            exec::execute_ext(&ext_cmds.lock().unwrap(), self)?;
            exec::execute_central_ext(&central_ext_cmds.lock().unwrap(), self)?;
        }

        // let arrstr_step = StringId::from_unchecked("step");
        // if !event_queue.contains(&arrstr_step) {
        //     event_queue.push(arrstr_step);
        // }
        // self.event_queue.clear();
        // self.event_queue = event_queue;

        self.clock += 1;

        if !self.event_queue.contains(&arrstr_step) {
            self.event_queue.push(arrstr_step);
        }

        Ok(())
    }
}

#[cfg(feature = "machine")]
pub(crate) fn step_entity_local(
    model: &SimModel,
    event_queue: &Vec<StringId>,
    ent_uid: &EntityUid,
    mut entity: &mut Entity,
    ext_cmds: &Arc<Mutex<Vec<(ExecutionContext, ExtCommand)>>>,
    central_ext_cmds: &Arc<Mutex<Vec<(ExecutionContext, CentralExtCommand)>>>,
) -> Result<(), Error> {
    for event in event_queue {
        if let Some(event_queue) = entity.comp_queue.get(event) {
            for comp_uid in event_queue {
                if let Some(comp_state) = entity.comp_state.get_mut(comp_uid) {
                    // let comp_curr_state = &comp.current_state;
                    if comp_state.as_ref() == "idle" {
                        continue;
                    }
                    if let Some(comp_model) = model.get_component(comp_uid) {
                        let (start, end) = match comp_model.logic.states.get(comp_state) {
                            Some((s, e)) => (Some(*s), Some(*e)),
                            None => continue,
                        };
                        crate::machine::exec::execute_loc(
                            &comp_model.logic.commands,
                            &mut entity.storage,
                            &mut entity.insta,
                            comp_state,
                            //TODO
                            &EntityId::new(),
                            &comp_uid,
                            &model,
                            &ext_cmds,
                            &central_ext_cmds,
                            start,
                            end,
                        )?;
                    }
                }
            }
        } else {
            //TODO err
            return Ok(());
        }
        // for (comp_uid, mut comp) in &mut entity.components.map {
        // for comp_uid in entity.components.map.keys()
        //     .map(|c| *c).collect::<Vec<(ShortString, ShortString)>>().iter() {

        // debug!("inside entity: {:?}, processing comp from queue, id: {:?}", &ent_uid, &comp_uid);
        // let mut comp = match entity.components.get_mut(&comp_uid) {
        //     Some(comp) => comp,
        //     None => {
        //         let (comp_type, comp_id) = &comp_uid;
        //         debug!("failed getting component: {}/{} (perhaps it was recently detached?)",
        //                comp_type.as_str(), comp_id.as_str());
        //         continue;
        //     }
        // };

        // let (mut start, mut end) = (None, None);
        // if !comp_model.logic.states.is_empty() {
        //     let (s, e) =
        //         match &comp_model.logic.states.get(comp_curr_state.as_str()) {
        //             Some(se) => se,
        //             None => continue,
        //         };
        //     start = Some(*s);
        //     end = Some(*e);
        // }
        // }

        // remove selected components from ent event queues
        // for r in to_remove_from_ent_queue {
        //     let (n, _) = entity
        //         .components
        //         .queue
        //         .get(event)
        //         .unwrap()
        //         .iter()
        //         .enumerate()
        //         .find(|(n, puid)| **puid == r)
        //         .unwrap();
        //     entity.components.queue.get_mut(event).unwrap().remove(n);
        // }
    }

    Ok(())
}
