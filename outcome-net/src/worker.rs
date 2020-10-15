#![allow(dead_code)]

extern crate outcome_core as outcome;
extern crate rmp_serde as rmps;
extern crate serde;

use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
//use std::net::TcpListener::;
use std::io::prelude::*;
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

use crate::msg::coord_worker::{IntroduceCoordRequest, IntroduceCoordResponse, SignalRequest};
use crate::msg::*;
use crate::transport::WorkerDriverInterface;
use crate::WorkerDriver;
use crate::{error::Error, Result};

use outcome_core::distr::{NodeCommunication, Signal};
use outcome_core::Address;

/// Default address for the worker
pub const WORKER_ADDRESS: &str = "0.0.0.0:5922";

pub type WorkerId = u32;

/// Represents a single cluster node.
pub struct Worker {
    /// List of other workers in the cluster.
    pub comrades: Vec<Comrade>,
    /// Network driver
    pub driver: WorkerDriver,
    /// Whether the worker uses a password to authorize connecting comrade workers.
    pub use_auth: bool,
    /// Password used for new client authorization.
    pub passwd_list: Vec<String>,

    /// Simulation node running on this worker.
    pub sim_node: Option<outcome::distr::SimNode>,
}

impl Worker {
    pub fn new(my_addr: &str) -> Result<Worker> {
        Ok(Worker {
            comrades: vec![],
            driver: WorkerDriver::new(my_addr).unwrap(),
            use_auth: false,
            passwd_list: vec![],
            sim_node: None,
        })
    }
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

    // TODO
    pub fn handle_coordinator(&mut self) -> Result<()> {
        // unimplemented!();
        print!("waiting for coordinator to initiate connection... ");
        io::stdout().flush()?;

        let msg = self.driver.accept()?;
        println!("success!");
        println!("got message from central: {:?}", msg);

        let req: IntroduceCoordRequest = msg.unpack_payload().unwrap();

        let resp = Message::from_payload(
            IntroduceCoordResponse {
                error: "".to_string(),
            },
            false,
        )?;

        self.driver.connect_to_coord(&req.ip_addr, resp)?;

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
            sleep(Duration::from_millis(500));
            // println!("{:?}", self.driver.msg_read_central());

            // println!("{:?}", self.driver.msg_read_central());
            let msg = match self.driver.msg_read_central() {
                Ok(m) => m,
                Err(e) => {
                    println!("{:?}", e);
                    continue;
                }
            };
            println!("{:?}", msg);
            // handle_message(local_worker.clone(), msg, &mut in_stream, &mut out_stream);
        }
    }
}

/// Handle message from a new client (it can only be a RegisterClientRequest)
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
fn handle_message(
    worker_arc: Arc<Mutex<Worker>>,
    msg: Message,
    mut in_stream: &mut TcpStream,
    mut out_stream: &mut TcpStream,
) -> Result<()> {
    println!("Got a new Message: {}", &msg.kind);

    let payload = msg.payload;
    match msg.kind.as_str() {
        PING_REQUEST => handle_ping_request(payload, worker_arc)?,
        //        REGISTER_CLIENT_REQUEST => handle_data_transfer_request(payload, server_arc, stream),
        DATA_TRANSFER_REQUEST => handle_data_transfer_request(payload, worker_arc)?,
        DATA_PULL_REQUEST => handle_data_pull_request(payload, worker_arc)?,
        STATUS_REQUEST => handle_status_request(payload, worker_arc)?,
        // SIGNAL_REQUEST => handle_distr_msg_request(payload, worker_arc)?,
        _ => (),
    }
    Ok(())
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
    // client self-assigned id
    pub name: String,
    // ip address of the client
    pub ip_addr: SocketAddr,
    // password used by the worker
    pub passwd: String,
}

// TODO
pub fn handle_ping_request(payload: Vec<u8>, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    unimplemented!();
    // let req: PingRequest = match unpack_payload(&payload, false, None) {
    //     Some(p) => p,
    //     None => return,
    // };
    // let resp = PingResponse { bytes: req.bytes };
    // send_message(message_from_payload(resp, false), stream, None);
}
// TODO
pub fn handle_status_request(payload: Vec<u8>, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
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
pub fn handle_data_transfer_request(
    payload: Vec<u8>,
    server_arc: Arc<Mutex<Worker>>,
) -> Result<()> {
    let dtr: DataTransferRequest = unpack_payload(&payload, false, None)?;
    let mut data_pack = SimDataPack::new();
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
pub fn handle_data_pull_request(payload: Vec<u8>, server_arc: Arc<Mutex<Worker>>) -> Result<()> {
    let mut server = server_arc.lock().unwrap();
    //TODO
    //    let mut sim_model = &server.sim_model.clone();
    let mut sim_instance = &mut server.sim_node;
    let dpr: DataPullRequest = unpack_payload(&payload, false, None)?;
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

pub fn handle_distr_msg_request(payload: Vec<u8>, worker_arc: Arc<Mutex<Worker>>) -> Result<()> {
    println!("handling distr msg request");
    let distr_msg_req: SignalRequest = unpack_payload(&payload, false, None)?;
    let mut worker = worker_arc.lock().map_err(|e| Error::Other(e.to_string()))?;
    match distr_msg_req.signal {
        // Signal::InitializeNode((model, entities)) => {
        //     println!("{:?}", entities);
        //     let node = SimNode::from_model(&model, &entities).unwrap();
        //     worker.sim_node = Some(node);
        //     let resp = SignalResponse {
        //         distr_msg: Signal::EndOfMessages,
        //     };
        //     send_message(message_from_payload(resp, false), &mut stream_out, None).unwrap();
        // }
        Signal::StartProcessStep(event_queue) => {
            let mut node = worker.sim_node.as_mut().unwrap();
            // let entity_node_map = HashMap::new();
            // TODO
            // let mut addr_book = HashMap::new();
            // addr_book.insert(
            //     "0".to_string(),
            //     TcpStreamConnection {
            //         stream_in: stream_in.try_clone().unwrap(),
            //         stream_out: stream_out.try_clone().unwrap(),
            //     },
            // );
            // node.step(&entity_node_map, &mut addr_book);
        }
        _ => unimplemented!(),
    }
    Ok(())
}
