use crate::msg::{
    DataTransferResponse, Message, TurnAdvanceRequest, TurnAdvanceResponse, TypedSimDataPack,
};
use crate::server::{handle_data_transfer_request_local, ClientId};
use crate::{Server, SimConnection};

use crate::msg::TransferResponseData::AddressedVar;
use crate::{Error, Result};
use outcome::distr::NodeCommunication;

impl Server {
    pub fn handle_turn_advance_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let req: TurnAdvanceRequest =
            msg.unpack_payload(self.clients.get(client_id).unwrap().connection.encoding())?;

        let mut client_furthest_tick = 0;

        let mut no_blocking_clients = true;
        let current_tick = match &self.sim {
            SimConnection::Local(s) => s.get_clock(),
            SimConnection::ClusterCoord(c) => c.central.clock,
            SimConnection::ClusterWorker(w) => w.sim_node.as_ref().unwrap().clock,
        };
        trace!("current_tick before: {}", current_tick);
        let mut common_furthest_tick = current_tick + 99999;
        for (id, _client) in &mut self.clients {
            if _client.id == *client_id {
                trace!(
                    "({}) furthest_tick: {}, current_tick: {}",
                    _client.id,
                    _client.furthest_step,
                    current_tick
                );
                if _client.furthest_step < current_tick {
                    _client.furthest_step = current_tick;
                }
                if _client.furthest_step - current_tick < req.tick_count as usize {
                    _client.furthest_step = _client.furthest_step + req.tick_count as usize;
                }
                client_furthest_tick = _client.furthest_step.clone();
            }
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
            if _client.furthest_step == current_tick {
                common_furthest_tick = current_tick;
                break;
            }
            if _client.furthest_step < common_furthest_tick {
                common_furthest_tick = _client.furthest_step;
            }
        }
        if no_blocking_clients {
            let t = self.clients.get(&client_id).unwrap().furthest_step;
            common_furthest_tick = t;
        } else {
            match &mut self.sim {
                SimConnection::ClusterCoord(coord) => {
                    coord.is_blocking_step = true;
                }
                _ => (),
            }
        }

        trace!("common_furthest_tick: {}", common_furthest_tick);
        if common_furthest_tick > current_tick {
            match &mut self.sim {
                SimConnection::Local(sim_instance) => {
                    // for local sim instance simply step until common
                    // furthest step is achieved
                    for _ in 0..common_furthest_tick - current_tick {
                        sim_instance.step();
                        // let events = sim_instance.event_queue.clone();
                        trace!("processed single tick");
                        trace!(
                            "common_furthest_tick: {}, current_tick: {}",
                            common_furthest_tick,
                            current_tick
                        );

                        // advanced turn, check if any scheduled transfers/queries need sending
                        for (_, client) in &mut self.clients {
                            for (event, dts_list) in &client.scheduled_dts.clone() {
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
                        }
                    }
                    trace!("current_tick after: {}", sim_instance.get_clock());
                }
                SimConnection::ClusterCoord(coord) => {
                    let mut event_queue = coord.central.event_queue.clone();

                    let step_event_name = outcome::arraystring::new_unchecked("step");
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
                SimConnection::ClusterWorker(worker) => {
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
                            (common_furthest_tick - current_tick) as u32,
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
                SimConnection::ClusterCoord(coord) => {
                    coord.is_blocking_step = true;
                }
                _ => (),
            }
        }

        let client = self.clients.get_mut(client_id).unwrap();

        // responses
        if common_furthest_tick == current_tick {
            let resp = TurnAdvanceResponse {
                error: "BlockedFully".to_string(),
            };
            trace!("BlockedFully");
            client.connection.send_payload(resp, None)?;
        } else if common_furthest_tick < client_furthest_tick {
            let resp = TurnAdvanceResponse {
                error: "BlockedPartially".to_string(),
            };
            trace!("BlockedPartially");
            client.connection.send_payload(resp, None)?;
            //        } else if common_furthest_tick == client_furthest_tick {
        } else {
            let resp = TurnAdvanceResponse {
                error: String::new(),
            };
            trace!("Didn't block");
            client.connection.send_payload(resp, None)?;
        }

        Ok(())
    }
}
