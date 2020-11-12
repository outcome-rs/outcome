#![allow(unused)]

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::{io, thread};

use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, IntroduceWorkerToCoordRequest,
    IntroduceWorkerToCoordResponse, INTRODUCE_WORKER_TO_COORD_REQUEST,
    INTRODUCE_WORKER_TO_COORD_RESPONSE,
};
use crate::msg::{unpack_payload, Message};

use crate::error::{Error, Result};
use crate::sig;
use crate::transport::{CoordDriverInterface, SocketInterface};
use crate::worker::WorkerId;
use crate::{tcp_endpoint, CoordDriver, PairSocket};

use id_pool::IdPool;
use outcome::distr::{EntityAssignMethod, Signal, SimCentral, SimNode};
use outcome::sim::interface::SimInterface;
use outcome::{distr, EntityUid, SimModel};

const COORD_ADDRESS: &str = "0.0.0.0:5912";

/// Single worker as seen by the coordinator.
pub struct Worker {
    pub id: WorkerId,
    pub address: String,
    pub entities: Vec<EntityUid>,
    pub pair_sock: PairSocket,
}

/// Cluster coordinator.
///
/// # Abstraction over `SimCentral`
///
/// Coordinator wraps
pub struct Coord {
    /// IP address of the coordinator
    pub address: String,
    /// Network driver
    driver: CoordDriver,

    /// Central authority abstraction
    pub central: SimCentral,
    /// List of co-op workers
    pub workers: Vec<Worker>,
    /// Id pool for workers
    id_pool: IdPool,
    /// Entity-worker routing table
    pub routing_table: HashMap<EntityUid, WorkerId>,
}

impl Coord {
    /// Creates a new coordinator.
    pub fn new(central: SimCentral, addr: &str, worker_addrs: Vec<String>) -> Result<Self> {
        let mut workers = Vec::new();
        let mut id_pool = IdPool::new();

        let driver = CoordDriver::new(addr)?;

        let mut coord = Coord {
            address: addr.to_string(),
            driver,
            central,
            routing_table: HashMap::new(),
            workers,
            id_pool,
        };

        for worker_addr in &worker_addrs {
            coord.add_worker(worker_addr)?;
        }

        Ok(coord)
    }

    /// Creates a new coordinator using path to scenario.
    pub fn new_from_scenario_at(path: &str, addr: &str, worker_addrs: Vec<String>) -> Result<Self> {
        let sim_central = distr::central::SimCentral::from_scenario_at(PathBuf::from(path))?;
        Self::new(sim_central, addr, worker_addrs)
    }

    fn add_worker(&mut self, worker_addr: &str) -> Result<u32> {
        let pair_sock = self.driver.new_pair_socket()?;
        let id = self.id_pool.request_id().unwrap();
        pair_sock.bind(&format!("127.0.0.1:898{}", id))?;
        let worker = Worker {
            id,
            address: worker_addr.to_string(),
            entities: vec![],
            pair_sock,
        };
        self.workers.push(worker);
        Ok(id)
    }

    fn initialize_worker(&mut self, id: u32) -> Result<()> {
        let worker = self
            .workers
            .iter()
            .find(|w| w.id == id)
            .ok_or(Error::Other("".to_string()))?;

        let req = IntroduceCoordRequest {
            ip_addr: worker.pair_sock.last_endpoint(),
            passwd: "".to_string(),
        };
        self.driver
            .inviter
            .connect(&tcp_endpoint(&worker.address))?;
        self.driver
            .inviter
            .send_msg(Message::from_payload(req, false)?)?;
        // println!("sent... ");
        let resp: IntroduceCoordResponse = self.driver.inviter.read_msg()?.unpack_payload()?;
        // println!("got response: {:?}", resp);
        self.driver.inviter.disconnect("")?;
        let init_sig = Signal::InitializeNode((self.central.model.clone(), Vec::new()));
        worker
            .pair_sock
            .send(crate::sig::Signal::from(init_sig).to_bytes()?)?;
        Ok(())
    }

    fn add_initialize_worker(&mut self, worker_addr: &str) -> Result<()> {
        let id = self.add_worker(worker_addr)?;
        self.initialize_worker(id)?;
        Ok(())
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
    /// Initializes coordinator by connecting to all the workers.
    pub fn initialize(&mut self) -> Result<()> {
        for worker_id in self.workers.iter().map(|w| w.id).collect::<Vec<u32>>() {
            self.initialize_worker(worker_id)?;
        }

        Ok(())
    }

    /// Starts a new cluster coordinator and initializes workers.
    pub fn start(
        scenario_path: &str,
        addr: &str,
        worker_addrs: Vec<String>,
    ) -> Result<Arc<Mutex<Coord>>> {
        let mut coord = Coord::new_from_scenario_at(scenario_path, addr, worker_addrs)?;
        coord.initialize();
        let coord_arc = Arc::new(Mutex::new(coord));
        debug!("created new cluster coordinator");

        let coord_arc_clone = coord_arc.clone();
        thread::spawn(move || loop {
            sleep(Duration::from_millis(100));
            let mut coord_guard = coord_arc_clone.lock().unwrap();
            if let Ok(msg) = &coord_guard.driver.greeter.try_read_msg(None) {
                match msg.kind.as_str() {
                    INTRODUCE_WORKER_TO_COORD_REQUEST => {
                        debug!("handling new worker connection request");
                        let req: IntroduceWorkerToCoordRequest = msg.unpack_payload().unwrap();
                        let resp = IntroduceWorkerToCoordResponse {
                            error: "".to_string(),
                        };
                        &coord_guard
                            .driver
                            .greeter
                            .send_msg(Message::from_payload(resp, false).unwrap());

                        coord_guard.add_initialize_worker(&req.worker_addr).unwrap();
                    }
                    _ => trace!("msg.kind: {}", msg.kind),
                }
            }
        });
        let coord_arc_clone = coord_arc.clone();
        thread::spawn(move || loop {
            sleep(Duration::from_millis(1));
            let mut coord_guard = coord_arc_clone.lock().unwrap();
            for worker in &coord_guard.workers {
                match worker.pair_sock.try_read(None) {
                    Ok(sig) => debug!("{:?}", sig::Signal::from_bytes(&sig).unwrap()),
                    _ => (),
                }
            }
        });

        Ok(coord_arc)
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

impl outcome::distr::CentralCommunication for &mut Coord {
    fn sig_read(&mut self) -> outcome::Result<(String, Signal)> {
        //TODO
        for worker in &self.workers {
            let bytes = worker.pair_sock.read().unwrap();
            let sig = sig::Signal::from_bytes(&bytes).unwrap();
            return Ok((worker.id.to_string(), sig.inner()));
        }
        Err(outcome::error::Error::Other(
            "failed reading sig".to_string(),
        ))
    }

    fn sig_read_from(&mut self, node_id: &str) -> outcome::Result<Signal> {
        unimplemented!()
    }

    fn sig_send_to_node(&mut self, node_id: &str, signal: Signal) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_send_to_entity(&mut self, entity_uid: u32) -> outcome::Result<()> {
        unimplemented!()
    }

    fn sig_broadcast(&mut self, signal: Signal) -> outcome::Result<()> {
        let sig_bytes = sig::Signal::from(signal).to_bytes().unwrap();
        for worker in &self.workers {
            worker.pair_sock.send(sig_bytes.clone()).unwrap();
        }
        Ok(())
    }
}
