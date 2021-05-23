#![allow(unused)]

use std::collections::HashMap;
use std::io::Write;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::ops::DerefMut;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::{io, thread};

use fnv::FnvHashMap;
use id_pool::IdPool;

use outcome::distr::{CentralCommunication, Signal, SimCentral, SimNode};
use outcome::model::Scenario;
use outcome::SimStarter;
use outcome::{distr, EntityId, SimModel};

use crate::error::{Error, Result};
use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordResponse,
    IntroduceWorkerToOrganizerRequest,
};
use crate::msg::{Message, MessageType};
use crate::socket::{CompositeSocketAddress, Socket, SocketAddress, Transport};
use crate::worker::{WorkerId, WorkerTask};
use crate::{sig, TaskId};
use std::convert::TryFrom;

const COORD_ADDRESS: &str = "0.0.0.0:5912";

/// Single worker as seen by the organizer.
pub struct Worker {
    //pub id: WorkerId,
    pub address: SocketAddress,
    pub entities: Vec<EntityId>,
    pub connection: Socket,
    /// Relays information about worker synchronization situation. Workers
    /// that are also servers can block processing of further steps if any of
    /// their connected clients blocks.
    pub is_blocking_step: bool,
}

/// Organizer's networking capabilities.
pub struct OrganizerNet {
    // TODO multiple greeter sockets with different transports/encodings
    /// Outward facing socket workers can connect to
    greeter: Socket,
    /// Socket used for initiating connections with workers
    inviter: Socket,
    /// Workers mapped by their unique integer identifier
    pub workers: FnvHashMap<u32, Worker>,

    /// Entity-worker routing table
    pub routing_table: HashMap<EntityId, WorkerId>,

    task_id_pool: IdPool,
}

/// Enumeration of all possible tasks tracked by organizer.
pub enum OrganizerTask {
    WaitForQueryResponses {
        remaining: u32,
        products: Vec<outcome::query::QueryProduct>,
    },
    WaitForSnapshotResponses {
        remaining: u32,
        snapshots: Vec<outcome::snapshot::SnapshotPart>,
    },
}

impl OrganizerTask {
    pub fn is_finished(&self) -> bool {
        match self {
            OrganizerTask::WaitForQueryResponses { remaining, .. } => *remaining == 0,
            OrganizerTask::WaitForSnapshotResponses { remaining, .. } => *remaining == 0,
            _ => unimplemented!(),
        }
    }
}

/// Organizer holds simulation's central authority struct and manages
/// a network of workers.
///
/// It doesn't hold any entity state, leaving that entirely to workers.
pub struct Organizer {
    /// Simulation's central authority structure
    pub central: SimCentral,
    /// Network connections
    pub net: OrganizerNet,

    /// IP address of the coordinator
    pub address: SocketAddress,
    /// Integer id pool for workers
    worker_pool: IdPool,

    /// This flag gets set to true once organizer had at least one worker
    /// connected and initialized the simulation union.
    ///
    /// # Organizer initialization wait
    ///
    /// Since organizer itself doesn't store and entity data, it must have
    /// worker connections available before it can start the simulation union.
    pub initialized: bool,

    /// If organizer is used as server backend, blocking clients connecting to
    /// that server can make it block the whole union execution.
    pub is_blocking_step: bool,

    /// Organizer tasks allow for doing work in a non-blocking way.
    ///
    /// A body of organizer work can be split into several smaller tasks.
    /// When a task is considered blocking it's stored here. Later the
    /// organizer poller will check on the status of the task, and progress
    /// further with that particular body of work if possible.
    pub tasks: HashMap<u32, OrganizerTask>,
}

impl Organizer {
    /// Starts a new organizer at a randomly chosen localhost port.
    pub fn new_at_any(central: SimCentral, worker_addrs: Vec<String>) -> Result<Self> {
        Self::new(central, "0.0.0.0:0", worker_addrs)
    }

    /// Creates a new organizer listening on the given address.
    pub fn new(central: SimCentral, addr: &str, worker_addrs: Vec<String>) -> Result<Self> {
        let greeter_target: CompositeSocketAddress = addr.parse()?;
        let addr_ip = addr.split(":").collect::<Vec<&str>>()[0];
        let net = OrganizerNet {
            greeter: Socket::new(Some(greeter_target.address.clone()), Transport::Tcp)?,
            inviter: Socket::new(None, greeter_target.transport.unwrap_or(Transport::Tcp))?,
            workers: Default::default(),
            routing_table: Default::default(),
            task_id_pool: IdPool::new(),
        };
        let mut organ = Self {
            central,
            net,
            address: greeter_target.address.clone(),
            worker_pool: IdPool::new_ranged(0..u32::max_value()),
            // routing_table: Default::default(),
            initialized: false,
            is_blocking_step: false,

            // task_id_pool: IdPool::new(),
            tasks: Default::default(),
        };
        for worker_addr in &worker_addrs {
            organ.add_worker(worker_addr)?;
        }
        organ.reach_initialize();
        Ok(organ)
    }

    /// Adds a new worker using provided address.
    ///
    /// On success returns newly assigned unique worker id.
    fn add_worker(&mut self, worker_addr: &str) -> Result<u32> {
        let target_socket: CompositeSocketAddress = worker_addr.parse()?;
        let id = self.worker_pool.request_id().unwrap();
        let ip = match self.net.greeter.listener_addr()? {
            SocketAddress::Net(addr) => addr.ip().to_string(),
            _ => unimplemented!(),
        };
        let socket = Socket::new(
            Some(format!("{}:0", ip).parse()?),
            target_socket.transport.unwrap_or(Transport::Tcp),
        )?;
        let worker = Worker {
            address: target_socket.address,
            entities: vec![],
            connection: socket,
            is_blocking_step: true,
        };
        self.net.workers.insert(id, worker);
        self.central.node_entities.insert(id, Vec::new());
        Ok(id)
    }

    /// Initializes coordinator by connecting to all the listed workers.
    ///
    /// # Connection process
    ///
    /// Coordinator sends an *introduction* request to the worker. The worker
    /// responds by sending back a *redirect* address with a dedicated port
    /// where coordinator can connect to. Coordinator connects to the worker
    /// at provided address.
    ///
    /// # Public address workers
    ///
    /// Coordinator can only initiate connection with workers that are publicly
    /// visible on the network. Workers behind a firewall that don't have any
    /// ports exposed will have to initiate connection to the coordinator
    /// themselves.
    pub fn reach_initialize(&mut self) -> Result<()> {
        for worker_id in self
            .net
            .workers
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<u32>>()
        {
            let worker = self.net.workers.get_mut(&worker_id).unwrap();
            let req = IntroduceCoordRequest {
                ip_addr: worker.connection.listener_addr()?.to_string(),
                //ip_addr: self.address.clone(),
                passwd: "".to_string(),
            };
            self.net.inviter.connect(worker.address.clone())?;
            self.net.inviter.send_payload(req, None)?;
            // println!("sent... ");
            let resp: IntroduceCoordResponse = self
                .net
                .inviter
                .recv_msg()?
                .1
                .unpack_payload(self.net.inviter.encoding())?;
            println!("got response: {:?}", resp);
            self.net.inviter.disconnect(None)?;

            worker.connection.connect(resp.conn_socket.parse()?)?;

            self.initialize_worker_node(&worker_id)?;
        }
        Ok(())
    }

    fn initialize_worker_node(&mut self, id: &u32) -> Result<()> {
        let (worker_id, worker) = self
            .net
            .workers
            .iter_mut()
            .find(|(wid, _)| *wid == id)
            .ok_or(Error::Other(format!(
                "unable to find worker with id: {}",
                id
            )))?;

        println!("inside initialize_worker_node");
        let init_sig = Signal::InitializeNode(self.central.model.clone());
        worker
            .connection
            .send_sig(sig::Signal::from(0, init_sig), None)?;
        println!("did send sig initialize node");

        // check if this is the first worker connected
        // if so, make sure to set up any required additional initialization
        if self.net.workers.len() > 0 && !self.initialized {
            debug!("first worker connected");

            if let Some(starter) = &self.central.starter {
                warn!("starter: {:?}", starter);
                match starter {
                    SimStarter::Scenario(_) => {
                        // module script init
                        //TODO put this somewhere else?
                        if outcome::FEATURE_MACHINE_SCRIPT {
                            self.central
                                .spawn_entity(
                                    Some(outcome::string::new_truncate("_mod_init")),
                                    Some(outcome::string::new_truncate("_mod_init")),
                                    outcome::distr::DistributionPolicy::Random,
                                )
                                .unwrap();
                            self.central
                                .event_queue
                                .push(outcome::string::new_truncate("_scr_init"));

                            self.central.flush_queue(&mut self.net).unwrap();
                        }
                    }
                    SimStarter::Snapshot(snapshot) => {
                        info!("initializing ");
                        // self.central.snap
                    }
                    SimStarter::Experiment(_) => unimplemented!(),
                }
                self.initialized = true;
            } else {
                warn!("no starter");
            }
        }
        Ok(())
    }

    // /// Creates a new coordinator.
    // pub fn new_with_central(central: SimCentral) -> Result<Self> {
    //     let mut coord = Coord { central };
    //     Ok(coord)
    // }
    //
    // /// Creates a new coordinator using a sim model.
    // pub fn new_with_model(model: SimModel) -> Result<Self> {
    //     let sim_central = distr::central::SimCentral::from_model(model)?;
    //     Self::new_with_central(sim_central)
    // }

    /// Starts the polling loop.
    pub fn start(&mut self) -> Result<()> {
        loop {
            self.manual_poll()?;
        }
        Ok(())
    }

    /// Polls for messages coming from workers and processes them accordingly.
    pub fn manual_poll(&mut self) -> Result<()> {
        // TODO support less frequent polling of the greeter socket
        if let Ok((address, msg)) = &self.net.greeter.try_recv_msg() {
            match msg.type_ {
                MessageType::IntroduceWorkerToCoordRequest => {
                    debug!("handling new worker connection request");
                    let req: IntroduceWorkerToOrganizerRequest =
                        msg.unpack_payload(self.net.greeter.encoding()).unwrap();
                    println!("req: {:?}", req);

                    // let worker_id = self
                    //     .add_initialize_worker(&address.to_string(), self.central.model.clone())?;
                    let worker_id = self.add_worker(&address.to_string())?;

                    let resp = IntroduceWorkerToCoordResponse {
                        redirect: self
                            .net
                            .workers
                            .get(&worker_id)
                            .unwrap()
                            .connection
                            .listener_addr()?
                            .to_string(),
                        error: "".to_string(),
                    };
                    println!("redirect: {}", resp.redirect);
                    // &self.net.greeter.send_payload(resp, None).unwrap();
                    &self
                        .net
                        .greeter
                        .send_payload(resp, Some(address.clone()))
                        .unwrap();
                    debug!("sent response");

                    self.net.greeter.disconnect(None);
                }
                _ => trace!("msg.kind: {:?}", msg.type_),
            }
        }

        let mut do_step = false;
        let mut to_unregister = Vec::new();
        let mut to_initialize_node = Vec::new();
        for (worker_id, worker) in self.net.workers.iter_mut() {
            if let Ok((addr, sig)) = worker.connection.try_recv_sig() {
                let (task_id, sig) = sig.into_inner();
                match sig {
                    Signal::WorkerConnected => {
                        warn!(
                            "worker successfully redirected: worker id: {}, worker addr: {}",
                            worker_id, addr,
                        );
                        to_initialize_node.push(worker_id.clone());
                    }
                    Signal::WorkerReady => {
                        worker.is_blocking_step = false;
                    }
                    Signal::WorkerNotReady => {
                        worker.is_blocking_step = true;
                        do_step = false;
                    }
                    Signal::WorkerStepAdvanceRequest(steps) => {
                        do_step = true;
                    }
                    Signal::DataRequestAll => {
                        debug!("got signal from worker {}: DataRequestAll ", worker_id);
                        worker.connection.send_sig(
                            sig::Signal::from(task_id, Signal::DataResponse(Default::default())),
                            None,
                        )?;
                    }
                    Signal::QueryResponse(product) => {
                        if let Some(OrganizerTask::WaitForQueryResponses {
                            remaining,
                            products,
                        }) = self.tasks.get_mut(&task_id)
                        {
                            *remaining -= 1;
                            products.push(product);
                        }
                    }
                    signal => debug!("{:?}", signal),
                }
            }
        }
        for worker_id in to_initialize_node {
            self.initialize_worker_node(&worker_id)?;
        }
        for task_id in to_unregister {
            self.unregister_task(task_id)?;
        }

        if do_step
            && !self.net.workers.iter().any(|(_, w)| w.is_blocking_step)
            && !self.is_blocking_step
        {
            info!("stepping");
            let mut event_queue = self.central.event_queue.clone();
            let step_event_name = outcome::string::new_truncate("step");
            if !event_queue.contains(&step_event_name) {
                event_queue.push(step_event_name);
            }
            self.central.event_queue.clear();
            self.central.step_network(&mut self.net, event_queue);
            self.central.clock += 1;
        }
        Ok(())
    }

    /// Creates a new cluster coordinator and initializes workers.
    pub fn new_with_path(
        scenario_path: &str,
        addr: &str,
        worker_addrs: Vec<String>,
    ) -> Result<Self> {
        let scenario_path = PathBuf::from(scenario_path);
        let scenario = Scenario::from_path(scenario_path.clone())?;
        let model = SimModel::from_scenario(scenario)?;
        let starter = SimStarter::Scenario(
            scenario_path
                .clone()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        );
        let sim_central = SimCentral::from_model(model, Some(starter))?;
        let mut coord = Organizer::new(sim_central, addr, worker_addrs)?;
        debug!("created new cluster coordinator");
        Ok(coord)
    }

    pub fn register_task(&mut self, task: OrganizerTask) -> Result<u32> {
        let task_id = self.net.task_id_pool.request_id().unwrap();
        self.tasks.insert(task_id, task);
        Ok(task_id)
    }

    pub fn unregister_task(&mut self, task_id: u32) -> Result<()> {
        self.tasks.remove(&task_id);
        self.net.task_id_pool.return_id(task_id).unwrap();
        Ok(())
    }
}

impl Organizer {
    pub fn download_snapshots(&mut self) -> Result<TaskId> {
        let task_id = self.register_task(OrganizerTask::WaitForSnapshotResponses {
            remaining: self.net.workers.len() as u32,
            snapshots: vec![],
        })?;
        self.net.broadcast_sig(task_id, Signal::SnapshotRequest)?;
        Ok(task_id)
    }
}

impl outcome::distr::CentralCommunication for OrganizerNet {
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

    fn get_node_ids(&self) -> outcome::Result<Vec<u32>> {
        let worker_ids = self.workers.iter().map(|(wid, _)| *wid).collect();
        Ok(worker_ids)
    }

    fn try_recv_sig(&mut self) -> outcome::Result<(u32, u32, Signal)> {
        // iterate over workers and get the first signal
        for (worker_id, worker) in &mut self.workers {
            match worker.connection.try_recv_sig() {
                Ok((addr, sig)) => {
                    let (_task_id, _sig) = sig.into_inner();
                    return Ok((*worker_id, _task_id, _sig));
                }
                Err(e) => match e {
                    Error::WouldBlock => continue,
                    _ => return Err(outcome::error::Error::Other(e.to_string())),
                },
            }
        }
        Err(outcome::error::Error::WouldBlock)
    }

    fn try_recv_sig_from(&mut self, node_id: u32) -> outcome::Result<(u32, Signal)> {
        let worker = self
            .workers
            .get_mut(&node_id)
            .ok_or(outcome::error::Error::Other(format!(
                "tried to read sig from worker with id: {}, which does not exist",
                node_id
            )))?;
        match worker.connection.try_recv_sig() {
            Ok((addr, sig)) => Ok(sig.into_inner()),
            Err(e) => match e {
                Error::WouldBlock => Err(outcome::error::Error::WouldBlock),
                _ => Err(outcome::error::Error::Other(e.to_string())),
            },
        }
    }

    fn send_sig_to_node(
        &mut self,
        node_id: u32,
        task_id: u32,
        signal: Signal,
    ) -> outcome::Result<()> {
        let signal = sig::Signal::from(task_id, signal);
        self.workers
            .get_mut(&node_id)
            .ok_or(outcome::error::Error::Other(format!(
                "tried to send sig to worker with id: {}, which does not exist",
                node_id
            )))?
            .connection
            .send_sig(signal, None)
            .map_err(|e| outcome::error::Error::NetworkError(format!("{}", e.to_string())));
        Ok(())
    }

    fn send_sig_to_entity(
        &mut self,
        entity_uid: u32,
        task_id: u32,
        signal: Signal,
    ) -> outcome::Result<()> {
        let worker_id = self
            .routing_table
            .get(&entity_uid)
            .ok_or(outcome::error::Error::Other("".to_string()))?;
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(outcome::error::Error::Other(format!(
                "tried to send sig to worker with id: {}, which does not exist",
                worker_id
            )))?;
        let signal = sig::Signal::from(task_id, signal);
        worker.connection.send_sig(signal, None).unwrap();
        Ok(())
    }

    fn broadcast_sig(&mut self, task_id: u32, signal: Signal) -> outcome::Result<()> {
        let signal = sig::Signal::from(task_id, signal);
        let len = self.workers.len();
        for (idx, (worker_id, worker)) in &mut self.workers.iter_mut().enumerate() {
            trace!(
                "broadcasting to {}/{} ({:?})",
                idx + 1,
                len,
                worker.connection.listener_addr()
            );
            worker
                .connection
                .send_sig(signal.clone(), None)
                .unwrap_or_else(|e| error!("{:?}", e));
        }
        Ok(())
    }
}
