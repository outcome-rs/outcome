//! This module implements the full driver set using `ZeroMQ` messaging
//! library. More specifically, the crate used is `rust-zmq`.

use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

use outcome::distr::{DistrError, Signal};
use outcome::StringId;

use crate::error::{Error, Result};
use crate::msg::coord_worker::{IntroduceCoordRequest, IntroduceCoordResponse};
use crate::msg::{Message, RegisterClientRequest, RegisterClientResponse};
use crate::server::{ClientId, SERVER_ADDRESS};
use crate::transport::{
    ClientDriverInterface, CoordDriverInterface, SocketInterface, WorkerDriverInterface,
};
use crate::worker::WorkerId;

use super::ServerDriverInterface;
use zmq::{PollEvents, SocketType};

pub(crate) struct ClientDriver {
    ctx: zmq::Context,
    conn: zmq::Socket,
}
impl ClientDriver {
    pub fn req_socket(&self) -> Result<ReqSocket> {
        Ok(ReqSocket {
            inner: self.ctx.socket(zmq::SocketType::REQ)?,
        })
    }
    pub fn connect_to_server(&self, addr: &str, msg: Option<Message>) -> Result<()> {
        println!("connect to server: {}", addr);
        self.conn.connect(&tcp_endpoint(addr))?;
        Ok(())
    }
    pub fn disconnect(&self) -> Result<()> {
        self.conn.disconnect("")?;
        Ok(())
    }
    pub fn try_read(&self) -> Result<Message> {
        let id = self.conn.recv_bytes(zmq::DONTWAIT)?;
        let msg = self.conn.recv_bytes(0).unwrap();
        let message = Message::from_bytes(&msg).unwrap();
        Ok(message)
    }
}
impl ClientDriverInterface for ClientDriver {
    fn new() -> Result<ClientDriver> {
        let ctx = zmq::Context::new();
        let conn = ctx.socket(zmq::SocketType::PAIR).unwrap();
        Ok(ClientDriver { ctx, conn })
    }

    fn my_addr(&self) -> String {
        self.conn.get_last_endpoint().unwrap().unwrap()
    }

    fn dial_server(&self, addr: &str, msg: Message) -> Result<()> {
        let temp_client = self.ctx.socket(zmq::SocketType::REQ).unwrap();
        //thread::sleep(Duration::from_millis(100));
        temp_client.connect(&tcp_endpoint(addr));
        temp_client.send(msg.pack(), 0).unwrap();
        Ok(())
    }

    fn read(&self) -> Result<Message> {
        let msg = self.conn.recv_bytes(0)?;
        let message = Message::from_bytes(&msg)?;
        Ok(message)
    }
    fn send(&self, message: Message) -> Result<()> {
        self.conn.send(message.pack(), 0)?;
        Ok(())
    }
}

pub struct PairSocket {
    inner: zmq::Socket,
}
impl SocketInterface for PairSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.bind(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.connect(&tcp_endpoint(addr))?)
    }
    fn read(&self) -> Result<Message> {
        let msg = self.inner.recv_bytes(0)?;
        let message = Message::from_bytes(&msg)?;
        Ok(message)
    }
    fn try_read(&self) -> Result<Message> {
        // println!("reading");
        let events = self.inner.get_events().unwrap();
        // if events.contains()
        let poll = self.inner.poll(PollEvents::POLLIN, 0)?;

        if poll == 0 {
            return Err(Error::WouldBlock);
        } else {
            let msg = self.inner.recv_bytes(0)?;
            let message = Message::from_bytes(&msg)?;
            Ok(message)
        }
    }
    fn send(&self, message: Message) -> Result<()> {
        self.inner.send(message.pack(), 0)?;
        Ok(())
    }
}

pub struct ReqSocket {
    inner: zmq::Socket,
}
impl SocketInterface for ReqSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.bind(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.connect(&tcp_endpoint(addr))?)
    }
    fn read(&self) -> Result<Message> {
        let msg = self.inner.recv_bytes(0)?;
        let message = Message::from_bytes(&msg)?;
        Ok(message)
    }
    fn try_read(&self) -> Result<Message> {
        let poll = self.inner.poll(PollEvents::POLLIN, 1)?;
        if poll == 0 {
            return Err(Error::WouldBlock);
        } else {
            let msg = self.inner.recv_bytes(0)?;
            let message = Message::from_bytes(&msg)?;
            Ok(message)
        }
    }
    fn send(&self, message: Message) -> Result<()> {
        self.inner.send(message.pack(), 0)?;
        Ok(())
    }
}
pub struct RepSocket {
    inner: zmq::Socket,
}
impl SocketInterface for RepSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.bind(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.connect(&tcp_endpoint(addr))?)
    }
    fn read(&self) -> Result<Message> {
        let msg = self.inner.recv_bytes(0)?;
        let message = Message::from_bytes(&msg)?;
        Ok(message)
    }
    fn try_read(&self) -> Result<Message> {
        let poll = self.inner.poll(PollEvents::POLLIN, 1)?;
        if poll == 0 {
            return Err(Error::WouldBlock);
        } else {
            let msg = self.inner.recv_bytes(0)?;
            let message = Message::from_bytes(&msg)?;
            Ok(message)
        }
    }
    fn send(&self, message: Message) -> Result<()> {
        self.inner.send(message.pack(), 0)?;
        Ok(())
    }
}

pub struct ServerDriver {
    ctx: zmq::Context,
    pub greeter: RepSocket,
    // clients: HashMap<u32, zmq::Socket>,
    pub port_count: u32,
}

impl ServerDriver {
    pub fn new_connection(&mut self) -> Result<PairSocket> {
        Ok(PairSocket {
            inner: self.ctx.socket(SocketType::PAIR)?,
        })
    }
    // pub fn try_accept(&mut self) -> Result<(ClientId, Message)> {
    //     // println!("{:?}", msg);
    //     // use std::convert::TryInto;
    //     // let id = u32::from_be_bytes(msg[1..].try_into().unwrap());
    //     let msg = self.greeter.recv_bytes(0)?;
    //     // println!("{:?}", msg);
    //     let message = Message::from_bytes(&msg).unwrap();
    //     let req: RegisterClientRequest = message.unpack_payload().unwrap();
    //     self.port_count += 1;
    //     let newport = format!("127.0.0.1:{}", self.port_count);
    //     println!("newport: {}", newport);
    //     let client_socket = self.ctx.socket(zmq::SocketType::PAIR).unwrap();
    //     client_socket.bind(&new_endpoint(&newport))?;
    //     // client_socket.connect(&new_endpoint(&req.addr))?;
    //     println!("req.addr: {}", req.addr);
    //     // self.clients.insert(self.port_count, client_socket);
    //
    //     let resp = RegisterClientResponse {
    //         //redirect: format!("192.168.2.106:{}", client_id),
    //         redirect: newport,
    //         error: String::new(),
    //     };
    //     self.greeter
    //         .send(Message::from_payload(resp, false)?.pack(), 0)?;
    //     println!("responded to client: {}", self.port_count);
    //
    //     Ok((self.port_count, message))
    // }
}
impl ServerDriver {
    pub fn new(addr: &str) -> Result<ServerDriver> {
        let ctx = zmq::Context::new();
        let greeter = RepSocket {
            inner: ctx.socket(zmq::SocketType::REP)?,
        };
        greeter.bind(&tcp_endpoint(addr));
        Ok(ServerDriver {
            ctx,
            greeter,
            // clients: HashMap::new(),
            port_count: 9222,
        })
    }
    // fn try_read(&self, client_id: &ClientId) -> Result<Message> {
    //     unimplemented!()
    //     // println!("reading from client: {}", client_id);
    //     // let poll = self
    //     //     // .clients
    //     //     .get(client_id)
    //     //     .unwrap()
    //     //     .poll(PollEvents::POLLIN, 100)?;
    //     //
    //     // if poll == 0 {
    //     //     return Err(Error::WouldBlock);
    //     // } else {
    //     //     let msg = self.clients.get(client_id).unwrap().recv_bytes(0)?;
    //     //     println!("{:?}", msg);
    //     //     let message = Message::from_bytes(&msg)?;
    //     //     Ok(message)
    //     // }
    //
    //     // let msg = self.clients.get(client_id).unwrap().recv_msg().unwrap();
    //     // let message = Message::from_bytes(msg.as_bytes()).unwrap();
    //     // Ok(message)
    // }
    // fn read(&self, client_id: &ClientId) -> Result<Message> {
    //     unimplemented!();
    //     // let msg = self.clients.get(client_id).unwrap().recv_bytes(0).unwrap();
    //     // println!("{:?}", msg);
    //     // let message = Message::from_bytes(&msg).unwrap();
    //     // Ok(message)
    //
    //     // let msg = self.clients.get(client_id).unwrap().recv_msg().unwrap();
    //     // let message = Message::from_bytes(msg.as_bytes()).unwrap();
    //     // Ok(message)
    // }
    // fn send(&mut self, client_id: &ClientId, message: Message) -> Result<()> {
    //     unimplemented!();
    //     // let client_sock = self.clients.get(client_id).unwrap();
    //     // client_sock.send(message.pack(), 0)?;
    //
    //     //self.clients
    //     //.get(client_id)
    //     //.unwrap()
    //     //.send(message.pack(), 0)
    //     //.unwrap();
    //     Ok(())
    // }
    //
    // /// Broadcasts a message to all connected clients.
    // fn broadcast(&mut self, message: Message) -> Result<()> {
    //     unimplemented!();
    // }
    //
    // /// Accepts incoming client connection and assigns it a unique id. Returns
    // /// both the id and the received message. Blocks until a new incoming
    // /// connection is received.
    // fn accept(&mut self) -> Result<(ClientId, Message)> {
    //     unimplemented!();
    //
    //     // let msg = match self.greeter.recv_bytes(0) {
    //     //     Ok(m) => m,
    //     //     Err(e) => return Err(Error::Other(e.to_string())),
    //     // };
    //     // let message = Message::from_bytes(&msg).unwrap();
    //     // let req: RegisterClientRequest = message.unpack_payload().unwrap();
    //     // self.port_count += 1;
    //     // let newport = format!("0.0.0.0:{}", self.port_count);
    //     // println!("{}", newport);
    //     // let client_socket = self.ctx.socket(zmq::SocketType::ROUTER).unwrap();
    //     // client_socket.bind(&new_endpoint(&newport)).unwrap();
    //     // self.clients.insert(self.port_count, client_socket);
    //     // Ok((self.port_count, message))
    // }
}

/// Basic networking interface for `Coord`.
pub(crate) struct CoordDriver {
    conn: zmq::Socket,
}

// impl CoordDriver {
//     pub fn connect_worker(&mut self, addr: &str) -> Result<(), NetworkError> {
//
//         let tcp_addr = TcpAddr::from_str(&addr).unwrap();
//     }
// }

impl CoordDriverInterface for CoordDriver {
    fn new(addr: &str) -> Result<CoordDriver> {
        let ctx = zmq::Context::new();
        let conn = ctx.socket(zmq::SocketType::ROUTER).unwrap();
        Ok(CoordDriver { conn })
    }

    fn accept(&mut self) -> Result<(WorkerId, Message)> {
        unimplemented!();
        //let msg = self.server.recv_msg().unwrap();
        //let id = msg.routing_id().unwrap();
        //let message = Message::from_bytes(msg.as_bytes()).unwrap();
        //Ok((id.0, message))
    }
    fn connect_to_worker(&self, addr: &str, msg: Message) -> Result<()> {
        unimplemented!();
        //let client = libzmq::ClientBuilder::new().build().unwrap();
        //client.connect(new_endpoint(addr).unwrap());
        //thread::sleep(Duration::from_millis(100));
        //client.send(msg.pack()).unwrap();
        //Ok(())
    }

    fn msg_send_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<()> {
        unimplemented!();
        //self.server
        //.route(msg.pack(), libzmq::RoutingId(*worker_id))
        //.unwrap();
        //Ok(())
    }

    fn msg_read_worker(&self, worker_id: &u32, msg: Message) -> Result<()> {
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
    greeter: zmq::Socket,
    coord: zmq::Socket,
    comrades: HashMap<String, (zmq::Socket, zmq::Socket)>,
}

impl WorkerDriverInterface for WorkerDriver {
    /// Create a new worker driver using an address
    fn new(addr: &str) -> Result<WorkerDriver> {
        let ctx = zmq::Context::new();
        let greeter = ctx.socket(zmq::SocketType::ROUTER).unwrap();
        greeter.bind(&tcp_endpoint(addr)).unwrap();
        let coord = ctx.socket(zmq::SocketType::DEALER).unwrap();
        Ok(WorkerDriver {
            greeter,
            coord,
            comrades: HashMap::new(),
        })
    }
    fn accept(&self) -> Result<Message> {
        unimplemented!();
        //let msg = Message::from_bytes(self.greeter.recv_msg().unwrap().as_bytes()).unwrap();
        //Ok(msg)
    }
    fn connect_to_coord(&mut self, addr: &str, msg: Message) -> Result<()> {
        unimplemented!();
        //self.coord.connect(new_endpoint(addr).unwrap()).unwrap();
        //thread::sleep(Duration::from_millis(100));
        //self.coord.send(msg.pack()).unwrap();
        //Ok(())
    }

    fn msg_read_central(&self) -> Result<Message> {
        unimplemented!();
        //let msg = self.coord.recv_msg().unwrap();
        //let message = Message::from_bytes(msg.as_bytes()).unwrap();
        //Ok(message)
    }
    fn msg_send_central(&self, msg: Message) -> Result<()> {
        unimplemented!();
        //self.coord
        //.send(msg.pack())
        //.map_err(|e| NetworkError::Other(e.to_string()))
    }

    fn msg_read_worker(&self, worker_id: u32) -> Result<Message> {
        unimplemented!()
    }
    fn msg_send_worker(&self, worker_id: u32, msg: Message) -> Result<()> {
        unimplemented!()
    }
}

// impl outcome::distr::NodeCommunication for WorkerDriver {
//     fn sig_read_central(&mut self) -> Result<Signal> {
//         unimplemented!();
//         //let msg = self.coord.recv_msg().unwrap();
//         //let sig: Signal = Message::from_bytes(msg.as_bytes())
//         //.unwrap()
//         //.unpack_payload()
//         //.unwrap();
//         //Ok(sig)
//
//         //// let mut de = Deserializer::new(msg.as_bytes());
//         //// let sig: Signal = match Deserialize::deserialize(&mut de) {
//         ////     Ok(m) => m,
//         ////     Err(e) => {
//         ////         println!("{}", e);
//         ////         return Err(NetworkError::Other("failed deserializing msg".to_string()));
//         ////     }
//         //// };
//     }
//     fn sig_send_central(&mut self, signal: Signal) -> Result<()> {
//         unimplemented!();
//         //// let mut buf = Vec::new();
//         //// signal.serialize(&mut Serializer::new(&mut buf)).unwrap();
//         //let msg = Message::from_payload(signal, false).pack();
//         //self.coord.send(msg).unwrap();
//         //Ok(())
//     }
//
//     fn sig_read(&mut self) -> Result<(String, Signal)> {
//         unimplemented!()
//     }
//
//     fn sig_read_from(&mut self, node_id: &str) -> Result<Signal> {
//         unimplemented!()
//     }
//
//     fn sig_send_to_node(&mut self, node_id: &str, signal: Signal) -> Result<()> {
//         unimplemented!()
//     }
//
//     fn sig_send_to_entity(&mut self, entity_uid: (StringIndex, StringIndex)) -> Result<()> {
//         unimplemented!()
//     }
//
//     fn sig_broadcast(&mut self, signal: Signal) -> Result<()> {
//         unimplemented!()
//     }
//
//     fn get_nodes(&mut self) -> Vec<String> {
//         self.comrades.keys().map(|s| s.clone()).collect()
//     }
// }

/// Create a valid tcp address that includes the prefix.
pub(crate) fn tcp_endpoint(s: &str) -> String {
    if s.contains("://") {
        s.to_string()
    } else {
        format!("tcp://{}", s)
    }
}
