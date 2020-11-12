extern crate nng;

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use nng::{Protocol, Socket};

use crate::msg::{Message, RegisterClientRequest};
use crate::server::ClientId;
use crate::transport::{
    ClientDriverInterface, CoordDriverInterface, ServerDriverInterface, WorkerDriverInterface,
};
use crate::worker::WorkerId;
use crate::{error::Error, Result};

pub(crate) struct ClientDriver {
    my_addr: String,
    conn: Socket,
}
impl ClientDriver {
    pub fn connect_to_server(&self, addr: &str, msg: Option<Message>) -> Result<()> {
        println!("connect to server: {}", addr);
        self.conn.dial(&new_endpoint(addr))?;
        Ok(())
    }
}
impl ClientDriverInterface for ClientDriver {
    fn new(addr: Option<&str>) -> Result<ClientDriver> {
        let my_addr = String::from(addr.unwrap_or("tcp://0.0.0.0:3213"));
        let socket = Socket::new(Protocol::Pair0)?;
        socket.listen(&my_addr)?;
        Ok(ClientDriver {
            my_addr,
            // do dial?
            conn: socket,
        })
    }

    fn my_addr(&self) -> String {
        self.my_addr.clone()
    }

    fn dial_server(&self, addr: &str, msg: Message) -> Result<()> {
        let temp_client = Socket::new(Protocol::Req0)?;
        thread::sleep(Duration::from_millis(1000));
        temp_client.dial(&new_endpoint(addr))?;
        thread::sleep(Duration::from_millis(1000));
        temp_client.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        Ok(())
    }

    fn read(&self) -> Result<Message> {
        let msg = self.conn.recv()?;
        Ok(Message::from_bytes(msg.as_slice())?)
    }

    fn send(&self, message: Message) -> Result<()> {
        self.conn.send(&message.to_bytes()).map_err(|(_, e)| e)?;
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
    pub client_counter: u32,
}

impl ServerDriver {
    /// Non-blocking function that accepts an incoming connection from a client
    /// and performs the initial exchange.
    ///
    /// Initial exchange includes redirection to target pair socket port and
    /// potentially also authorization.
    pub fn try_accept(&mut self) -> Result<(ClientId, Message)> {
        let msg = match self.greeter.try_recv() {
            Ok(m) => m,
            Err(e) => {
                // println!("{:?}", e);
                return Err(Error::WouldBlock);
            }
        };
        let message = Message::from_bytes(msg.as_slice())?;
        let req: RegisterClientRequest = message.unpack_payload()?;
        self.client_counter += 1;
        let newport = format!("tcp://0.0.0.0:{}", self.client_counter);
        // println!("{}", newport);
        let socket = Socket::new(Protocol::Pair0)?;
        socket.listen(&newport).expect("couldn't listen on newport");
        println!("{}", &req.addr);
        socket.dial(&new_endpoint(&req.addr))?;
        self.clients.insert(self.client_counter, socket);
        Ok((self.client_counter, Message::from_bytes(msg.as_slice())?))
    }
}
impl ServerDriverInterface for ServerDriver {
    fn new(addr: &str) -> Result<ServerDriver> {
        let greeter = Socket::new(Protocol::Rep0)?;
        greeter.listen(&new_endpoint(addr))?;
        Ok(ServerDriver {
            greeter,
            clients: HashMap::new(),
            client_counter: 9223,
        })
    }
    fn read(&self, client_id: &ClientId) -> Result<Message> {
        let msg = self.clients.get(client_id).unwrap().recv()?;
        Ok(Message::from_bytes(msg.as_slice())?)
    }
    fn send(&mut self, client_id: &u32, message: Message) -> Result<()> {
        self.clients
            .get(client_id)
            .unwrap()
            .send(&message.to_bytes());
        Ok(())
    }

    /// Broadcasts a message to all connected clients.
    fn broadcast(&mut self, message: Message) -> Result<()> {
        unimplemented!();
    }

    /// Accepts incoming client connection and assigns it a unique id. Returns
    /// both the id and the received message. Blocks until a new incoming
    /// connection is received.
    fn accept(&mut self) -> Result<(u32, Message)> {
        let msg = match self.greeter.recv() {
            Ok(m) => m,
            Err(e) => return Err(Error::Other(e.to_string())),
        };
        let id = self.client_counter;
        self.client_counter += 1;
        Ok((id, Message::from_bytes(msg.as_slice())?))
    }
}

/// Basic networking interface for `Coord`.
pub(crate) struct CoordDriver {
    server: Socket,
}

// impl CoordDriver {
//     pub fn connect_worker(&mut self, addr: &str) -> Result<(), NetworkError> {
//
//         let tcp_addr = TcpAddr::from_str(&addr).unwrap();
//     }
// }

impl CoordDriverInterface for CoordDriver {
    fn new(addr: &str) -> Result<CoordDriver> {
        let server = Socket::new(Protocol::Pair0)?;
        server.listen(&new_endpoint(addr))?;
        Ok(CoordDriver { server })
    }
    fn accept(&mut self) -> Result<(WorkerId, Message)> {
        //unimplemented!();
        let msg = self.server.recv()?;
        let id = 12;
        let message = Message::from_bytes(msg.as_slice())?;
        Ok((id, message))
        // let id = msg.routing_id().unwrap();
        // let message = Message::from_bytes(msg.as_bytes()).unwrap();
        // Ok((id.0, message))
    }
    fn connect_to_worker(&self, addr: &str, msg: Message) -> Result<()> {
        let sock = Socket::new(Protocol::Pair0)?;
        //thread::sleep(Duration::from_millis(100));
        sock.dial(&new_endpoint(addr))?;
        sock.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        thread::sleep(Duration::from_millis(1000));

        Ok(())
    }

    fn msg_send_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<()> {
        self.server.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        Ok(())
        //unimplemented!()
        // self.server
        //     .route(msg.pack(), libzmq::RoutingId(*worker_id))
        //     .unwrap();
        // Ok(())
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
    greeter: Socket,
    coord: Socket,
    comrades: HashMap<String, SymmetriConn>,
}
impl WorkerDriverInterface for WorkerDriver {
    /// Create a new worker driver using an address
    fn new(addr: &str) -> Result<WorkerDriver> {
        // let addr = TcpAddr::from_str(my_addr).unwrap();
        let greeter = Socket::new(Protocol::Pair0)?;
        greeter.listen(&new_endpoint(addr))?;
        println!("started greeter listener at addr: {}", new_endpoint(addr));
        Ok(WorkerDriver {
            greeter,
            coord: Socket::new(Protocol::Pair0)?,
            comrades: HashMap::new(),
        })
    }
    fn accept(&self) -> Result<Message> {
        println!("waiting at accept func");
        let msg = self.greeter.recv()?;
        let message = Message::from_bytes(msg.as_slice())?;
        Ok(message)
    }
    fn connect_to_coord(&mut self, addr: &str, msg: Message) -> Result<()> {
        //unimplemented!();
        println!("connecting to coord at addr: {}", addr);
        self.coord.dial(&new_endpoint(addr))?;
        self.coord.send(&msg.to_bytes()).map_err(|(_, e)| e)?;
        // self.coord.connect(new_endpoint(addr).unwrap()).unwrap();
        // thread::sleep(Duration::from_millis(100));
        // self.coord.send(msg.pack()).unwrap();
        Ok(())
    }

    fn msg_read_central(&self) -> Result<Message> {
        let msg = self.coord.recv().unwrap();
        let message = Message::from_bytes(msg.as_slice()).unwrap();
        Ok(message)
    }
    fn msg_send_central(&self, msg: Message) -> Result<()> {
        self.coord
            .send(&msg.to_bytes())
            .map_err(|(_, e)| Error::Other(e.to_string()))
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
