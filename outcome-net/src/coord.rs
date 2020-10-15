#![allow(unused)]

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::{io, thread};

use crate::msg::coord_worker::{
    IntroduceCoordRequest, IntroduceCoordResponse, SignalRequest, SignalResponse,
};
use crate::msg::{unpack_payload, Message};

use crate::error::{Error, Result};
use crate::transport::CoordDriverInterface;
use crate::worker::WorkerId;
use crate::CoordDriver;

use outcome::distr::{EntityAssignMethod, Signal, SimCentral, SimNode};
use outcome::sim::interface::SimInterface;
use outcome::{distr, EntityId, SimModel};
use std::time::Duration;

const COORD_ADDRESS: &str = "0.0.0.0:5912";

pub struct Worker {
    pub id: WorkerId,
    pub addr: String,
    pub entities: Vec<EntityId>,
}

/// Central authority of a cluster.
pub struct Coord {
    pub main: SimCentral,
    pub my_addr: String,
    pub entity_node_map: HashMap<EntityId, WorkerId>,
    pub workers: Vec<Worker>,
    driver: CoordDriver,
}
impl Coord {
    /// Start a new cluster coordinator.
    pub fn start(scenario_path: PathBuf, coord_addr: &str, workers_addr: &str) -> Result<Coord> {
        let mut worker_addr_list: Vec<&str> = workers_addr.split(",").collect();
        let sim_central = distr::central::SimCentral::from_scenario_at(scenario_path)?;
        let ent_assignment =
            sim_central.assign_entities(worker_addr_list.len(), EntityAssignMethod::Random);

        let mut driver = CoordDriver::new(coord_addr)?;
        thread::sleep(Duration::from_millis(100));

        let mut workers = Vec::new();
        // let mut connections = HashMap::new();
        let mut all_good = true;
        for (n, worker_addr) in worker_addr_list.iter().enumerate() {
            print!("inviting worker at: {}... ", worker_addr);
            io::stdout().flush()?;

            let msg = Message::from_payload(
                IntroduceCoordRequest {
                    ip_addr: coord_addr.to_string(),
                    passwd: "".to_string(),
                },
                false,
            )?;
            driver.connect_to_worker(worker_addr, msg.clone())?;
            println!("sent... ");
            //thread::sleep(Duration::from_millis(1000));

            let (worker_id, msg) = driver.accept()?;
            println!("connection established!");
            println!("worker {} responded msg: {:?}", worker_id, msg);

            // let (mut stream2, addr) = listener.accept().unwrap();
            // stream.set_nonblocking(true).unwrap();
            // stream2.set_nonblocking(true).unwrap();
            // println!("{} is the new socket address!", addr);
            // receive response
            // let intro_resp = read_message(&mut stream2).unwrap();
            // println!("{:?}", intro_resp);

            // let stream2 = match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            //     Ok(s) => s,
            //     Err(e) => {
            //         all_good = false;
            //         continue;
            //     }
            // };
            // let worker_id = format!("{}", n);
            workers.push(Worker {
                id: worker_id.clone(),
                addr: worker_addr.to_string(),
                entities: ent_assignment[n].clone(),
                // connection: TcpStreamConnection {
                //     stream_in: stream2.try_clone().unwrap(),
                //     stream_out: stream.try_clone().unwrap(),
                // },
            });
            //            connections.insert(worker_id, (stream, stream2));
        }
        if !all_good {
            // println!("failed connecting to one or more workers, aborting!");
            return Err(Error::Other(
                "failed connecting to one or more workers, aborting!".to_string(),
            ));
        }

        let mut entity_node_map = HashMap::new();
        for worker in &workers {
            for ent_uid in &worker.entities {
                entity_node_map.insert(*ent_uid, worker.id.clone());
            }
        }

        // send initialize messages to nodes
        for mut worker in &workers {
            // TODO
            let sig = Signal::InitializeNode((sim_central.model.clone(), worker.entities.clone()));
            // let init_req = SignalRequest { signal: distr_msg };
            let msg = Message::from_payload(sig, false)?;
            driver.msg_send_worker(&worker.id, msg)?;
            println!("sent initialize_node msg");
        }
        // receive initialization responses from nodes
        for mut node in &mut workers {
            // TODO
            // let distr_msg_msg = read_message(&mut node.connection.stream_in).unwrap();
            // let distr_msg_resp: SignalResponse =
            //     unpack_payload(&distr_msg_msg.payload, false, None).unwrap();
            // println!("{:?}", distr_msg_resp.distr_msg);
        }

        let coord = Coord {
            my_addr: coord_addr.to_string(),
            main: sim_central,
            entity_node_map,
            workers,
            driver,
        };

        Ok(coord)
    }
}
