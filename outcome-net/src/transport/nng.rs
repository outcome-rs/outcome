extern crate nng;

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use nng::{Protocol, Socket};

use crate::util::tcp_endpoint;

use crate::msg::{Message, RegisterClientRequest};
use crate::server::ClientId;
use crate::transport::{
    ClientDriverInterface, CoordDriverInterface, ServerDriverInterface, SocketInterface,
    WorkerDriverInterface,
};
use crate::worker::WorkerId;
use crate::{error::Error, Result};

pub struct PairSocket {
    inner: Socket,
}
impl PairSocket {
    pub fn last_endpoint(&self) -> String {
        unimplemented!()
        // self.inner.end().unwrap().unwrap()
    }
}

impl SocketInterface for PairSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.listen(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.dial(&tcp_endpoint(addr))?)
    }
    fn disconnect(&self, addr: &str) -> Result<()> {
        self.inner.close();
        Ok(())
    }
    fn read(&self) -> Result<Vec<u8>> {
        let bytes = self.inner.recv()?.to_vec();
        Ok(bytes)
    }
    fn try_read(&self, timeout: Option<u32>) -> Result<Vec<u8>> {
        // unimplemented!()
        let bytes = self.inner.try_recv()?.to_vec();
        Ok(bytes)
        // let events = self.inner.get_events().unwrap();
        // let poll = self
        //     .inner
        //     .poll(PollEvents::POLLIN, timeout.unwrap_or(0) as i64)?;
        // if poll == 0 {
        //     return Err(Error::WouldBlock);
        // } else {
        //     let bytes = self.inner.recv_bytes(0)?;
        //     Ok(bytes)
        // }
    }
    fn send(&self, bytes: Vec<u8>) -> Result<()> {
        self.inner.send(&bytes).map_err(|(_, e)| e)?;
        Ok(())
    }

    fn read_msg(&self) -> Result<Message> {
        let bytes = self.read()?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn try_read_msg(&self, timeout: Option<u32>) -> Result<Message> {
        let bytes = self.try_read(timeout)?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        self.send(msg.to_bytes())
    }
}

pub struct ReqSocket {
    inner: Socket,
}

pub struct RepSocket {
    inner: Socket,
}

impl SocketInterface for RepSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.listen(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.dial(&tcp_endpoint(addr))?)
    }
    fn disconnect(&self, addr: &str) -> Result<()> {
        self.inner.close();
        Ok(())
    }
    fn read(&self) -> Result<Vec<u8>> {
        let bytes = self.inner.recv()?.to_vec();
        Ok(bytes)
    }
    fn try_read(&self, timeout: Option<u32>) -> Result<Vec<u8>> {
        // unimplemented!()
        // debug!("starting try_read");
        let bytes = self.inner.try_recv()?.to_vec();
        // debug!("finished try_read");
        Ok(bytes)
        // let poll = self
        //     .inner
        //     .poll(PollEvents::POLLIN, timeout.unwrap_or(1) as i64)?;
        // if poll == 0 {
        //     return Err(Error::WouldBlock);
        // } else {
        //     let bytes = self.inner.recv_bytes(0)?;
        //     Ok(bytes)
        // }
    }
    fn send(&self, bytes: Vec<u8>) -> Result<()> {
        self.inner.send(&bytes).map_err(|(_, e)| e)?;
        Ok(())
    }

    fn read_msg(&self) -> Result<Message> {
        let bytes = self.read()?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn try_read_msg(&self, timeout: Option<u32>) -> Result<Message> {
        let bytes = self.try_read(timeout)?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        self.send(msg.to_bytes())
    }
}

impl SocketInterface for ReqSocket {
    fn bind(&self, addr: &str) -> Result<()> {
        Ok(self.inner.listen(&tcp_endpoint(addr))?)
    }
    fn connect(&self, addr: &str) -> Result<()> {
        Ok(self.inner.dial(&tcp_endpoint(addr))?)
    }
    fn disconnect(&self, addr: &str) -> Result<()> {
        self.inner.close();
        Ok(())
    }
    fn read(&self) -> Result<Vec<u8>> {
        debug!("starting read");
        let bytes = self.inner.recv()?.to_vec();
        debug!("finished read");
        Ok(bytes)
    }
    fn try_read(&self, timeout: Option<u32>) -> Result<Vec<u8>> {
        unimplemented!()
        // let poll = self
        //     .inner
        //     .poll(PollEvents::POLLIN, timeout.unwrap_or(1) as i64)?;
        // if poll == 0 {
        //     return Err(Error::WouldBlock);
        // } else {
        //     let bytes = self.inner.recv_bytes(0)?;
        //     Ok(bytes)
        // }
    }
    fn send(&self, bytes: Vec<u8>) -> Result<()> {
        debug!("starting send");
        self.inner.send(&bytes).map_err(|(_, e)| e)?;
        debug!("finished send");
        Ok(())
    }
    fn read_msg(&self) -> Result<Message> {
        let bytes = self.read()?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn try_read_msg(&self, timeout: Option<u32>) -> Result<Message> {
        let bytes = self.try_read(timeout)?;
        let msg = Message::from_bytes(&bytes)?;
        Ok(msg)
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        self.send(msg.to_bytes())
    }
}

pub(crate) struct ClientDriver {
    // my_addr: String,
    conn: ReqSocket,
}
impl ClientDriver {
    pub fn req_socket(&self) -> Result<ReqSocket> {
        // let my_addr = String::from("tcp://0.0.0.0:3213");
        // let socket = ReqSocket {
        //     inner: Socket::new(Protocol::Req0)?,
        // };
        // socket.bind(&my_addr)?;
        // Ok(socket)

        Ok(ReqSocket {
            inner: Socket::new(Protocol::Req0)?,
        })
    }

    pub fn connect_to_server(&self, addr: &str, msg: Option<Message>) -> Result<()> {
        println!("connect to server: {}", addr);
        self.conn.connect(&new_endpoint(addr))?;
        Ok(())
    }
    pub fn disconnect(&self) -> Result<()> {
        self.conn.disconnect("")?;
        Ok(())
    }
}
impl ClientDriverInterface for ClientDriver {
    fn new() -> Result<ClientDriver> {
        // let my_addr = String::from(addr.unwrap_or("tcp://0.0.0.0:3213"));
        let socket = ReqSocket {
            inner: Socket::new(Protocol::Req0)?,
        };
        // socket.bind(&my_addr)?;
        Ok(ClientDriver { conn: socket })
    }

    fn my_addr(&self) -> String {
        unimplemented!()
    }

    fn dial_server(&self, addr: &str, msg: Message) -> Result<()> {
        let temp_client = Socket::new(Protocol::Req0)?;
        // thread::sleep(Duration::from_millis(1000));
        temp_client.dial(&new_endpoint(addr))?;
        // thread::sleep(Duration::from_millis(1000));
        temp_client.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        Ok(())
    }

    fn read(&self) -> Result<Message> {
        let msg = self.conn.read()?;
        Ok(Message::from_bytes(msg.as_slice())?)
    }

    fn send(&self, message: Message) -> Result<()> {
        self.conn.send(message.to_bytes())?;
        Ok(())
    }
}

/// Basic networking interface for `Server`.
///
/// Server's main job is keeping track of the connected `Client`s and handling
/// any requests they may send it's way.
pub struct ServerDriver {
    /// Public-facing greeter will listen to incoming clients and point them
    /// to dedicated client sockets
    pub greeter: RepSocket,
    // /// Map of clients by id to their respective connection points
    // pub(crate) clients: HashMap<u32, Socket>,
    /// Counter used for assigning client ids
    pub port_count: u32,
}

impl ServerDriver {
    pub fn new(addr: &str) -> Result<ServerDriver> {
        let greeter = RepSocket {
            inner: Socket::new(Protocol::Rep0)?,
        };
        greeter.bind(&tcp_endpoint(addr));
        Ok(ServerDriver {
            greeter,
            // clients: HashMap::new(),
            port_count: 9222,
        })
    }
    pub fn new_connection(&mut self) -> Result<PairSocket> {
        Ok(PairSocket {
            inner: Socket::new(Protocol::Pair0)?,
        })
    }

    /// Non-blocking function that accepts an incoming connection from a client
    /// and performs the initial exchange.
    ///
    /// Initial exchange includes redirection to target pair socket port and
    /// potentially also authorization.
    pub fn try_accept(&mut self) -> Result<(ClientId, Message)> {
        unimplemented!()
        // let msg = match self.greeter.try_recv() {
        //     Ok(m) => m,
        //     Err(e) => {
        //         // println!("{:?}", e);
        //         return Err(Error::WouldBlock);
        //     }
        // };
        // let message = Message::from_bytes(msg.as_slice())?;
        // let req: RegisterClientRequest = message.unpack_payload()?;
        // self.client_counter += 1;
        // let newport = format!("tcp://0.0.0.0:{}", self.client_counter);
        // // println!("{}", newport);
        // let socket = Socket::new(Protocol::Pair0)?;
        // socket.listen(&newport).expect("couldn't listen on newport");
        // println!("{:?}", &req.addr);
        // socket.dial(&new_endpoint(&req.addr))?;
        // self.clients.insert(self.client_counter, socket);
        // Ok((self.client_counter, Message::from_bytes(msg.as_slice())?))
    }
}

// impl ServerDriverInterface for ServerDriver {
//     fn new(addr: &str) -> Result<ServerDriver> {
//         // let greeter = Socket::new(Protocol::Rep0)?;
//         let greeter = RepSocket {
//             inner: Socket::new(Protocol::Rep0)?,
//         };
//         greeter.bind(&new_endpoint(addr))?;
//         Ok(ServerDriver {
//             greeter,
//             // clients: HashMap::new(),
//             client_counter: 9223,
//         })
//     }
//     fn read(&self, client_id: &ClientId) -> Result<Message> {
//         let msg = self.clients.get(client_id).unwrap().recv()?;
//         Ok(Message::from_bytes(msg.as_slice())?)
//     }
//
//     fn try_read(&self, client_id: &u32) -> Result<Message> {
//         unimplemented!()
//     }
//
//     fn send(&mut self, client_id: &u32, message: Message) -> Result<()> {
//         self.clients
//             .get(client_id)
//             .unwrap()
//             .send(&message.to_bytes());
//         Ok(())
//     }
//
//     /// Broadcasts a message to all connected clients.
//     fn broadcast(&mut self, message: Message) -> Result<()> {
//         unimplemented!();
//     }
//
//     /// Accepts incoming client connection and assigns it a unique id. Returns
//     /// both the id and the received message. Blocks until a new incoming
//     /// connection is received.
//     fn accept(&mut self) -> Result<(u32, Message)> {
//         let msg = match self.greeter.recv() {
//             Ok(m) => m,
//             Err(e) => return Err(Error::Other(e.to_string())),
//         };
//         let id = self.client_counter;
//         self.client_counter += 1;
//         Ok((id, Message::from_bytes(msg.as_slice())?))
//     }
// }

/// Basic networking interface for `Coord`.
pub(crate) struct CoordDriver {
    // server: Socket,
    pub greeter: RepSocket,
    pub inviter: ReqSocket,
}

impl CoordDriver {
    // pub fn connect_worker(&mut self, addr: &str) -> Result<(), NetworkError> {
    //
    //     let tcp_addr = TcpAddr::from_str(&addr).unwrap();
    // }
    pub fn new_pair_socket(&self) -> Result<PairSocket> {
        Ok(PairSocket {
            inner: Socket::new(Protocol::Pair0)?,
        })
    }
}

impl CoordDriverInterface for CoordDriver {
    fn new(addr: &str) -> Result<CoordDriver> {
        let server = Socket::new(Protocol::Pair0)?;
        server.listen(&new_endpoint(addr))?;
        Ok(CoordDriver {
            greeter: RepSocket {
                inner: Socket::new(Protocol::Rep0)?,
            },
            inviter: ReqSocket {
                inner: Socket::new(Protocol::Req0)?,
            },
        })
    }
    fn accept(&mut self) -> Result<(WorkerId, Message)> {
        unimplemented!();

        // let msg = self.server.recv()?;
        // let id = 12;
        // let message = Message::from_bytes(msg.as_slice())?;
        // Ok((id, message))
        // // let id = msg.routing_id().unwrap();
        // // let message = Message::from_bytes(msg.as_bytes()).unwrap();
        // // Ok((id.0, message))
    }
    fn connect_to_worker(&self, addr: &str, msg: Message) -> Result<()> {
        unimplemented!();

        // let sock = Socket::new(Protocol::Pair0)?;
        // //thread::sleep(Duration::from_millis(100));
        // sock.dial(&new_endpoint(addr))?;
        // sock.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        // thread::sleep(Duration::from_millis(1000));
        //
        // Ok(())
    }

    fn msg_send_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<()> {
        unimplemented!();

        // self.server.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        // Ok(())
        // //unimplemented!()
        // // self.server
        // //     .route(msg.pack(), libzmq::RoutingId(*worker_id))
        // //     .unwrap();
        // // Ok(())
    }

    fn msg_read_worker(&self, worker_id: &u32, msg: Message) -> Result<()> {
        unimplemented!()
    }
}

pub(crate) struct SymmetriConn {
    outgoing: Socket,
    incoming: Socket,
}

pub struct WorkerDriver {
    pub my_addr: String,
    pub greeter: RepSocket,
    pub inviter: ReqSocket,
    pub coord: PairSocket,
    comrades: HashMap<u32, PairSocket>,
}
impl WorkerDriverInterface for WorkerDriver {
    /// Create a new worker driver using an address
    fn new(addr: &str) -> Result<WorkerDriver> {
        let greeter = RepSocket {
            inner: Socket::new(Protocol::Rep0)?,
        };
        greeter.bind(&tcp_endpoint(addr))?;

        let inviter = ReqSocket {
            inner: Socket::new(Protocol::Req0)?,
        };
        inviter.bind(&tcp_endpoint(&format!("{}1", addr)))?;

        let coord = PairSocket {
            inner: Socket::new(Protocol::Pair0)?,
        };
        Ok(WorkerDriver {
            my_addr: addr.to_string(),
            greeter,
            inviter,
            coord,
            comrades: HashMap::new(),
        })
    }
    fn accept(&self) -> Result<Message> {
        println!("waiting at accept func");
        let msg = self.greeter.read()?;
        let message = Message::from_bytes(msg.as_slice())?;
        Ok(message)
    }
    fn connect_to_coord(&mut self, addr: &str, msg: Message) -> Result<()> {
        //unimplemented!();
        println!("connecting to coord at addr: {}", addr);
        self.coord.connect(&new_endpoint(addr))?;
        self.coord.send(msg.to_bytes())?;
        // self.coord.connect(new_endpoint(addr).unwrap()).unwrap();
        // thread::sleep(Duration::from_millis(100));
        // self.coord.send(msg.pack()).unwrap();
        Ok(())
    }

    fn msg_read_central(&self) -> Result<Message> {
        let msg = self.coord.read()?;
        let message = Message::from_bytes(msg.as_slice())?;
        Ok(message)
    }
    fn msg_send_central(&self, msg: Message) -> Result<()> {
        self.coord.send(msg.to_bytes())
    }

    fn msg_read_worker(&self, worker_id: u32) -> Result<Message> {
        unimplemented!()
    }
    fn msg_send_worker(&self, worker_id: u32, msg: Message) -> Result<()> {
        unimplemented!()
    }
}

fn new_endpoint(addr: &str) -> String {
    if addr.contains("://") {
        return addr.to_string();
    } else {
        return format!("tcp://{}", addr);
    }
}
