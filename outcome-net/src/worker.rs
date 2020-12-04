#![allow(dead_code)]

extern crate outcome_core as outcome;
extern crate rmp_serde as rmps;
extern crate serde;

use std::io::prelude::*;
use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::{io, thread};

use self::rmps::{Deserializer, Serializer};
use self::serde::{Deserialize, Serialize};

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;

use outcome::Sim;

use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordRequest,
    IntroduceWorkerToCoordResponse,
};
use crate::msg::*;
use crate::transport::{SocketInterface, WorkerDriverInterface};
use crate::{error::Error, sig, Result};
use crate::{tcp_endpoint, WorkerDriver};

use outcome_core::distr::{NodeCommunication, Signal, SimNode};
use outcome_core::{Address, CompId, EntityId, EntityUid, SimModel, StringId, Var, VarType};

//TODO remove this
/// Default address for the worker
pub const WORKER_ADDRESS: &str = "0.0.0.0:5922";

/// Network-unique identifier for a single worker
pub type WorkerId = u32;

pub struct WorkerNetwork {
    /// IP address of the worker
    pub address: String,
    /// Network driver
    driver: WorkerDriver,
}

/// Represents a single cluster node, connected to and controlled by
/// the cluster coordinator. `Worker`s are also connected to each other, either
/// directly or not, depending on network topology used.
///
/// # Usage details
///
/// In a simulation cluster made up of multiple machines, there is at least
/// one `Worker` running on each machine.
///
/// In terms of initialization, `Worker`s can be either actively reach out to
/// an already existing cluster to join in, or passively wait for incoming
/// connection from a coordinator.
///
/// Unless configured otherwise, new `Worker`s can dynamically join into
/// already initialized clusters, allowing for on-the-fly changes to the
/// cluster composition.
///
/// # Discussion
///
/// Worker abstraction could work well with "thread per core" strategy. This
/// means there would be a single worker per every machine core, instead of
/// single worker per machine utilizing multiple cores with thread-pooling.
/// "Thread per core" promises performance improvements caused by reducing
/// expensive context switching operations. It would require having the ability
/// to switch `SimNode`s to process entities in a single-threaded fashion.
pub struct Worker {
    /// List of other workers in the cluster
    pub comrades: Vec<Comrade>,
    pub network: WorkerNetwork,
    /// Whether the worker uses a password to authorize connecting comrade workers
    pub use_auth: bool,
    /// Password used for incoming connection authorization
    pub passwd_list: Vec<String>,

    /// Simulation node running on this worker
    pub sim_node: Option<outcome::distr::SimNode>,
}

impl Worker {
    /// Creates a new `Worker`.
    pub fn new(my_addr: &str) -> Result<Worker> {
        Ok(Worker {
            comrades: vec![],
            network: WorkerNetwork {
                address: "".to_string(),
                driver: WorkerDriver::new(my_addr).unwrap(),
            },
            // driver: WorkerDriver::new(my_addr).unwrap(),
            use_auth: false,
            passwd_list: vec![],
            sim_node: None,
        })
    }

    /// Registers a fellow worker.
    pub fn register_comrade(&mut self, comrade: Comrade) -> Result<()> {
        if self.use_auth {
            if !&self.passwd_list.contains(&comrade.passwd) {
                println!("Client provided wrong password");
                return Err(Error::Other(String::from("WrongPasswd")));
            }
            self.comrades.push(comrade);
        } else {
            self.comrades.push(comrade);
        }
        return Ok(());
    }

    pub fn initiate_coord_connection(&mut self, addr: &str, timeout: Duration) -> Result<()> {
        let req_msg = Message::from_payload(
            IntroduceWorkerToCoordRequest {
                worker_addr: self.network.driver.my_addr.clone(),
                //TODO
                worker_passwd: "".to_string(),
            },
            false,
        )?;
        self.network.driver.inviter.connect(&tcp_endpoint(addr))?;
        thread::sleep(Duration::from_millis(100));
        self.network.driver.inviter.send_msg(req_msg)?;

        let resp: IntroduceWorkerToCoordResponse = self
            .network
            .driver
            .inviter
            .try_read_msg(Some(timeout.as_millis() as u32))?
            .unpack_payload()?;

        self.network.driver.inviter.disconnect("")?;
        Ok(())
    }

    // TODO
    /// Handles initial connection from the cluster coordinator.
    pub fn handle_coordinator(&mut self) -> Result<()> {
        print!("Waiting for message from coordinator... ");
        std::io::stdout().flush()?;
        let msg = self.network.driver.accept()?;
        println!("success");

        debug!("message from coordinator: {:?}", msg);

        let req: IntroduceCoordRequest = msg.unpack_payload()?;

        print!(
            "Coordinator announced itself as {}, with {}",
            req.ip_addr,
            match req.passwd.as_str() {
                "" => "no password".to_string(),
                s => format!("the following password: {}", s),
            }
        );
        print!("... ");
        std::io::stdout().flush()?;

        // TODO check password

        println!("accepted");

        let resp = Message::from_payload(
            IntroduceCoordResponse {
                error: "".to_string(),
            },
            false,
        )?;
        self.network.driver.greeter.send_msg(resp)?;

        self.network
            .driver
            .coord
            .bind(&format!("{}6", self.network.driver.my_addr))?;
        self.network.driver.coord.connect(&req.ip_addr)?;

        self.network
            .driver
            .coord
            .send(crate::sig::Signal::from(Signal::EndOfMessages).to_bytes()?)?;

        // self.driver.connect_to_coord(&req.ip_addr, resp)?;

        // self.driver.establish_coord_conn();
        // let req: IntroduceCoordRequest = self
        //     .driver
        //     .msg_read_central()
        //     .unwrap()
        //     .unpack_payload()
        //     .unwrap();
        //
        // println!("{:?}", req);

        // let ou =
        // let msg = match local_worker.lock().unwrap().driver.read() {
        //     Ok(m) => m,
        //     Err(e) => {
        //         println!("failed registration: read_message error: {}", e);
        //         return;
        //     }
        // };
        // println!("{:?}", msg);
        // let req: IntroduceCoordRequest = unpack_payload(&msg.payload, false, None).unwrap();

        // let mut out_stream = TcpStream::connect(req.ip_addr).unwrap();
        // let resp = IntroduceCoordResponse {
        //     error: "".to_string(),
        // };
        // send_message(message_from_payload(resp, false), &mut out_stream, None);
        // println!("sent response");

        loop {
            // sleep a little to make this thread less expensive
            // sleep(Duration::from_micros(50));

            let bytes = match self.network.driver.coord.try_read(Some(1)) {
                Ok(m) => m,
                Err(e) => {
                    // println!("{:?}", e);
                    continue;
                }
            };
            let sig = crate::sig::Signal::from_bytes(&bytes)?;
            // debug!("{:?}", sig);
            self.handle_signal(sig.inner())?;
            // self.handle_message(msg, &mut in_stream, &mut out_stream);
        }
    }
}

/// Handles first message from a fellow worker.
fn handle_message_new_comrade(
    worker_arc: Arc<Mutex<Worker>>,
    buf: &mut Vec<u8>,
    mut stream: TcpStream,
) -> Option<Comrade> {
    unimplemented!();
    ////    println!("{:?}", buf);
    //    let mut msg = match unpack_message(buf.to_vec()) {
    //        Some(m) => m,
    //        None => return None,
    //    };
    ////    println!("unpacked message");
    //    let rwr: IntroduceCoordRequest = match unpack_payload(&msg.payload, false, Some(msg.payload_size)) {
    //        Some(r) => r,
    //        None => return None,
    //    };
    ////    println!("unpacked payload");
    //    println!("{:?}", rwr.clone());
    //
    //    let mut server = worker_arc.lock().unwrap();
    //
    ////    if !server.passwd_list.contains(&rcr.passwd) {
    ////        println!("new client failed password auth!");
    ////        return None;
    ////    }
    //
    //    let comrade = Comrade {
    //        name: rwr.name,
    //        ip_addr: stream.peer_addr().unwrap(),
    //        passwd: rwr.passwd,
    //        stream: Some(stream.try_clone().unwrap()),
    //    };
    //    let mut error: String = String::new();
    //    if let Err(e) = server.register_comrade(comrade.try_clone().unwrap()) {
    //        error = e;
    //    }
    //    let resp = IntroduceCoordResponse {
    ////        clients: Vec::new(),
    //        error,
    //    };
    //
    //    send_message(message_from_payload(resp, false), &mut stream, None);
    //    Some(comrade)
}
impl Worker {
    fn handle_signal(&mut self, sig: Signal) -> Result<()> {
        debug!("handling signal: {:?}", sig);

        match sig {
            Signal::InitializeNode(model) => self.handle_sig_initialize_node(model)?,
            Signal::StartProcessStep(event_queue) => {
                self.sim_node
                    .as_mut()
                    .unwrap()
                    .step(&mut self.network, &event_queue)?;
                // self.network
                //     .driver
                //     .coord
                //     .send(crate::sig::Signal::from(Signal::ProcessStepFinished).to_bytes()?)?
            }
            Signal::DataRequestAll => self.handle_sig_data_request_all()?,
            Signal::SpawnEntities(entities) => self.handle_sig_spawn_entities(entities)?,
            _ => (),
        }

        Ok(())
    }
    //TODO include event_queue in the initialization process?
    fn handle_sig_initialize_node(&mut self, model: SimModel) -> Result<()> {
        let mut node = SimNode::from_model(&model)?;
        self.sim_node = Some(node);
        Ok(())
    }

    fn handle_sig_spawn_entities(
        &mut self,
        entities: Vec<(EntityUid, Option<EntityId>, Option<EntityId>)>,
    ) -> Result<()> {
        // debug!("spawning entities: {:?}", entities);
        for (ent_uid, prefab_id, target_id) in entities {
            self.sim_node
                .as_mut()
                .unwrap()
                .add_entity(ent_uid, prefab_id, target_id)?;
        }
        Ok(())
    }

    fn handle_sig_data_request_all(&self) -> Result<()> {
        let mut collection = Vec::new();
        for (entity_uid, entity) in &self.sim_node.as_ref().unwrap().entities {
            for ((comp_id, var_id), var) in entity.storage.get_all_var() {
                collection.push((
                    Address {
                        entity: StringId::from_truncate(&entity_uid.to_string()),
                        component: *comp_id,
                        var_type: VarType::Str,
                        var_id: *var_id,
                    },
                    var.clone(),
                ))
            }
        }
        let signal = Signal::DataResponse(collection);
        self.network
            .driver
            .coord
            .send(crate::sig::Signal::from(signal).to_bytes()?)?;

        Ok(())
    }
    /// Handles an incoming message.
    fn handle_message(&mut self, msg: Message) -> Result<()> {
        debug!("handling message: {}", &msg.kind);

        match msg.kind.as_str() {
            // PING_REQUEST => handle_ping_request(msg, worker)?,
            // DATA_TRANSFER_REQUEST => handle_data_transfer_request(msg, worker)?,
            // DATA_PULL_REQUEST => handle_data_pull_request(msg, worker)?,
            // STATUS_REQUEST => handle_status_request(msg, worker)?,

            //        REGISTER_CLIENT_REQUEST => handle_data_transfer_request(payload, server_arc, stream),
            // SIGNAL_REQUEST => handle_distr_msg_request(payload, worker_arc)?,
            _ => (),
        }
        Ok(())
    }
}
// TODO
fn handle_comrade(local_worker: Arc<Mutex<Worker>>) {
    unimplemented!();
    // println!(
    //     "incoming connection from comrade worker: {:?}",
    //     stream.peer_addr().unwrap()
    // );
    // let msg = match local_worker.lock().unwrap().driver.read() {
    //     Ok(m) => m,
    //     Err(e) => {
    //         println!("failed registration: read_message error: {}", e);
    //         return;
    //     }
    // };
    // println!("{:?}", msg);
}

/// Fellow worker from the same cluster.
pub struct Comrade {
    /// Client self-assigned id
    pub name: String,
    /// Address of the client
    pub addr: SocketAddr,
    /// Password used by the comrade for authentication
    pub passwd: String,
}

// TODO
pub fn handle_ping_request(msg: Message, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    unimplemented!();
    // let req: PingRequest = match unpack_payload(&payload, false, None) {
    //     Some(p) => p,
    //     None => return,
    // };
    // let resp = PingResponse { bytes: req.bytes };
    // send_message(message_from_payload(resp, false), stream, None);
}
// TODO
pub fn handle_status_request(msg: Message, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    unimplemented!();
    // let req: StatusRequest = match unpack_payload(&payload, false, None) {
    //     Some(p) => p,
    //     None => return,
    // };
    // let mut worker = server_arc.lock().unwrap();

    //    let resp = StatusResponse {
    //        connected_comrades: worker.comrades.iter().map(|c| c.name.clone()).collect(),
    //        loaded_scenario: String::new(),
    //    };
    //    send_message(message_from_payload(resp, false), stream, None);
}

pub fn handle_data_transfer_request(msg: Message, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    let dtr: DataTransferRequest = msg.unpack_payload()?;
    let mut data_pack = SimDataPack::empty();
    let mut server = server_arc.lock().unwrap();
    match dtr.transfer_type.as_str() {
        "Full" => {
            unimplemented!();
            for (_, entity) in &server.sim_node.as_ref().unwrap().entities {
                //entity.storage.get
                for (var_name, var) in entity.storage.get_all_str() {

                    //                    let addr = Address::from_str_global(
                    //                        &format!("{}/{}/{}/{}/{}/{}", entity.type_, entity.id, )
                    //                    ).unwrap();
                    ////                    data_pack.strings.insert(addr.to_string(), entity.entity_db.string_vec[var_index]).unwrap();
                    //                    data_pack.strings.insert(
                    //                        format!("{}/{}", entity.type_, entity.id, ),
                    //                        s.to_owned());
                }
            }
        }
        "SelectedAddresses" => {
            for address in &dtr.selection {
                //                println!("{}", address.clone());
                let address = match outcome::Address::from_str(&address) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                match address.var_type {
                    //                    VarType::Str => match server.sim_node.as_ref().unwrap().get_str(&address) {
                    //                        Some(s) => data_pack.strings.insert(
                    //                            address.to_string(),
                    //                            s.to_owned()),
                    //                        None => continue,
                    //                    }
                    _ => continue,
                };
            }
        }
        _ => (),
    }
    let response = DataTransferResponse {
        data: Some(data_pack),
        error: String::new(),
    };
    Ok(())
    // TODO
    // server.driver.send(Message::from_payload(response, true));

    // let msg_size = send_message(message_from_payload(response, true), stream, Some(512000));
    // if let Ok(ms) = msg_size {
    //     println!("sent DataTransferResponse ({} KB)", ms as f32 / 1000.0);
    // }
}

pub fn handle_data_pull_request(msg: Message, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    let mut server = server_arc.lock().unwrap();
    //TODO
    //    let mut sim_model = &server.sim_model.clone();
    let mut sim_instance = &mut server.sim_node;
    let dpr: DataPullRequest = msg.unpack_payload()?;
    //TODO do all other var types
    //TODO handle errors
    for (address, string_var) in dpr.data.strings {
        let addr = Address::from_str(&address)?;
        //        *sim_instance.as_mut().unwrap().get_str_mut(&addr).unwrap() = string_var;
    }

    let resp = DataPullResponse {
        error: String::new(),
    };

    Ok(())
    // TODO
    // server.driver.send(Message::from_payload(resp, false));

    // send_message(message_from_payload(resp, false), stream, None);
}

impl outcome::distr::NodeCommunication for WorkerNetwork {
    fn sig_read_central(&mut self) -> outcome::Result<Signal> {
        let bytes = self.driver.coord.read().unwrap();
        let sig = sig::Signal::from_bytes(&bytes).unwrap();
        Ok(sig.inner())
    }

    fn sig_send_central(&mut self, signal: Signal) -> outcome::Result<()> {
        let sig_bytes = sig::Signal::from(signal).to_bytes().unwrap();
        self.driver.coord.send(sig_bytes).unwrap();
        Ok(())
    }

    fn sig_read(&mut self) -> outcome::Result<(String, Signal)> {
        unimplemented!()
    }

    fn sig_read_from(&mut self, node_id: u32) -> outcome::Result<Signal> {
        unimplemented!()
    }

    fn sig_send_to_node(&mut self, node_id: u32, signal: Signal) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_send_to_entity(&mut self, entity_uid: u32) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_broadcast(&mut self, signal: Signal) -> outcome::Result<()> {
        unimplemented!()
    }

    fn get_nodes(&mut self) -> Vec<String> {
        unimplemented!()
    }
}

// pub fn handle_distr_msg_request(payload: Vec<u8>, worker_arc: Arc<Mutex<Worker>>) -> Result<()> {
//     println!("handling distr msg request");
//     let distr_msg_req: SignalRequest = unpack_payload(&payload, false, None)?;
//     let mut worker = worker_arc.lock().map_err(|e| Error::Other(e.to_string()))?;
//     match distr_msg_req.signal {
//         // Signal::InitializeNode((model, entities)) => {
//         //     println!("{:?}", entities);
//         //     let node = SimNode::from_model(&model, &entities).unwrap();
//         //     worker.sim_node = Some(node);
//         //     let resp = SignalResponse {
//         //         distr_msg: Signal::EndOfMessages,
//         //     };
//         //     send_message(message_from_payload(resp, false), &mut stream_out, None).unwrap();
//         // }
//         Signal::StartProcessStep(event_queue) => {
//             let mut node = worker.sim_node.as_mut().unwrap();
//             // let entity_node_map = HashMap::new();
//             // TODO
//             // let mut addr_book = HashMap::new();
//             // addr_book.insert(
//             //     "0".to_string(),
//             //     TcpStreamConnection {
//             //         stream_in: stream_in.try_clone().unwrap(),
//             //         stream_out: stream_out.try_clone().unwrap(),
//             //     },
//             // );
//             // node.step(&entity_node_map, &mut addr_book);
//         }
//         _ => unimplemented!(),
//     }
//     Ok(())
// }
