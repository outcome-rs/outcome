use crate::msg::{
    DataTransferResponse, Message, TurnAdvanceRequest, TurnAdvanceResponse, TypedSimDataPack,
};
use crate::server::{handle_data_transfer_request_local, ClientId};
use crate::{Server, SimConnection};

use crate::msg::TransferResponseData::AddressedVar;
use crate::{Error, Result};
use outcome::distr::NodeCommunication;

impl Server {
    // fn advance_turn(&mut self, tick_num: u32) -> Result<()> {}

    pub fn handle_turn_advance_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let req: TurnAdvanceRequest =
            msg.unpack_payload(self.clients.get(client_id).unwrap().connection.encoding())?;

        let mut client_furthest_step = 0;

        let mut no_blocking_clients = true;
        let mut step_before_advance = match &self.sim {
            SimConnection::Local(s) => s.get_clock(),
            SimConnection::UnionOrganizer(c) => c.central.clock,
            SimConnection::UnionWorker(w) => w.sim_node.as_ref().unwrap().clock,
        };

        trace!("step count before advance attempt: {}", step_before_advance);
        let mut common_furthest_step = step_before_advance + 99999;

        if let Some(_client) = self.clients.get_mut(&client_id) {
            trace!(
                "[client_id: {}] current_step: {}, furthest_step: {:?}",
                _client.id,
                step_before_advance,
                _client.furthest_step,
            );

            if _client.furthest_step < step_before_advance {
                _client.furthest_step = step_before_advance;
            }
            if _client.furthest_step - step_before_advance < req.step_count as usize {
                _client.furthest_step = _client.furthest_step + req.step_count as usize;
            }
            client_furthest_step = _client.furthest_step;
        }

        for (id, _client) in &mut self.clients {
            if !_client.is_blocking {
                trace!("omit non-blocking client..");
                continue;
            } else {
                no_blocking_clients = false;
            }
            trace!(
                "client_furthest_tick inside loop: {}",
                _client.furthest_step
            );
            // if _client.furthest_step < current_step {
            //     common_furthest_step = current_step;
            //     break;
            // }
            if _client.furthest_step < common_furthest_step {
                common_furthest_step = _client.furthest_step;
            }
        }

        if no_blocking_clients {
            if let Some(client) = self.clients.get(&client_id) {
                common_furthest_step = client.furthest_step;
            }
        } else {
            match &mut self.sim {
                SimConnection::UnionOrganizer(coord) => {
                    coord.is_blocking_step = true;
                }
                _ => (),
            }
        }

        let mut clock_after_advance = step_before_advance;
        trace!(
            "common_furthest_step: {}, step_before_advance: {}",
            common_furthest_step,
            step_before_advance
        );
        if common_furthest_step > step_before_advance {
            match &mut self.sim {
                SimConnection::Local(sim_instance) => {
                    // for local sim instance simply step until common
                    // furthest step is achieved
                    for _ in 0..common_furthest_step - step_before_advance {
                        sim_instance.step();
                        clock_after_advance += 1;
                        // let events = sim_instance.event_queue.clone();
                        trace!("processed single tick");
                        trace!(
                            "common_furthest_step: {}, step_before_advance: {}",
                            common_furthest_step,
                            step_before_advance
                        );

                        // advanced turn, check if any scheduled transfers/queries need sending
                        for (_, client) in &mut self.clients {
                            for (event, dts_list) in &client.scheduled_transfers.clone() {
                                trace!("handling scheduled data transfer: event: {}", event);
                                if sim_instance.event_queue.contains(&event) {
                                    for dtr in dts_list {
                                        info!("handling scheduled data transfer: dtr: {:?}", dtr);
                                        handle_data_transfer_request_local(
                                            dtr,
                                            sim_instance,
                                            client,
                                        )?
                                    }
                                }
                            }
                            for (event, queries) in &client.scheduled_queries {
                                if sim_instance.event_queue.contains(event) {
                                    for (task_id, query) in queries {
                                        trace!("handling scheduled query: {:?}", query);
                                        let product = query.process(
                                            &sim_instance.entities,
                                            &sim_instance.entity_idx,
                                        )?;

                                        let mut data_pack = TypedSimDataPack::empty();
                                        if let outcome::query::QueryProduct::AddressedVar(map) =
                                            product
                                        {
                                            if let Err(e) =
                                                client.connection.send_payload_with_task(
                                                    DataTransferResponse {
                                                        data: AddressedVar(map),
                                                    },
                                                    *task_id,
                                                    None,
                                                )
                                            {
                                                error!("{}", e);
                                            }
                                        }
                                    }
                                }
                            }

                            if &client.id == client_id {
                                continue;
                            }
                            if let Some(scheduled_step) = client.scheduled_advance_response {
                                trace!(
                                    "[client: {}] scheduled_step: {}, current_step: {}",
                                    client.id,
                                    scheduled_step,
                                    clock_after_advance
                                );
                                if scheduled_step == clock_after_advance {
                                    let resp = TurnAdvanceResponse {
                                        error: String::new(),
                                    };
                                    client.connection.send_payload(resp, None)?;
                                    client.scheduled_advance_response = None;
                                }
                            }
                        }
                    }
                    trace!("clock step after advance: {}", clock_after_advance);
                }
                SimConnection::UnionOrganizer(coord) => {
                    let mut event_queue = coord.central.event_queue.clone();

                    let step_event_name = outcome::string::new_truncate("step");
                    if !event_queue.contains(&step_event_name) {
                        event_queue.push(step_event_name);
                    }
                    coord.central.event_queue.clear();

                    // let network = &coord_lock.network;
                    // let central = &mut coord_lock.central;
                    // for (worker_id, worker) in &coord.net.workers {
                    //     if worker.is_blocking_step {
                    //         let resp = TurnAdvanceResponse {
                    //             error: "BlockedFully".to_string(),
                    //         };
                    //         trace!("BlockedFully");
                    //         let client = self.clients.get_mut(client_id).unwrap();
                    //         client.connection.pack_send_msg_payload(resp, None)?;
                    //         return Ok(());
                    //     }
                    // }
                    coord.central.step_network(&mut coord.net, event_queue);
                    // coord_lock
                    //     .central
                    //     .step_network(&mut coord_lock.network, event_queue)?;
                    coord.central.clock += 1;

                    // let mut addr_book = HashMap::new();
                    // for node in &coord.nodes {
                    //     addr_book.insert(node.id.clone(), node.connection.try_clone().unwrap());
                    // }
                    //coord.main.step(&coord.entity_node_map, &mut addr_book);
                }
                SimConnection::UnionWorker(worker) => {
                    // worker can't initiate step processing on it's own, it
                    // has to signal to the coordinator

                    // first let them know the worker is ready
                    worker
                        .network
                        .sig_send_central(0, outcome::distr::Signal::WorkerReady)?;

                    // request coordinator to step cluster forward

                    // id is attached to the request so that when incoming signals are
                    // read on the worker, it knows what the response is related to
                    worker.network.sig_send_central(
                        1220,
                        outcome::distr::Signal::WorkerStepAdvanceRequest(
                            (common_furthest_step - step_before_advance) as u32,
                        ),
                    )?;
                    // once coordinator receives this request, it will store that
                    // particular worker's furthest step, similar to how this server
                    // is doing for it's clients
                    //
                    // coordinator evaluates readiness of all other workers, and if all
                    // are ready it will initiate a new step
                    //
                    // if other workers are not ready, coordinator response to this
                    // message will be delayed
                }
            };
        } else {
            match &mut self.sim {
                SimConnection::UnionOrganizer(coord) => {
                    coord.is_blocking_step = true;
                }
                _ => (),
            }
        }

        let client = self.clients.get_mut(client_id).unwrap();

        // clock wasn't moved
        // if common_furthest_step == current_step {
        if common_furthest_step == step_before_advance {
            trace!("BlockedFully");
            // client.scheduled_advance_response = Some(client.)
            // immediate response requested
            if !req.wait {
                let resp = TurnAdvanceResponse {
                    error: "BlockedFully".to_string(),
                };
                client.connection.send_payload(resp, None)?;
            } else {
                client.scheduled_advance_response = Some(client.furthest_step);
            }
        } else if common_furthest_step < client_furthest_step {
            trace!("BlockedPartially");
            if !req.wait {
                let resp = TurnAdvanceResponse {
                    error: "BlockedPartially".to_string(),
                };
                client.connection.send_payload(resp, None)?;
            } else {
                client.scheduled_advance_response = Some(client.furthest_step);
            }
            //        } else if common_furthest_tick == client_furthest_tick {
        } else {
            trace!("Didn't block");
            let resp = TurnAdvanceResponse {
                error: String::new(),
            };
            client.connection.send_payload(resp, None)?;
        }

        // // check the clients for scheduled step advance responses
        // for (_, client) in &mut self.clients {
        //     if &client.id == client_id {
        //         continue;
        //     }
        //     if let Some(scheduled_step) = client.scheduled_advance_response {
        //         warn!:
        //             "[client: {}] scheduled_step: {}, current_step: {}",
        //             client.id, scheduled_step, clock_after_advance
        //         );
        //         if scheduled_step == clock_after_advance {
        //             let resp = TurnAdvanceResponse {
        //                 error: String::new(),
        //             };
        //             client.connection.send_payload(resp, None)?;
        //             client.scheduled_advance_response = None;
        //         }
        //     }
        // }

        Ok(())
    }
}
