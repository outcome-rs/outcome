use std::io::{ErrorKind, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordRequest,
    IntroduceWorkerToCoordResponse,
};
use crate::msg::*;
use crate::socket::{Encoding, Socket, SocketConfig, Transport};
use crate::transport::{SocketInterface, WorkerDriverInterface};
use crate::util::tcp_endpoint;
use crate::{error::Error, sig, Result};

use outcome::Sim;
use outcome_core::distr::{NodeCommunication, Signal, SimNode};
use outcome_core::{
    arraystring, Address, CompName, EntityId, EntityName, SimModel, StringId, Var, VarType,
};

//TODO remove this
/// Default address for the worker
pub const WORKER_ADDRESS: &str = "0.0.0.0:5922";

/// Network-unique identifier for a single worker
pub type WorkerId = u32;

/// Represents a single cluster node.
///
/// `Worker`s are connected to, and orchestrated by, a cluster coordinator.
/// They are also connected to each other, either directly or not, depending
/// on network topology used.
///
/// # Usage details
///
/// In a simulation cluster made up of multiple machines, there is at least
/// one `Worker` running on each machine.
///
/// In terms of initialization, `Worker`s can either actively reach out to
/// an already existing cluster to join in, or passively wait for incoming
/// connection from a coordinator.
///
/// Unless configured otherwise, new `Worker`s can dynamically join into
/// already initialized clusters, allowing for on-the-fly changes to the
/// cluster composition.
///
/// # Discussion
///
/// Worker abstraction could work well with a "thread per core" strategy. This
/// means there would be a single worker per every machine core, instead of
/// single worker per machine utilizing multiple cores with thread-pooling.
/// "Thread per core" promises performance improvements caused by reducing
/// expensive context switching operations. It would require having the ability
/// to switch `SimNode`s to process entities in a single-threaded fashion.
///
/// "Worker spawner" mode could allow for instantiating multiple workers within
/// a context of a single CLI application, based on incoming coordinators'
/// requests. This could make it easier for people to share their machines
/// with people who want to run simulations. For safety reasons it would make
/// sense to allow running it in "sandbox" mode, with only the runtime-level
/// logic enabled.
pub struct Worker {
    pub addr: String,
    pub greeter: Socket,
    pub inviter: Socket,
    pub network: WorkerNetwork,

    /// Whether the worker uses a password to authorize connecting comrade workers
    pub use_auth: bool,
    /// Password used for incoming connection authorization
    pub passwd_list: Vec<String>,

    /// Simulation node running on this worker
    pub sim_node: Option<outcome::distr::SimNode>,
}

pub struct WorkerNetwork {
    /// List of other workers in the cluster
    pub comrades: Vec<Comrade>,
    /// Main coordinator
    pub coord: Option<Socket>,
}

impl Worker {
    /// Creates a new `Worker`.
    pub fn new(addr: &str) -> Result<Worker> {
        Ok(Worker {
            addr: addr.to_string(),
            greeter: Socket::bind(addr, Transport::Tcp)?,
            inviter: Socket::bind_any(Transport::Tcp)?,
            network: WorkerNetwork {
                comrades: vec![],
                coord: None,
            },
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
            self.network.comrades.push(comrade);
        } else {
            self.network.comrades.push(comrade);
        }
        return Ok(());
    }

    pub fn initiate_coord_connection(&mut self, addr: &str, timeout: Duration) -> Result<()> {
        self.inviter.connect(addr)?;
        thread::sleep(Duration::from_millis(100));
        self.inviter.pack_send_msg_payload(
            IntroduceWorkerToCoordRequest {
                worker_addr: self.addr.clone(),
                //TODO
                worker_passwd: "".to_string(),
            },
            None,
        )?;

        let resp: IntroduceWorkerToCoordResponse = self
            .inviter
            .try_recv_msg()?
            .unpack_payload(self.inviter.encoding())?;

        self.inviter.disconnect(None)?;
        Ok(())
    }

    // TODO
    /// Handles initial connection from the cluster coordinator.
    pub fn handle_coordinator(&mut self) -> Result<()> {
        print!("Waiting for message from coordinator... ");
        std::io::stdout().flush()?;
        let (_, msg) = self.greeter.recv_msg()?;
        println!("success");

        debug!("message from coordinator: {:?}", msg);

        let req: IntroduceCoordRequest = msg.unpack_payload(self.greeter.encoding())?;

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

        let addr_stem = self.addr.split(":").collect::<Vec<&str>>()[0];
        let socket_addr = format!("{}:0", addr_stem);

        let soc_config = SocketConfig {
            ..Default::default()
        };
        let mut coord = Socket::bind_any_with_config(Transport::Tcp, soc_config)?;

        self.greeter.pack_send_msg_payload(
            IntroduceCoordResponse {
                conn_socket: coord.last_endpoint().unwrap().to_string(),
                error: "".to_string(),
            },
            None,
        )?;

        coord.connect(&req.ip_addr)?;

        coord.send_sig(Signal::EndOfMessages, None)?;

        self.network.coord = Some(coord);

        loop {
            // sleep a little to make this thread less expensive
            thread::sleep(Duration::from_millis(10));

            if let Ok((addr, sig)) = self.network.coord.as_mut().unwrap().try_recv_sig() {
                self.handle_signal(sig.into_inner())?;
            } else {
                continue;
            }
        }
    }
}

impl Worker {
    fn handle_signal(&mut self, sig: Signal) -> Result<()> {
        debug!("handling signal: {:?}", sig);

        match sig {
            Signal::InitializeNode(model) => self.handle_sig_initialize_node(model)?,
            Signal::StartProcessStep(event_queue) => {
                let sim_node = self.sim_node.as_mut().unwrap();
                sim_node.step(&mut self.network, &event_queue)?;
            }
            Signal::DataRequestAll => self.handle_sig_data_request_all()?,
            Signal::SpawnEntities(entities) => self.handle_sig_spawn_entities(entities)?,
            _ => warn!("unhandled signal: {:?}", sig),
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
        entities: Vec<(EntityId, Option<EntityName>, Option<EntityName>)>,
    ) -> Result<()> {
        warn!("spawning entities: {:?}", entities);
        for (ent_uid, prefab_id, target_id) in entities {
            self.sim_node
                .as_mut()
                .unwrap()
                .add_entity(ent_uid, prefab_id, target_id)?;
        }
        Ok(())
    }

    fn handle_sig_data_request_all(&mut self) -> Result<()> {
        let mut collection = Vec::new();
        for (entity_uid, entity) in &self.sim_node.as_ref().unwrap().entities {
            for ((comp_id, var_id), var) in entity.storage.map.iter() {
                warn!("sending: {}:{} = {:?}", comp_id, var_id, var);
                collection.push((
                    Address {
                        entity: arraystring::new_truncate(&entity_uid.to_string()),
                        component: *comp_id,
                        var_type: var.get_type(),
                        var_id: *var_id,
                    },
                    var.clone(),
                ))
            }
        }
        let signal = Signal::DataResponse(collection);
        self.network
            .coord
            .as_mut()
            .unwrap()
            .send_sig(signal, None)?;

        Ok(())
    }
    /// Handles an incoming message.
    fn handle_message(&mut self, msg: Message) -> Result<()> {
        debug!("handling message: {:?}", &msg.type_);

        match msg.type_ {
            // PING_REQUEST => handle_ping_request(msg, worker)?,
            // MessageKind::DataTransferRequest => handle_data_transfer_request(msg, worker)?,
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
    pub name: String,
    pub addr: SocketAddr,
    pub connection: Socket,
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
    unimplemented!();
    let dtr: DataTransferRequest = msg.unpack_payload(&Encoding::Bincode)?;
    let mut data_pack = VarSimDataPack::default();
    let mut server = server_arc.lock().unwrap();
    match dtr.transfer_type.as_str() {
        "Full" => {
            unimplemented!();
            for (_, entity) in &server.sim_node.as_ref().unwrap().entities {
                //entity.storage.get
                for (var_name, var) in entity.storage.map.iter() {

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
        data: Some(TransferResponseData::Var(data_pack)),
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
    let dpr: DataPullRequest = msg.unpack_payload(&Encoding::Bincode)?;
    match dpr.data {
        PullRequestData::Typed(data) => {
            //TODO do all other var types
            //TODO handle errors
            for (address, string_var) in data.strings {
                let addr = Address::from_str(&address)?;
                //        *sim_instance.as_mut().unwrap().get_str_mut(&addr).unwrap() = string_var;
            }
        }
        PullRequestData::Var(data) => {
            //
        }
        PullRequestData::VarOrdered(order_idx, data) => {
            //
        }
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
        if let Some(coord) = &mut self.coord {
            let sig = match coord.recv_sig() {
                Ok((addr, sig)) => sig,
                Err(e) => return Err(outcome::error::Error::Other(e.to_string())),
            };
            Ok(sig.into_inner())
        } else {
            Err(outcome::error::Error::Other("no coord".to_string()))
        }
    }

    fn sig_send_central(&mut self, signal: Signal) -> outcome::Result<()> {
        self.coord.as_mut().unwrap().send_sig(signal, None).unwrap();
        Ok(())
    }

    fn sig_read(&mut self) -> outcome::Result<(String, Signal)> {
        for comrade in &mut self.comrades {
            if let Ok((addr, sig)) = comrade.connection.recv_sig() {
                return Ok((comrade.name.to_string(), sig.into_inner()));
            }
        }
        Err(outcome::error::Error::Other(
            "failed reading sig".to_string(),
        ))
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
