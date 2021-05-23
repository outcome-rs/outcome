use std::io::{ErrorKind, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordResponse,
    IntroduceWorkerToOrganizerRequest,
};
use crate::msg::*;
use crate::socket::{
    Encoding, Socket, SocketAddress, SocketConfig, SocketEvent, SocketEventType, Transport,
};
use crate::{error::Error, sig, Result, TaskId};

use fnv::FnvHashMap;
use id_pool::IdPool;
use outcome::Sim;
use outcome_core::distr::{NodeCommunication, Signal, SimNode};
use outcome_core::query::{Query, QueryProduct};
use outcome_core::{
    string, Address, CompName, EntityId, EntityName, SimModel, StringId, Var, VarType,
};
use std::str::FromStr;

pub enum WorkerTask {
    RequestedCoordToProcessStep,
}

/// Network-unique identifier for a single worker
pub type WorkerId = u32;

/// Represents a single union node.
///
/// `Worker`s are connected to, and orchestrated by, a union organizer.
/// They are also connected to each other, either directly or not, depending
/// on network topology used.
///
/// # Usage details
///
/// In a simulation union made up of multiple machines, there is at least
/// one `Worker` running on each machine.
///
/// In terms of initialization, `Worker`s can either actively reach out to
/// an already existing cluster to join in, or passively wait for incoming
/// connection from a coordinator.
///
/// Unless configured otherwise, new `Worker`s can dynamically join into
/// already initialized unions, introducing on-the-fly changes to the
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

    // TODO overhaul the authentication system
    /// Whether the worker uses a password to authorize connecting comrade workers
    pub use_auth: bool,
    /// Password used for incoming connection authorization
    pub passwd_list: Vec<String>,

    /// Simulation node running on this worker
    pub sim_node: Option<outcome::distr::SimNode>,

    tasks: Vec<(u32, WorkerTask)>,
}

pub struct WorkerNetwork {
    /// List of other workers in the cluster
    pub comrades: FnvHashMap<u32, Comrade>,
    /// Organizer connection
    pub organizer: Option<Socket>,

    task_id_pool: IdPool,
}

impl Worker {
    /// Creates a new `Worker`.
    pub fn new(addr: Option<&str>) -> Result<Worker> {
        let address = match addr {
            Some(a) => a.parse()?,
            None => SocketAddr::from_str("0.0.0.0:0")?,
        };
        let greeter = Socket::new(Some(SocketAddress::Net(address)), Transport::Tcp)?;

        Ok(Worker {
            addr: greeter.listener_addr()?.to_string(),
            greeter,
            inviter: Socket::new(None, Transport::Tcp)?,
            network: WorkerNetwork {
                comrades: FnvHashMap::default(),
                organizer: None,
                task_id_pool: IdPool::new(),
            },
            use_auth: false,
            passwd_list: vec![],
            sim_node: None,
            tasks: vec![],
        })
    }

    /// Registers a fellow worker.
    pub fn register_comrade(&mut self, comrade: Comrade) -> Result<()> {
        // if self.use_auth {
        //     if !&self.passwd_list.contains(&comrade.passwd) {
        //         println!("Client provided wrong password");
        //         return Err(Error::Other(String::from("WrongPasswd")));
        //     }
        //     self.network.comrades.push(comrade);
        // } else {
        //     self.network.comrades.push(comrade);
        // }
        // return Ok(());
        unimplemented!()
    }

    pub fn initiate_coord_connection(&mut self, addr: &str, timeout: Duration) -> Result<()> {
        // self.inviter.connect(addr.parse()?)?;
        let mut socket_config = SocketConfig::default();
        // socket_config.heartbeat_interval = Some(Duration::from_secs(1));
        let socket = Socket::new_with_config(None, Transport::Tcp, socket_config)?;
        self.network.organizer = Some(socket);
        let organizer = self.network.organizer.as_mut().unwrap();
        organizer.connect(addr.parse()?)?;
        // thread::sleep(Duration::from_millis(100));

        organizer.send_payload(
            IntroduceWorkerToOrganizerRequest {
                worker_addr: None,
                // worker_addr: self.greeter.listener_addr().unwrap().to_string(),
                //TODO
                worker_passwd: "".to_string(),
            },
            None,
        )?;

        let resp: IntroduceWorkerToCoordResponse = organizer
            .recv_msg()?
            .1
            .unpack_payload(self.inviter.encoding())?;

        organizer.disconnect(None)?;

        println!("trying to connect to: {}", resp.redirect);

        organizer.connect(resp.redirect.parse()?)?;
        organizer.send_sig(crate::sig::Signal::from(0, Signal::WorkerConnected), None);

        // thread::sleep(Duration::from_millis(1000));
        self.manual_poll()?;

        Ok(())
    }

    // TODO
    /// Handles initial connection from the cluster coordinator.
    pub fn handle_coordinator(&mut self) -> Result<()> {
        print!("Waiting for message from coordinator... ");
        std::io::stdout().flush()?;
        let (peer_addr, msg) = self.greeter.recv_msg()?;
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
        let mut coord = Socket::new_with_config(
            Some(SocketAddress::from_str("0.0.0.0:0")?),
            Transport::Tcp,
            soc_config,
        )?;

        self.greeter.send_payload(
            IntroduceCoordResponse {
                conn_socket: coord.listener_addr()?.to_string(),
                error: "".to_string(),
            },
            Some(peer_addr),
        )?;

        coord.connect(req.ip_addr.parse()?)?;

        coord.send_sig(sig::Signal::from(0, Signal::EndOfMessages), None)?;

        self.network.organizer = Some(coord);

        // loop {
        //     // sleep a little to make this thread less expensive
        //     thread::sleep(Duration::from_millis(10));
        //
        //     if let Ok((addr, sig)) = self.network.coord.as_mut().unwrap().try_recv_sig() {
        //         self.handle_signal(sig.into_inner())?;
        //     } else {
        //         continue;
        //     }
        // }
        self.manual_poll()?;

        Ok(())
    }
}

impl Worker {
    pub fn manual_poll(&mut self) -> Result<()> {
        loop {
            if let Some(organ_connection) = self.network.organizer.as_mut() {
                // if let Some(heartbeat_interval) = organ_connection.config().heartbeat_interval {
                //     organ_connection.send_event(SocketEvent::new(SocketEventType::Heartbeat), None);
                // }
                match organ_connection.try_recv() {
                    Ok((addr, event)) => match event.type_ {
                        SocketEventType::Bytes => {
                            let sig =
                                crate::sig::Signal::from_bytes(&event.bytes, &Encoding::Bincode)?;
                            let (task_id, sig) = sig.into_inner();
                            self.handle_coord_signal(task_id, sig)
                                .unwrap_or_else(|e| error!("{:?}", e));
                        }
                        SocketEventType::Heartbeat => (),
                        SocketEventType::Connect => {
                            info!("coordinator connected");
                        }
                        SocketEventType::Timeout => {
                            info!("connection timed out");
                            break;
                        }
                        SocketEventType::Disconnect => {
                            info!("coordinator ended the connection");
                            break;
                            // return Err(Error::SocketNotConnected);
                        }
                    },
                    Err(e) => {
                        // error!("{}", e);
                        break;
                    }
                }
            }
            // if let Ok((addr, sig)) = self.network.coord.as_mut().unwrap().try_recv_sig() {
            //     let (task_id, sig) = sig.into_inner();
            //     self.handle_coord_signal(task_id, sig)?;
            // } else {
            //     break;
            // }
        }
        Ok(())
    }

    fn handle_coord_signal(&mut self, task_id: u32, sig: Signal) -> Result<()> {
        debug!("handling signal: {:?}", sig);

        match sig {
            Signal::InitializeNode(model) => self.handle_sig_initialize_node(model)?,
            Signal::StartProcessStep(event_queue) => {
                let sim_node = self.sim_node.as_mut().unwrap();
                sim_node.step(&mut self.network, &event_queue)?;
            }
            Signal::DataRequestAll => self.handle_sig_data_request_all()?,
            Signal::SpawnEntities(entities) => self.handle_sig_spawn_entities(entities)?,
            Signal::QueryRequest(query) => self.handle_sig_query_request(task_id, query)?,
            Signal::DataPullRequest(pull_data) => {
                self.handle_sig_pull_data_request(task_id, pull_data)?
            }
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

    fn handle_sig_query_request(&mut self, task_id: TaskId, query: Query) -> Result<()> {
        info!("handling query request: {:?}", query);
        if let Some(node) = &self.sim_node {
            let product = query.process(&node.entities, &node.entities_idx)?;
            info!("  product: {:?}", product);
            self.network
                .sig_send_central(task_id, Signal::QueryResponse(product))?;
        }
        Ok(())
    }

    fn handle_sig_pull_data_request(
        &mut self,
        task_id: TaskId,
        pull_data: Vec<(Address, Var)>,
    ) -> Result<()> {
        info!("handling pull data request: {:?}", pull_data);
        if let Some(node) = &mut self.sim_node {
            for (addr, var) in pull_data {
                *node.get_var_mut(&addr)? = var;
            }
        }
        Ok(())
    }

    fn handle_sig_data_request_all(&mut self) -> Result<()> {
        let mut collection = FnvHashMap::default();
        for (entity_uid, entity) in &self.sim_node.as_ref().unwrap().entities {
            for ((comp_id, var_id), var) in entity.storage.map.iter() {
                warn!("sending: {}:{} = {:?}", comp_id, var_id, var);
                collection.insert(
                    (
                        string::new_truncate(&entity_uid.to_string()),
                        comp_id.clone(),
                        var_id.clone(),
                    ),
                    var.clone(),
                );
            }
        }
        let signal = Signal::DataResponse(collection);
        self.network
            .organizer
            .as_mut()
            .unwrap()
            .send_sig(sig::Signal::from(0, signal), None)?;

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
        data: TransferResponseData::Var(data_pack),
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
                // let addr = Address::from_str(&address)?;
                //        *sim_instance.as_mut().unwrap().get_strd_mut(&addr).unwrap() = string_var;
            }
        }
        PullRequestData::NativeAddressedVars(data) => {
            //
        }
        PullRequestData::VarOrdered(order_idx, data) => {
            //
        }
        PullRequestData::NativeAddressedVar((ent_id, comp_name, var_name), var) => {
            //
        }
        PullRequestData::AddressedVars(data) => {
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
    fn request_task_id(&mut self) -> outcome::Result<u32> {
        self.task_id_pool
            .request_id()
            .ok_or(outcome::error::Error::RequestIdError)
    }

    fn return_task_id(&mut self, task_id: TaskId) -> outcome::Result<()> {
        self.task_id_pool
            .return_id(task_id)
            .map_err(|e| outcome::error::Error::ReturnIdError)
    }

    fn sig_read_central(&mut self) -> outcome::Result<(u32, Signal)> {
        if let Some(coord) = &mut self.organizer {
            let sig = match coord.recv_sig() {
                Ok((addr, sig)) => sig,
                Err(e) => return Err(outcome::error::Error::Other(e.to_string())),
            };
            Ok(sig.into_inner())
        } else {
            Err(outcome::error::Error::Other("no coord".to_string()))
        }
    }

    fn sig_send_central(&mut self, task_id: u32, signal: Signal) -> outcome::Result<()> {
        self.organizer
            .as_mut()
            .unwrap()
            .send_sig(sig::Signal::from(task_id, signal), None)
            .unwrap();
        Ok(())
    }

    fn sig_read(&mut self) -> outcome::Result<(u32, u32, Signal)> {
        for (comrade_id, comrade) in &mut self.comrades {
            if let Ok((addr, sig)) = comrade.connection.recv_sig() {
                let (task_id, signal) = sig.into_inner();
                return Ok((*comrade_id, task_id, signal));
            }
        }
        Err(outcome::error::Error::Other(
            "failed reading sig".to_string(),
        ))
    }

    fn sig_read_from(&mut self, node_id: u32) -> outcome::Result<(u32, Signal)> {
        unimplemented!()
    }

    fn sig_send_to_node(
        &mut self,
        node_id: u32,
        task_id: u32,
        signal: Signal,
    ) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_send_to_entity(
        &mut self,
        entity_uid: u32,
        task_id: u32,
        signal: Signal,
    ) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_broadcast(&mut self, task_id: u32, signal: Signal) -> outcome::Result<()> {
        unimplemented!()
    }

    fn get_nodes(&mut self) -> Vec<String> {
        unimplemented!()
    }
}
