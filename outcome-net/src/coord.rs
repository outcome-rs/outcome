#![allow(unused)]

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::{io, thread};

use fnv::FnvHashMap;
use id_pool::IdPool;

use outcome::distr::{Signal, SimCentral, SimNode};
use outcome::{distr, EntityId, SimModel};

use crate::error::{Error, Result};
use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordRequest,
    IntroduceWorkerToCoordResponse,
};
use crate::msg::{Message, MessageType};
use crate::sig;
use crate::socket::{Socket, Transport};
use crate::util::tcp_endpoint;
use crate::worker::WorkerId;

const COORD_ADDRESS: &str = "0.0.0.0:5912";

/// Single worker as seen by the coordinator.
pub struct Worker {
    //pub id: WorkerId,
    pub address: String,
    pub entities: Vec<EntityId>,
    pub connection: Socket,
}

pub struct CoordNetwork {
    greeter: Socket,
    inviter: Socket,

    /// Map of workers
    pub workers: FnvHashMap<u32, Worker>,
}

pub struct Coord {
    pub central: SimCentral,

    pub net: CoordNetwork,

    /// IP address of the coordinator
    pub address: String,
    /// Integer id pool for workers
    id_pool: IdPool,
    // /// Entity-worker routing table
    // pub routing_table: HashMap<EntityUid, WorkerId>,
}

impl Coord {
    /// Starts a new coordinator at a randomly chosen localhost port.
    pub fn new_at_any(central: SimCentral, worker_addrs: Vec<String>) -> Result<Self> {
        Self::new(central, "127.0.0.1:0", worker_addrs)
    }

    /// Creates a new coordinator listening on the given address.
    pub fn new(central: SimCentral, addr: &str, worker_addrs: Vec<String>) -> Result<Self> {
        let addr_ip = addr.split(":").collect::<Vec<&str>>()[0];
        let net = CoordNetwork {
            greeter: Socket::bind(addr, Transport::Tcp)?,
            inviter: Socket::bind(&format!("{}:4141", addr_ip), Transport::Tcp)?,
            workers: Default::default(),
        };
        let mut coord = Self {
            central,
            net,
            address: addr.to_string(),
            id_pool: IdPool::new(),
            // routing_table: Default::default(),
        };
        for worker_addr in &worker_addrs {
            coord.add_worker(worker_addr)?;
        }
        Ok(coord)
    }

    fn add_worker(&mut self, worker_addr: &str) -> Result<u32> {
        let id = self.id_pool.request_id().unwrap();
        let socket = Socket::bind(
            &format!(
                "{}:892{}",
                self.address
                    .split(':')
                    .collect::<Vec<&str>>()
                    .first()
                    .unwrap(),
                id
            ),
            Transport::prefer_laminar(),
        )?;
        let worker = Worker {
            // id,
            address: worker_addr.to_string(),
            entities: vec![],
            connection: socket,
        };
        self.net.workers.insert(id, worker);
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
    pub fn initialize(&mut self, model: SimModel) -> Result<()> {
        for worker_id in self
            .net
            .workers
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<u32>>()
        {
            self.initialize_worker(worker_id, model.clone())?;
        }
        Ok(())
    }

    fn initialize_worker(&mut self, id: u32, model: SimModel) -> Result<()> {
        let (worker_id, worker) = self
            .net
            .workers
            .iter_mut()
            .find(|(wid, _)| *wid == &id)
            .ok_or(Error::Other(format!(
                "unable to find worker with id: {}",
                id
            )))?;

        //worker.connection.connect(&worker.address)?;

        let req = IntroduceCoordRequest {
            ip_addr: worker.connection.last_endpoint().unwrap().to_string(),
            //ip_addr: self.address.clone(),
            passwd: "".to_string(),
        };
        self.net.inviter.connect(&worker.address)?;
        self.net.inviter.pack_send_msg_payload(req, None)?;
        // println!("sent... ");
        let resp: IntroduceCoordResponse = self
            .net
            .inviter
            .recv_msg()?
            .1
            .unpack_payload(self.net.inviter.encoding())?;
        println!("got response: {:?}", resp);
        self.net.inviter.disconnect(None)?;

        worker.connection.connect(&resp.laminar_socket)?;
        let init_sig = Signal::InitializeNode(model);
        worker
            .connection
            .send_sig(crate::sig::Signal::from(init_sig), None)?;
        Ok(())
    }

    fn add_initialize_worker(&mut self, worker_addr: &str, model: SimModel) -> Result<u32> {
        let id = self.add_worker(worker_addr)?;
        self.initialize_worker(id, model)?;
        Ok(id)
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

    /// Starts the  polling loop.
    pub fn start(&mut self) -> Result<()> {
        loop {
            self.manual_poll()?;
        }
        Ok(())
    }

    // TODO support less frequent polling of the greeter socket
    /// Polls for messages coming from workers and processes them accordingly.
    pub fn manual_poll(&mut self) -> Result<()> {
        if let Ok(msg) = &self.net.greeter.try_recv_msg() {
            match msg.type_ {
                MessageType::IntroduceWorkerToCoordRequest => {
                    debug!("handling new worker connection request");
                    let req: IntroduceWorkerToCoordRequest =
                        msg.unpack_payload(self.net.greeter.encoding()).unwrap();
                    let resp = IntroduceWorkerToCoordResponse {
                        error: "".to_string(),
                    };
                    &self.net.greeter.pack_send_msg_payload(resp, None).unwrap();

                    let worker_id = self
                        .add_initialize_worker(&req.worker_addr, self.central.model.clone())
                        .unwrap();

                    self.central.node_entities.insert(worker_id, Vec::new());

                    // check if this is the only (first) worker
                    if self.net.workers.len() == 1 {
                        debug!("first worker connected");

                        // module script init
                        //TODO put this somewhere else?
                        if outcome::FEATURE_MACHINE_SCRIPT {
                            self.central
                                .spawn_entity(
                                    Some(outcome::StringId::from("_mod_init").unwrap()),
                                    Some(outcome::StringId::from("_mod_init").unwrap()),
                                    outcome::distr::DistributionPolicy::Random,
                                )
                                .unwrap();
                            self.central
                                .event_queue
                                .push(outcome::StringId::from("_scr_init").unwrap());

                            self.central.flush_queue(&mut self.net).unwrap();
                        }
                    }
                }
                _ => trace!("msg.kind: {:?}", msg.type_),
            }
        }

        for (worker_id, mut worker) in &mut self.net.workers {
            if let Ok((addr, sig)) = worker.connection.try_recv_sig() {
                debug!("{:?}", sig);
            }
        }
        Ok(())
    }

    /// Creates a new cluster coordinator and initializes workers.
    pub fn new_with_path(
        scenario_path: &str,
        addr: &str,
        worker_addrs: Vec<String>,
    ) -> Result<Self> {
        // let mut net = CoordNetwork::new(addr, worker_addrs)?;
        let scenario = outcome::model::Scenario::from_path(PathBuf::from(scenario_path))?;
        let model = SimModel::from_scenario(scenario)?;
        let sim_central = SimCentral::from_model(model)?;

        // net.initialize(model.clone());
        let mut coord = Coord::new(sim_central, addr, worker_addrs)?;

        // let net_arc = Arc::new(Mutex::new(net));
        // let coord_arc = Arc::new(Mutex::new(coord));
        debug!("created new cluster coordinator");

        // let coord_arc_clone = coord_arc.clone();
        // let net_arc_clone = net_arc.clone();
        // let model = coord_arc.lock().unwrap().central.model.clone();
        // let net_arc_clone = net_arc.clone();
        // thread::spawn(move || loop {
        //     sleep(Duration::from_micros(100));
        // });

        Ok(coord)
    }

    // /// Starts a new cluster coordinator.
    // pub fn start(scenario_path: PathBuf, coord_addr: &str, workers_addr: &str) -> Result<Coord> {
    //     let mut worker_addrs: Vec<&str> = workers_addr.split(",").collect();
    //
    //     let sim_central = distr::central::SimCentral::from_scenario_at(scenario_path)?;
    //
    //     // let coord = Coord::new();
    //     // let ent_assignment =
    //     //     sim_central.assign_entities(worker_addrs.len(), EntityAssignMethod::Random);
    //     let ent_assignment: Vec<Vec<EntityUid>> = Vec::new();
    //
    //     // create new coord network driver using the provided address
    //     let mut driver = CoordDriver::new(coord_addr)?;
    //     thread::sleep(Duration::from_millis(100));
    //
    //     let mut workers = Vec::new();
    //     // let mut connections = HashMap::new();
    //     let mut all_good = true;
    //     for (n, worker_addr) in worker_addrs.iter().enumerate() {
    //         print!("inviting worker at: {}... ", worker_addr);
    //         io::stdout().flush()?;
    //
    //         let msg = Message::from_payload(
    //             IntroduceCoordRequest {
    //                 ip_addr: coord_addr.to_string(),
    //                 passwd: "".to_string(),
    //             },
    //             false,
    //         )?;
    //
    //         // driver.connect_to_worker(worker_addr, msg.clone())?;
    //         //thread::sleep(Duration::from_millis(1000));
    //
    //         let (worker_id, msg) = driver.accept()?;
    //         println!("connection established!");
    //         println!("worker {} responded msg: {:?}", worker_id, msg);
    //
    //         // let (mut stream2, addr) = listener.accept().unwrap();
    //         // stream.set_nonblocking(true).unwrap();
    //         // stream2.set_nonblocking(true).unwrap();
    //         // println!("{} is the new socket address!", addr);
    //         // receive response
    //         // let intro_resp = read_message(&mut stream2).unwrap();
    //         // println!("{:?}", intro_resp);
    //
    //         // let stream2 = match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
    //         //     Ok(s) => s,
    //         //     Err(e) => {
    //         //         all_good = false;
    //         //         continue;
    //         //     }
    //         // };
    //         // let worker_id = format!("{}", n);
    //
    //         workers.push(Worker {
    //             id: worker_id.clone(),
    //             address: worker_addr.to_string(),
    //             entities: ent_assignment[n].clone(),
    //             pair_sock: PairSocket::default(),
    //             // connection: TcpStreamConnection {
    //             //     stream_in: stream2.try_clone().unwrap(),
    //             //     stream_out: stream.try_clone().unwrap(),
    //             // },
    //         });
    //         //            connections.insert(worker_id, (stream, stream2));
    //     }
    //     if !all_good {
    //         // println!("failed connecting to one or more workers, aborting!");
    //         return Err(Error::Other(
    //             "failed connecting to one or more workers, aborting!".to_string(),
    //         ));
    //     }
    //
    //     let mut entity_node_map = HashMap::new();
    //     for worker in &workers {
    //         for ent_uid in &worker.entities {
    //             entity_node_map.insert(*ent_uid, worker.id.clone());
    //         }
    //     }
    //
    //     // send initialize messages to nodes
    //     for mut worker in &workers {
    //         // TODO
    //         let sig = Signal::InitializeNode((sim_central.model.clone(), worker.entities.clone()));
    //         // let init_req = SignalRequest { signal: distr_msg };
    //         let msg = Message::from_payload(sig, false)?;
    //         driver.msg_send_worker(&worker.id, msg)?;
    //         println!("sent initialize_node msg");
    //     }
    //     // receive initialization responses from nodes
    //     for mut node in &mut workers {
    //         // TODO
    //         // let distr_msg_msg = read_message(&mut node.connection.stream_in).unwrap();
    //         // let distr_msg_resp: SignalResponse =
    //         //     unpack_payload(&distr_msg_msg.payload, false, None).unwrap();
    //         // println!("{:?}", distr_msg_resp.distr_msg);
    //     }
    //
    //     let coord = Coord {
    //         address: coord_addr.to_string(),
    //         central: sim_central,
    //         routing_table: entity_node_map,
    //         workers,
    //         driver,
    //     };
    //
    //     Ok(coord)
    // }
}

impl outcome::distr::CentralCommunication for CoordNetwork {
    fn sig_read(&mut self) -> outcome::Result<(u32, Signal)> {
        //TODO
        for (worker_id, worker) in &mut self.workers {
            let (addr, sig) = worker.connection.recv_sig().unwrap();
            return Ok((*worker_id, sig.into_inner()));
        }
        Err(outcome::error::Error::Other(
            "failed reading sig".to_string(),
        ))
    }

    fn sig_read_from(&mut self, node_id: u32) -> outcome::Result<Signal> {
        unimplemented!()
    }

    fn sig_send_to_node(&mut self, node_id: u32, signal: Signal) -> outcome::Result<()> {
        let sig = sig::Signal::from(signal);
        self.workers
            .get_mut(&node_id)
            .ok_or(outcome::error::Error::Other(format!(
                "no worker with id: {}",
                node_id
            )))?
            .connection
            .send_sig(sig, None)
            .map_err(|e| outcome::error::Error::Other(format!("network error: {}", e)));
        Ok(())
    }

    fn sig_send_to_entity(&mut self, entity_uid: u32) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_broadcast(&mut self, signal: Signal) -> outcome::Result<()> {
        let sig = sig::Signal::from(signal);
        let len = self.workers.len();
        for (idx, (worker_id, worker)) in &mut self.workers.iter_mut().enumerate() {
            println!("broadcasting to {}/{}", idx, len);
            println!("{:?}", worker.connection.last_endpoint());
            worker.connection.send_sig(sig.clone(), None).unwrap();
        }
        Ok(())
    }
}
