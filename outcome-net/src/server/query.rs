use std::convert::TryInto;

use outcome::distr::{CentralCommunication, Signal};

use crate::msg::{
    DataTransferResponse, Message, NativeQueryRequest, NativeQueryResponse, QueryRequest,
    TransferResponseData,
};
use crate::organizer::OrganizerTask;
use crate::server::{ClientId, ServerTask};
use crate::{Error, Result};
use crate::{Server, SimConnection};

impl Server {
    pub fn handle_query_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        let qr: QueryRequest = msg.unpack_payload(client.connection.encoding())?;

        match &mut self.sim {
            SimConnection::Local(sim) => {
                let query: outcome::query::Query = qr.query.try_into()?;

                if let outcome::query::Trigger::Event(event_name) = &query.trigger {
                    client.push_event_triggered_query(*event_name, msg.task_id, query)?;
                } else if let outcome::query::Trigger::Mutation(address) = query.trigger {
                    unimplemented!()
                } else {
                    // let insta = std::time::Instant::now();
                    let product = query.process(&sim.entities, &sim.entity_idx)?;
                    // println!(
                    //     "processing query took: {} ms",
                    //     Instant::now().duration_since(insta).as_millis()
                    // );
                    // let mut data_pack = SimDataPack::empty();
                    println!("product: {:?}", product);
                    if let outcome::query::QueryProduct::AddressedVar(map) = product {
                        client.connection.send_payload_with_task(
                            DataTransferResponse {
                                data: TransferResponseData::AddressedVar(map),
                            },
                            msg.task_id,
                            None,
                        )?;
                    }
                    // println!("msg taskid: {}", msg.task_id);
                }
            }
            SimConnection::UnionOrganizer(coord) => {
                // TODO real query
                let query = outcome::Query {
                    trigger: outcome::query::Trigger::Event(
                        outcome::EventName::from("step").unwrap(),
                    ),
                    description: outcome::query::Description::Addressed,
                    layout: outcome::query::Layout::Typed,
                    filters: vec![outcome::query::Filter::AllComponents(vec![
                        outcome::CompName::from("transform").unwrap(),
                    ])],
                    mappings: vec![outcome::query::Map::All],
                };

                let task_id = coord.register_task(OrganizerTask::WaitForQueryResponses {
                    remaining: coord.net.workers.len() as u32,
                    products: vec![],
                })?;
                self.tasks
                    .insert(task_id, ServerTask::WaitForCoordQueryResponse(*client_id));
                coord
                    .net
                    .broadcast_sig(task_id, Signal::QueryRequest(query))?;
            }

            SimConnection::UnionWorker(worker) => {
                // // check if query wants local entities only
                // if query.filters.contains(&outcome::query::Filter::Node(0)) {}
            }
        }

        Ok(())
    }

    pub fn handle_native_query_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        let qr: NativeQueryRequest = msg.unpack_payload(client.connection.encoding())?;

        match &mut self.sim {
            SimConnection::Local(sim) => {
                let product = qr.query.process(&sim.entities, &sim.entity_idx)?;
                client.connection.send_payload(
                    NativeQueryResponse {
                        query_product: product,
                        error: None,
                    },
                    None,
                )?;
            }
            SimConnection::UnionOrganizer(ref mut coord) => {
                coord.net.broadcast_sig(0, Signal::DataRequestAll);
                // coord.net.
                // TODO
            }
            SimConnection::UnionWorker(worker) => {
                if let Some(node) = &worker.sim_node {
                    let product = qr.query.process(&node.entities, &node.entities_idx)?;
                    client.connection.send_payload(
                        NativeQueryResponse {
                            query_product: product,
                            error: None,
                        },
                        None,
                    )?;
                }
            }
        }
        Ok(())
    }
}
