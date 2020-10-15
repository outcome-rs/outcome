/// This module implements the full driver set using `ZeroMQ` messaging
/// library. More specifically, the crate used is `libzmq-rs`. While it exposes
/// only a selected subset of the original library, it provides a slightly
/// nicer and more idiomatic API than the `rust-zmq` crate.
///
/// It would probably be a good idea to implement the full driver set using
/// `rust-zmq` and compare performance. Better knowledge of the library would
/// also likely help with optimizing for performance.
use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;

use libzmq::prelude::{
    BuildHeartbeating, BuildRecv, BuildSend, BuildSocket, Heartbeating, RecvMsg, SendMsg, Socket,
};
use libzmq::{ClientBuilder, Heartbeat, RoutingId, ServerBuilder, TcpAddr};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

use outcome::distr::{DistrError, Signal};
use outcome::IndexString;

use crate::driver::{ClientDriverInterface, CoordDriverInterface, WorkerDriverInterface};
use crate::msg::coord_worker::{IntroduceCoordRequest, IntroduceCoordResponse};
use crate::msg::{Message, RegisterClientRequest};
use crate::NetworkError;

use super::ServerDriverInterface;
use crate::server::{ClientId, SERVER_ADDRESS};
use crate::worker::WorkerId;
use libzmq::addr::Endpoint;
use std::sync::{Arc, Mutex};
use std::thread;

pub(crate) struct SymmetriConn {
    outgoing: libzmq::Client,
    incoming: libzmq::Server,
}

pub(crate) struct ClientDriver {
    outgoing: libzmq::Client,
    incoming: Arc<Mutex<libzmq::Server>>,
    in_queue: Arc<Mutex<VecDeque<Message>>>,
}
impl ClientDriver {
    pub fn connect_to_server(&self, addr: &str, msg: Option<Message>) -> Result<(), NetworkError> {
        println!("connect to server: {}", addr);
        self.outgoing.connect(new_endpoint(addr).unwrap()).unwrap();
        Ok(())
    }
}
impl ClientDriverInterface for ClientDriver {
    fn new(addr: Option<&str>) -> Result<ClientDriver, String> {
        let incoming = Arc::new(Mutex::new(
            ServerBuilder::new()
                .bind(new_endpoint(addr.unwrap_or("0.0.0.0:3213")).unwrap())
                .heartbeat(
                    Heartbeat::new(Duration::from_millis(200))
                        .add_timeout(Duration::from_millis(600)),
                )
                .build()
                .unwrap(),
        ));
        // thread::sleep(Duration::from_millis(100));
        let in_queue = Arc::new(Mutex::new(VecDeque::new()));

        let _incoming = incoming.clone();
        let _in_queue = in_queue.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(100));
            let msg = match _incoming.lock().unwrap().try_recv_msg() {
                Ok(m) => m,
                Err(_) => {
                    continue;
                }
            };
            let message = Message::from_bytes(msg.as_bytes()).unwrap();
            _in_queue.lock().unwrap().push_back(message);
        });
        // thread::sleep(Duration::from_millis(100));
        Ok(ClientDriver {
            outgoing: libzmq::ClientBuilder::new()
                .heartbeat(
                    Heartbeat::new(Duration::from_millis(200))
                        .add_timeout(Duration::from_millis(600)),
                )
                .build()
                .unwrap(),
            incoming,
            in_queue,
        })
    }

    fn my_addr(&self) -> String {
        endpoint_to_string(&self.incoming.lock().unwrap().last_endpoint().unwrap())
    }

    fn dial_server(&self, addr: &str, msg: Message) -> Result<(), NetworkError> {
        let temp_client = libzmq::ClientBuilder::new()
            .connect(new_endpoint(addr).unwrap())
            .build()
            .unwrap();
        thread::sleep(Duration::from_millis(300));
        temp_client.send(msg.pack()).unwrap();
        Ok(())
    }

    fn read(&self) -> Result<Message, NetworkError> {
        loop {
            if let Some(msg) = self.in_queue.lock().unwrap().pop_front() {
                return Ok(msg);
            }
            // thread::sleep(Duration::from_millis(100));
            continue;
        }
    }

    fn send(&self, message: Message) -> Result<(), NetworkError> {
        self.outgoing.send(message.pack()).unwrap();
        Ok(())
    }
}

pub(crate) struct ServerDriver {
    greeter: libzmq::Server,
    // pub(crate) server: libzmq::Server,
    clients: HashMap<u32, SymmetriConn>,
    port_count: u32,
}

impl ServerDriver {
    pub fn try_accept(&mut self) -> Result<(ClientId, Message), NetworkError> {
        let msg = match self.greeter.try_recv_msg() {
            Ok(m) => m,
            Err(e) => match e.kind() {
                libzmq::ErrorKind::WouldBlock => return Err(NetworkError::WouldBlock),
                _ => return Err(NetworkError::Other(e.to_string())),
            },
        };
        let message = Message::from_bytes(msg.as_bytes()).unwrap();
        let req: RegisterClientRequest = message.unpack_payload().unwrap();
        self.port_count += 1;
        let newport = format!("0.0.0.0:{}", self.port_count);
        // println!("{}", newport);
        self.clients.insert(
            self.port_count,
            SymmetriConn {
                incoming: ServerBuilder::new()
                    .bind(new_endpoint(&newport).unwrap())
                    .build()
                    .unwrap(),
                outgoing: ClientBuilder::new()
                    .connect(new_endpoint(&req.addr))
                    .build()
                    .unwrap(),
            },
        );
        Ok((
            self.port_count,
            Message::from_bytes(msg.as_bytes()).unwrap(),
        ))
    }
}
impl ServerDriverInterface for ServerDriver {
    fn new(addr: &str) -> Result<ServerDriver, String> {
        Ok(ServerDriver {
            greeter: ServerBuilder::new()
                .bind(new_endpoint(addr).unwrap())
                .build()
                .unwrap(),
            clients: HashMap::new(),
            port_count: 9222,
        })
    }
    fn read(&self, client_id: &ClientId) -> Result<Message, NetworkError> {
        // println!("reading from client: {}", client_id);
        let msg = self
            .clients
            .get(client_id)
            .unwrap()
            .incoming
            .recv_msg()
            .unwrap();
        let message = Message::from_bytes(msg.as_bytes()).unwrap();
        Ok(message)
        // let msg = self.clients.get(client_id).unwrap().recv_msg().unwrap();
        // let message = Message::from_bytes(msg.as_bytes()).unwrap();
        // Ok(message)
    }
    fn send(&mut self, client_id: &ClientId, message: Message) -> Result<(), NetworkError> {
        // unimplemented!()
        self.clients
            .get(client_id)
            .unwrap()
            .outgoing
            .send(message.pack())
            .unwrap();
        Ok(())
    }

    /// Broadcasts a message to all connected clients.
    fn broadcast(&mut self, message: Message) -> Result<(), NetworkError> {
        unimplemented!();
    }

    /// Accepts incoming client connection and assigns it a unique id. Returns
    /// both the id and the received message. Blocks until a new incoming
    /// connection is received.
    fn accept(&mut self) -> Result<(ClientId, Message), NetworkError> {
        let msg = match self.greeter.recv_msg() {
            Ok(m) => m,
            Err(e) => return Err(NetworkError::Other(e.to_string())),
        };
        let message = Message::from_bytes(msg.as_bytes()).unwrap();
        let req: RegisterClientRequest = message.unpack_payload().unwrap();
        self.port_count += 1;
        let newport = format!("0.0.0.0:{}", self.port_count);
        println!("{}", newport);
        self.clients.insert(
            self.port_count,
            SymmetriConn {
                incoming: ServerBuilder::new()
                    .bind(new_endpoint(&newport).unwrap())
                    .build()
                    .unwrap(),
                outgoing: ClientBuilder::new()
                    .connect(new_endpoint(&req.addr))
                    .build()
                    .unwrap(),
            },
        );
        Ok((
            self.port_count,
            Message::from_bytes(msg.as_bytes()).unwrap(),
        ))
    }
}

/// Basic networking interface for `Coord`.
pub(crate) struct CoordDriver {
    server: libzmq::Server,
}

// impl CoordDriver {
//     pub fn connect_worker(&mut self, addr: &str) -> Result<(), NetworkError> {
//
//         let tcp_addr = TcpAddr::from_str(&addr).unwrap();
//     }
// }

impl CoordDriverInterface for CoordDriver {
    fn new(addr: &str) -> Result<CoordDriver, String> {
        Ok(CoordDriver {
            server: ServerBuilder::new()
                .bind(new_endpoint(addr).unwrap())
                .build()
                .unwrap(),
        })
    }
    fn accept(&mut self) -> Result<(WorkerId, Message), NetworkError> {
        let msg = self.server.recv_msg().unwrap();
        let id = msg.routing_id().unwrap();
        let message = Message::from_bytes(msg.as_bytes()).unwrap();
        Ok((id.0, message))
    }
    fn connect_to_worker(&self, addr: &str, msg: Message) -> Result<(), NetworkError> {
        let client = libzmq::ClientBuilder::new().build().unwrap();
        client.connect(new_endpoint(addr).unwrap());
        thread::sleep(Duration::from_millis(100));
        client.send(msg.pack()).unwrap();
        Ok(())
    }

    fn msg_send_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<(), NetworkError> {
        self.server
            .route(msg.pack(), libzmq::RoutingId(*worker_id))
            .unwrap();
        Ok(())
    }

    fn msg_read_worker(&self, worker_id: &u32, msg: Message) -> Result<(), NetworkError> {
        unimplemented!()
    }
}

/// Networking interface for `Worker`.
///
/// ## TODO: consider implementing separate "message bus" for `Signal`s
///
/// The use of two separate "buses" could potentially eliminate the need
/// for a *type check* for each incoming `Signal`.
pub struct WorkerDriver {
    greeter: libzmq::Server,
    coord: libzmq::Client,
    comrades: HashMap<String, SymmetriConn>,
}

impl WorkerDriverInterface for WorkerDriver {
    /// Create a new worker driver using an address
    fn new(addr: &str) -> Result<WorkerDriver, String> {
        // let addr = TcpAddr::from_str(my_addr).unwrap();
        Ok(WorkerDriver {
            greeter: ServerBuilder::new()
                .bind(new_endpoint(addr).unwrap())
                .build()
                .unwrap(),
            coord: libzmq::Client::new().unwrap(),
            comrades: HashMap::new(),
        })
    }
    fn accept(&self) -> Result<Message, NetworkError> {
        let msg = Message::from_bytes(self.greeter.recv_msg().unwrap().as_bytes()).unwrap();
        Ok(msg)
    }
    fn connect_to_coord(&mut self, addr: &str, msg: Message) -> Result<(), NetworkError> {
        self.coord.connect(new_endpoint(addr).unwrap()).unwrap();
        thread::sleep(Duration::from_millis(100));
        self.coord.send(msg.pack()).unwrap();
        Ok(())
    }

    fn msg_read_central(&self) -> Result<Message, NetworkError> {
        let msg = self.coord.recv_msg().unwrap();
        let message = Message::from_bytes(msg.as_bytes()).unwrap();
        Ok(message)
    }
    fn msg_send_central(&self, msg: Message) -> Result<(), NetworkError> {
        self.coord
            .send(msg.pack())
            .map_err(|e| NetworkError::Other(e.to_string()))
    }

    fn msg_read_worker(&self, worker_id: u32) -> Result<Message, NetworkError> {
        unimplemented!()
    }
    fn msg_send_worker(&self, worker_id: u32, msg: Message) -> Result<(), NetworkError> {
        unimplemented!()
    }
}

impl outcome::distr::NodeCommunication<NetworkError> for WorkerDriver {
    fn sig_read_central(&mut self) -> Result<Signal, NetworkError> {
        let msg = self.coord.recv_msg().unwrap();
        let sig: Signal = Message::from_bytes(msg.as_bytes())
            .unwrap()
            .unpack_payload()
            .unwrap();

        // let mut de = Deserializer::new(msg.as_bytes());
        // let sig: Signal = match Deserialize::deserialize(&mut de) {
        //     Ok(m) => m,
        //     Err(e) => {
        //         println!("{}", e);
        //         return Err(NetworkError::Other("failed deserializing msg".to_string()));
        //     }
        // };

        Ok(sig)
    }
    fn sig_send_central(&mut self, signal: Signal) -> Result<(), NetworkError> {
        // let mut buf = Vec::new();
        // signal.serialize(&mut Serializer::new(&mut buf)).unwrap();
        let msg = Message::from_payload(signal, false).pack();
        self.coord.send(msg).unwrap();
        Ok(())
    }

    fn sig_read(&mut self) -> Result<(String, Signal), NetworkError> {
        unimplemented!()
    }

    fn sig_read_from(&mut self, node_id: &str) -> Result<Signal, NetworkError> {
        unimplemented!()
    }

    fn sig_send_to_node(&mut self, node_id: &str, signal: Signal) -> Result<(), NetworkError> {
        unimplemented!()
    }

    fn sig_send_to_entity(
        &mut self,
        entity_uid: (IndexString, IndexString),
    ) -> Result<(), NetworkError> {
        unimplemented!()
    }

    fn sig_broadcast(&mut self, signal: Signal) -> Result<(), NetworkError> {
        unimplemented!()
    }

    fn get_nodes(&mut self) -> Vec<String> {
        self.comrades.keys().map(|s| s.clone()).collect()
    }
}

/// Create a new endpoint using zmq notation, e.g. `udp://127.0.0.1:1234`.
/// If the prefix is no provided, it will default to `tcp`.
fn new_endpoint(s: &str) -> Result<Endpoint, String> {
    match s.find("://") {
        Some(index) => match &s[0..index] {
            "tcp" => {
                let addr = TcpAddr::from_str(&s[index + 3..]).unwrap();
                Ok(Endpoint::Tcp(addr))
            }
            "inproc" => {
                let addr = libzmq::InprocAddr::from_str(&s[index + 3..]).unwrap();
                Ok(Endpoint::Inproc(addr))
            }
            "udp" => {
                let addr = libzmq::UdpAddr::from_str(&s[index + 3..]).unwrap();
                Ok(Endpoint::Udp(addr))
            }
            "pgm" => {
                let addr = libzmq::PgmAddr::from_str(&s[index + 3..]).unwrap();
                Ok(Endpoint::Pgm(addr))
            }
            "epgm" => {
                let addr = libzmq::EpgmAddr::from_str(&s[index + 3..]).unwrap();
                Ok(Endpoint::Epgm(addr))
            }
            _ => unreachable!(),
        },
        None => {
            let addr = match TcpAddr::from_str(&s) {
                Ok(a) => a,
                Err(e) => {
                    println!("{}: {:?}", s, e);
                    panic!();
                }
            };
            Ok(Endpoint::Tcp(addr))
        }
    }
}
fn endpoint_to_string(endpoint: &Endpoint) -> String {
    match endpoint {
        Endpoint::Tcp(addr) => format!("tcp://{}", addr),
        Endpoint::Inproc(addr) => format!("inproc://{}", addr),
        Endpoint::Udp(addr) => format!("udp://{}", addr),
        Endpoint::Epgm(addr) => format!("pgm://{}", addr),
        Endpoint::Pgm(addr) => format!("epgm://{}", addr),
    }
}
