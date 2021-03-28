extern crate outcome_core as outcome;

use std::collections::HashMap;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use outcome::{Address, Sim, SimModel, VarType};

use crate::msg::*;
use crate::{Coord, Worker};

//use crate::coord::CoordNetwork;
use crate::service::Service;
use crate::socket::{Encoding, Socket, SocketConfig, SocketEvent, SocketType, Transport};
use crate::{error::Error, Result};
use fnv::FnvHashMap;
use id_pool::IdPool;
use outcome_core::{arraystring, StringId};
use std::convert::{TryFrom, TryInto};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{current, sleep};

pub const SERVER_ADDRESS: &str = "0.0.0.0:9124";
pub const GREETER_ADDRESS: &str = "0.0.0.0:9123";

pub const CLIENT_KEEP_ALIVE_MILLIS: usize = 3000;

pub type ClientId = u32;

/// High-level representation of a simulation interface.
pub enum SimConnection {
    Local(Sim),
    ClusterCoord(Coord),
    ClusterWorker(Worker),
}

/// Connected client as seen by a server.
pub struct Client {
    /// Unique id assigned at registration.
    pub id: ClientId,
    /// IP address of the client
    pub addr: String,
    /// Connection interface
    pub connection: Socket,

    /// Blocking client has to explicitly agree to let server continue to next turn,
    /// while non-blocking client is more of a passive observer
    pub is_blocking: bool,
    /// Furthest simulation step client has announced it's ready to proceed to.
    /// If this is bigger than the current step that client counts as
    /// ready for processing to next common furthest step.
    pub furthest_step: usize,

    /// Client-specific keepalive value, if none server config value applies
    pub keepalive: Option<Duration>,
    pub time_since_last_event: Duration,

    /// Authentication pair used by the client
    pub auth_pair: Option<(String, String)>,
    /// Client self-assigned name
    pub name: String,

    /// List of scheduled data transfers
    pub scheduled_dts: FnvHashMap<StringId, Vec<DataTransferRequest>>,

    pub order_store: FnvHashMap<u32, Vec<Address>>,
    pub order_id_pool: IdPool,
}

/// Configuration settings for server.
pub struct ServerConfig {
    /// Name of the server
    pub name: String,
    /// Description of the server
    pub description: String,

    /// Time since last traffic from any client until server is shutdown,
    /// set to none to keep alive forever
    pub self_keepalive: Option<Duration>,
    /// Time between polls in the main loop
    pub poll_wait: Duration,
    /// Delay between polling for new incoming client connections
    pub accept_delay: Duration,

    /// Time since last traffic from client until connection is terminated
    pub client_keepalive: Option<Duration>,
    /// Compress outgoing messages
    pub use_compression: bool,

    /// Whether to require authorization of incoming clients
    pub use_auth: bool,
    /// User and password pairs for client authorization
    pub auth_pairs: Vec<(String, String)>,

    /// List of transports supported for client connections
    pub transports: Vec<Transport>,
    /// List of encodings supported for client connections
    pub encodings: Vec<Encoding>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            name: "".to_string(),
            description: "".to_string(),
            self_keepalive: None,
            poll_wait: Duration::from_millis(1),
            accept_delay: Duration::from_millis(200),

            client_keepalive: Some(Duration::from_secs(4)),
            use_compression: false,

            use_auth: false,
            auth_pairs: Vec::new(),

            transports: vec![
                Transport::Tcp,
                #[cfg(feature = "zmq_transport")]
                Transport::Zmq,
            ],
            encodings: vec![
                Encoding::Bincode,
                #[cfg(feature = "msgpack_encoding")]
                Encoding::MsgPack,
            ],
        }
    }
}

// TODO add an optional http interface to the server as a crate feature
/// Connection entry point for clients.
///
/// # Network interface overview
///
/// Server's main job is keeping track of the connected `Client`s and handling
/// any requests they may send it's way. It also provides a pipe-like, one-way
/// communication for fast transport of queried data.
///
/// # Listening to client connections
///
/// Server exposes a single stable listener at a known port. Any clients that
/// wish to connect have to send a proper request to that main address. The
/// `accept` function is used to accept new incoming client connections.
/// Here the client is assigned a unique id. Response includes a new address
/// to which client should connect.
///
/// # Initiating client connections
///
/// Server is able not only to receive from, but also to initiate connections
/// to clients. Sent connection request includes the socket address that the
/// client should connect to.
pub struct Server {
    /// Server configuration
    pub config: ServerConfig,

    /// Connection with the simulation
    sim: SimConnection,
    /// Outward facing sockets
    pub greeters: Vec<Socket>,
    /// Counter used for assigning client ids
    pub port_count: u32,

    /// List of clients
    pub clients: HashMap<ClientId, Client>,
    /// Time since creation of this server
    pub uptime: Duration,

    /// Time since last message received
    time_since_last_msg: Duration,
    /// Time since last new client connection accepted
    time_since_last_accept: Duration,

    pub services: Vec<Service>,
}

impl Server {
    /// Creates new server using provided address and default config.
    pub fn new(addr: &str, sim: SimConnection) -> Self {
        Self::new_with_config(addr, ServerConfig::default(), sim)
    }

    /// Creates new server at available localhost port using default config.
    pub fn new_at_any(sim: SimConnection) -> Self {
        Self::new_at_any_with_config(ServerConfig::default(), sim)
    }

    /// Creates a new server at available localhost port using provided config.
    pub fn new_at_any_with_config(config: ServerConfig, sim: SimConnection) -> Self {
        Self::new_with_config("0.0.0.0:0", config, sim)
    }

    /// Creates a new server using provided address and config.
    pub fn new_with_config(addr: &str, config: ServerConfig, sim: SimConnection) -> Self {
        let address: SocketAddr = addr.parse().unwrap();
        let mut port = address.port();
        let mut greeters = Vec::new();
        for transport in &config.transports {
            for encoding in &config.encodings {
                let greeter_config = SocketConfig {
                    type_: SocketType::Pair,
                    encoding: *encoding,
                    ..Default::default()
                };
                greeters.push(
                    Socket::bind_with_config(
                        SocketAddr::new(address.ip(), port).to_string().as_str(),
                        *transport,
                        greeter_config,
                    )
                    .unwrap(),
                );
                info!(
                    "listening on: {}:{} (transport: {:?}, encoding: {:?})",
                    address.ip(),
                    port,
                    transport,
                    encoding
                );
                port = port + 1;
            }
        }

        Self {
            sim,
            config,
            // TODO select transport based on config's transport list
            greeters,
            // TODO make this easier to find by defining it as a constant
            port_count: 9223,
            clients: Default::default(),
            uptime: Default::default(),
            time_since_last_msg: Default::default(),
            time_since_last_accept: Default::default(),
            services: vec![],
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        match &mut self.sim {
            SimConnection::Local(sim) => {
                // start the service processes
                for service_model in &sim.model.services {
                    info!("starting service: {}", service_model.name);
                    let service = Service::start_from_model(service_model.clone())?;
                    self.services.push(service);
                }
            }
            _ => (),
        }

        Ok(())
    }

    // TODO allow for client reconnect using the same server-side connection
    // TODO process less important tasks less frequently
    /// Main server polling function.
    ///
    /// Contains all the operations needed to be performed repeatedly for the
    /// server to function properly, including handling new client connections,
    /// managing existing ones, processing incoming events and keeping track
    /// of keepalive times for both the server and all the connected clients.
    ///
    /// In the case of a coord server, the coord poll is also called.
    pub fn manual_poll(&mut self) -> Result<()> {
        // handle the server's keepalive mechanism
        self.time_since_last_msg += self.config.poll_wait;
        if let Some(self_keepalive) = self.config.self_keepalive {
            if self.time_since_last_msg >= self_keepalive {
                return Err(Error::ServerKeepaliveLimitReached(
                    self_keepalive.as_millis() as u32,
                ));
            }
        }

        // TODO implement time setting for monitoring every n-th poll
        // monitor services
        for service in &mut self.services {
            service.monitor();
        }

        // handle new incoming clients
        if self.time_since_last_accept >= self.config.accept_delay {
            self.time_since_last_accept = Duration::from_millis(0);
            if let Err(e) = self.try_accept_client(true) {
                match e {
                    Error::WouldBlock => (),
                    _ => debug!("{:?}", e),
                }
            }
        }

        // handle idle clients
        let mut clients_to_remove = Vec::new();
        for (client_id, client) in &mut self.clients {
            client.time_since_last_event += self.config.poll_wait;
            if let Some(keepalive) = client.keepalive {
                if client.time_since_last_event > keepalive {
                    clients_to_remove.push(*client_id);
                }
            }
        }
        for client_id in clients_to_remove {
            info!("removing idle client: {}", client_id);
            self.clients
                .get_mut(&client_id)
                .unwrap()
                .connection
                .disconnect(None);
            self.clients.remove(&client_id);
        }

        // handle coord poll if applicable
        if let SimConnection::ClusterCoord(coord) = &mut self.sim {
            coord.manual_poll()?;
        }

        // handle events from clients
        let client_ids: Vec<u32> = self.clients.keys().cloned().collect();
        for client_id in client_ids {
            let (addr, event) = match self
                .clients
                .get_mut(&client_id)
                .unwrap()
                .connection
                .try_recv()
            {
                Ok(e) => e,
                Err(e) => match e {
                    Error::WouldBlock => continue,
                    _ => {
                        warn!("try_handle_client failed: {:?}", e);
                        continue;
                    }
                },
            };
            self.clients
                .get_mut(&client_id)
                .unwrap()
                .time_since_last_event = Duration::from_millis(0);
            self.time_since_last_msg = Duration::from_millis(0);
            self.handle_event(event, &client_id)?;
        }
        Ok(())
    }

    /// This function handles shutdown cleanup, like killing spawned services.
    pub fn cleanup(&mut self) -> Result<()> {
        for service in &mut self.services {
            service.stop();
        }
        Ok(())
    }

    /// Start a polling loop.
    ///
    /// Allows for remote termination.
    ///
    pub fn start_polling(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        loop {
            // terminate loop if the `running` bool gets flipped to false
            if !running.load(Ordering::SeqCst) {
                break;
            }

            // wait a little to reduce polling overhead
            thread::sleep(self.config.poll_wait);
            self.uptime += self.config.poll_wait;
            self.time_since_last_accept += self.config.poll_wait;

            // perform manual poll, match for loop-breaking errors
            if let Err(err) = self.manual_poll() {
                match err {
                    Error::ServerKeepaliveLimitReached(_) => return Err(err),
                    _ => warn!("server error: {:?}", err),
                }
            }
        }
        Ok(())
    }

    /// Tries to accept a single new client connection.
    ///
    /// On success returns a newly assigned client id.
    ///
    /// # Redirection
    ///
    /// Help
    pub fn try_accept_client(&mut self, redirect: bool) -> Result<u32> {
        for mut greeter in &mut self.greeters {
            // let event = greeter.try_recv()?;
            // println!("{:?}", event);
            // let (_, msg) = match greeter.recv_msg() {
            let msg = match greeter.try_recv_msg() {
                Ok(msg) => msg,
                Err(e) => {
                    match e {
                        Error::WouldBlock => (),
                        _ => {
                            error!("{:?}", e);
                        }
                    }
                    // error!("{}", e.to_string());
                    continue;
                }
            };

            println!("got new client message");

            let req: RegisterClientRequest = msg.unpack_payload(greeter.encoding())?;
            self.port_count += 1;

            // TODO
            let newport = format!("{}:{}", greeter.last_endpoint()?.ip(), self.port_count);

            debug!("newport: {}", newport);

            let socket = Socket::bind_with_config(&newport, greeter.transport(), greeter.config())?;
            // let client_socket = client_socket;
            debug!("req.addr: {:?}", req.addr);

            let resp = RegisterClientResponse {
                //redirect: format!("192.168.2.106:{}", client_id),
                redirect: newport,
                error: String::new(),
            };

            greeter.pack_send_msg_payload(resp, None)?;
            debug!("responded to client: {}", self.port_count);
            debug!("client is blocking? {}", req.is_blocking);
            let client = Client {
                id: self.port_count,
                addr: "".to_string(),
                connection: socket,
                is_blocking: req.is_blocking,
                keepalive: self.config.client_keepalive,
                time_since_last_event: Default::default(),
                auth_pair: None,
                name: "".to_string(),
                furthest_step: match &self.sim {
                    SimConnection::Local(sim) => sim.get_clock(),
                    SimConnection::ClusterCoord(coord) => coord.central.get_clock(),
                    _ => unimplemented!(),
                },
                scheduled_dts: Default::default(),
                order_store: Default::default(),
                order_id_pool: IdPool::new(),
            };

            self.clients.insert(self.port_count, client);
            return Ok(self.port_count);
        }

        Err(Error::WouldBlock)
    }

    // /// Handle new client connection.
    // ///
    // /// # Idle Timeout
    // ///
    // /// `idle_timeout` argument specifies the time after which client is
    // /// dropped if there are not messages being received. `None` means idle
    // /// client will not get dropped.
    // pub fn handle_new_client_connection(&mut self, client_id: &ClientId) -> Result<()> {
    //     let mut timeout_counter = 0;
    //     loop {
    //         // sleep a little to make this thread less expensive
    //         sleep(Duration::from_millis(10));
    //
    //         let msg = match client_socket.try_read_msg(None) {
    //             Ok(m) => m,
    //             Err(e) => match e {
    //                 Error::WouldBlock => {
    //                     if let Some(t) = idle_timeout {
    //                         if timeout_counter > t {
    //                             break;
    //                         } else {
    //                             timeout_counter += 10;
    //                         }
    //                     };
    //                     continue;
    //                 }
    //                 Error::HostUnreachable => {
    //                     println!("{:?}", e);
    //                     break;
    //                 }
    //                 _ => unimplemented!(),
    //             },
    //         };
    //
    //         // got a new message, reset the timeout counter
    //         timeout_counter = 0;
    //         self.handle_message(msg, client_id)?;
    //     }
    //
    //     // drop client
    //     info!("dropping client {}!", client_id);
    //     server.lock().unwrap().clients.remove(client_id);
    //     Ok(())
    // }

    /// Handle message, delegating further processing to a specialized function.
    fn handle_event(&mut self, event: SocketEvent, client_id: &ClientId) -> Result<()> {
        debug!("handling message: {:?}", event);
        let encoding = self
            .clients
            .get(client_id)
            .unwrap()
            .connection
            .encoding()
            .clone();
        match event {
            SocketEvent::Heartbeat => (),
            SocketEvent::Bytes(bytes) => {
                self.handle_message(Message::from_bytes(bytes)?, client_id)?
            }
            SocketEvent::Message(msg) => self.handle_message(msg, client_id)?,
            SocketEvent::Connect => println!("new connection event from client: {}", client_id),
            SocketEvent::Disconnect => println!("disconnected event from client: {}", client_id),
            _ => unimplemented!(),
        }
        trace!("handled");
        Ok(())
    }

    fn handle_message(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let message_type = MessageType::try_from(msg.type_)?;
        match message_type {
            // MessageKind::Heartbeat => (),
            MessageType::PingRequest => self.handle_ping_request(msg, client_id)?,
            MessageType::StatusRequest => self.handle_status_request(msg, client_id)?,
            MessageType::TurnAdvanceRequest => self.handle_turn_advance_request(msg, client_id)?,

            MessageType::DataTransferRequest => {
                self.handle_data_transfer_request(msg, client_id)?
            }
            MessageType::TypedDataTransferRequest => {
                self.handle_typed_data_transfer_request(msg, client_id)?
            }
            MessageType::DataPullRequest => self.handle_data_pull_request(msg, client_id)?,
            MessageType::TypedDataPullRequest => {
                self.handle_typed_data_pull_request(msg, client_id)?
            }
            MessageType::ScheduledDataTransferRequest => {
                self.handle_scheduled_data_transfer_request(msg, client_id)?
            }
            // DATA_TRANSFER_REQUEST => self.handle_data_transfer_request(msg, client_id)?,
            // SCHEDULED_DATA_TRANSFER_REQUEST => {
            //     self.handle_scheduled_data_transfer_request(msg, client_id)?
            // }
            // DATA_PULL_REQUEST => self.handle_data_pull_request(msg, client_id)?,
            //
            MessageType::SpawnEntitiesRequest => {
                self.handle_spawn_entities_request(msg, client_id)?
            }
            MessageType::ExportSnapshotRequest => {
                self.handle_export_snapshot_request(msg, client_id)?
            }
            _ => println!("unknown message type: {:?}", message_type),
        }
        Ok(())
    }

    pub fn handle_export_snapshot_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let client = self.clients.get(client_id).unwrap();
        let req: ExportSnapshotRequest = msg.unpack_payload(client.connection.encoding())?;
        if req.save_to_disk {
            let snap = match &self.sim {
                SimConnection::Local(sim) => sim.to_snapshot(false)?,
                _ => unimplemented!(),
            };
            let target_path = match &self.sim {
                SimConnection::Local(sim) => {
                    sim.model.scenario.path.join("snapshots").join(req.name)
                }
                _ => unimplemented!(),
            };
            // let target_path = self.local_project_path.join("snapshots").join(req.name);
            if std::fs::File::open(&target_path).is_ok() {
                std::fs::remove_file(&target_path);
            }
            let mut file = std::fs::File::create(target_path)?;
            file.write(&snap);
        }

        let resp = ExportSnapshotResponse {
            error: "".to_string(),
            snapshot: vec![],
        };

        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_spawn_entities_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let client = self.clients.get(client_id).unwrap();
        let req: SpawnEntitiesRequest = msg.unpack_payload(client.connection.encoding())?;
        for (i, prefab) in req.entity_prefabs.iter().enumerate() {
            trace!("handling prefab: {}", prefab);
            let entity_name = match req.entity_names[i].as_str() {
                "" => None,
                _ => Some(arraystring::new_truncate(&req.entity_names[i])),
            };
            match &mut self.sim {
                SimConnection::Local(sim) => {
                    sim.spawn_entity(
                        Some(&outcome::arraystring::new_truncate(&prefab)),
                        entity_name,
                    )?;
                }
                _ => unimplemented!(),
            }
        }
        let resp = SpawnEntitiesResponse {
            error: "".to_string(),
        };
        trace!("starting send..");

        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_ping_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let client = self.clients.get_mut(client_id).unwrap();
        let req: PingRequest = msg.unpack_payload(client.connection.encoding())?;
        let resp = PingResponse { bytes: req.bytes };
        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_status_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let connected_clients = self.clients.iter().map(|(id, c)| c.name.clone()).collect();
        let mut client = self.clients.get_mut(client_id).unwrap();
        let req: StatusRequest = msg.unpack_payload(client.connection.encoding())?;
        let model_scenario = match &self.sim {
            SimConnection::Local(sim) => sim.model.scenario.clone(),
            SimConnection::ClusterCoord(coord) => coord.central.model.scenario.clone(),
            _ => unimplemented!(),
        };
        let resp = StatusResponse {
            name: self.config.name.clone(),
            description: self.config.description.clone(),
            address: self.greeters.first().unwrap().last_endpoint()?.to_string(),
            connected_clients,
            engine_version: outcome_core::VERSION.to_owned(),
            uptime: self.uptime.as_millis() as usize,
            current_tick: match &self.sim {
                SimConnection::Local(sim) => sim.get_clock(),
                SimConnection::ClusterCoord(coord) => coord.central.get_clock(),
                _ => unimplemented!(),
            },
            scenario_name: model_scenario.manifest.name.clone(),
            scenario_title: model_scenario
                .manifest
                .title
                .clone()
                .unwrap_or("".to_string()),
            scenario_desc: model_scenario
                .manifest
                .desc
                .clone()
                .unwrap_or("".to_string()),
            scenario_desc_long: model_scenario
                .manifest
                .desc_long
                .clone()
                .unwrap_or("".to_string()),
            scenario_author: model_scenario
                .manifest
                .author
                .clone()
                .unwrap_or("".to_string()),
            scenario_website: model_scenario
                .manifest
                .website
                .clone()
                .unwrap_or("".to_string()),
            scenario_version: model_scenario.manifest.version.clone(),
            scenario_engine: model_scenario.manifest.engine.clone(),
            scenario_mods: model_scenario
                .manifest
                .mods
                .clone()
                .iter()
                .map(|smd| format!("{} ({})", smd.name, smd.version_req))
                .collect(),
            scenario_settings: model_scenario
                .manifest
                .settings
                .clone()
                .iter()
                .map(|(k, v)| format!("{} = {:?}", k, v))
                .collect(),
        };
        trace!("sent status response");
        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_data_transfer_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        let dtr: DataTransferRequest = match msg.unpack_payload(client.connection.encoding()) {
            Ok(r) => r,
            Err(e) => {
                let response = DataTransferResponse {
                    data: None,
                    error: "FailedUnpackingPayload".to_string(),
                };
                client.connection.pack_send_msg_payload(response, None)?;
                // if let Ok(ms) = msg_size {
                //     println!("sent DataTransferResponse ({} KB)", ms as f32 / 1000.0);
                // }
                panic!("failed unpacking payload: {}", e);
                // return Ok(());
            }
        };
        let mut data_pack = TypedSimDataPack::empty();
        match &mut self.sim {
            SimConnection::ClusterCoord(coord) => {
                let mut collection = Vec::new();
                match dtr.transfer_type.as_str() {
                    "Full" => {
                        for (worker_id, worker) in &mut coord.net.workers {
                            worker
                                .connection
                                .send_sig(outcome::distr::Signal::DataRequestAll, None)?
                        }
                        for (worker_id, worker) in &mut coord.net.workers {
                            let (_, sig) = worker.connection.recv_sig()?;
                            match sig.into_inner() {
                                outcome::distr::Signal::DataResponse(data) => {
                                    collection.extend(data)
                                }
                                _ => unimplemented!(),
                            }
                        }
                        let mut sdp = TypedSimDataPack::empty();
                        for (addr, var) in collection {
                            match addr.var_type {
                                VarType::Str => {
                                    sdp.strings.insert(addr.to_string(), var.to_string());
                                }
                                VarType::Int => {
                                    sdp.ints.insert(addr.to_string(), var.to_int());
                                }
                                VarType::Float => {
                                    sdp.floats.insert(addr.to_string(), var.to_float());
                                }
                                _ => (),
                            }
                        }

                        let response = DataTransferResponse {
                            data: Some(TransferResponseData::Typed(sdp)),
                            error: String::new(),
                        };
                        client.connection.pack_send_msg_payload(response, None)?;
                    }
                    _ => unimplemented!(),
                }
            }
            SimConnection::Local(sim_instance) => {
                handle_data_transfer_request_local(&dtr, sim_instance, client)?
            }
            _ => unimplemented!(),
        };

        Ok(())
    }

    pub fn handle_typed_data_transfer_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        let dtr: TypedDataTransferRequest = match msg.unpack_payload(client.connection.encoding()) {
            Ok(r) => r,
            Err(e) => {
                let response = DataTransferResponse {
                    data: None,
                    error: "FailedUnpackingPayload".to_string(),
                };
                client.connection.pack_send_msg_payload(response, None)?;
                panic!("failed unpacking payload: {}", e);
            }
        };
        let mut data_pack = TypedSimDataPack::empty();
        match &mut self.sim {
            SimConnection::Local(sim_instance) => {
                let model = &sim_instance.model;
                match dtr.transfer_type.as_str() {
                    "Full" => {
                        let mut data_pack = TypedSimDataPack::empty();
                        for (entity_uid, entity) in &sim_instance.entities {
                            for ((comp_name, var_id), v) in entity.storage.map.iter() {
                                if v.is_float() {
                                    data_pack.floats.insert(
                                        format!(
                                            ":{}:{}:{}:{}",
                                            // get entity string id if available
                                            sim_instance
                                                .entities_idx
                                                .iter()
                                                .find(|(e_id, e_idx)| e_idx == &entity_uid)
                                                .map(|(e_id, _)| e_id.as_str())
                                                .unwrap_or(entity_uid.to_string().as_str()),
                                            comp_name,
                                            VarType::Float.to_str(),
                                            var_id
                                        ),
                                        // comp_name.to_string(),
                                        *v.as_float().unwrap(),
                                    );
                                }
                            }
                        }

                        let response = TypedDataTransferResponse {
                            data: Some(data_pack),
                            error: String::new(),
                        };
                        client.connection.pack_send_msg_payload(response, None);
                    }
                    _ => unimplemented!(),
                }
            }
            _ => unimplemented!(),
        }
        Ok(())
    }

    pub fn handle_scheduled_data_transfer_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let mut client = self
            .clients
            .get_mut(client_id)
            .ok_or(Error::Other("failed getting client".to_string()))?;
        let sdtr: ScheduledDataTransferRequest =
            msg.unpack_payload(client.connection.encoding())?;
        for event_trigger in sdtr.event_triggers {
            let event_id = outcome::arraystring::new(&event_trigger)?;
            if !client.scheduled_dts.contains_key(&event_id) {
                client.scheduled_dts.insert(event_id, Vec::new());
            }
            let dtr = DataTransferRequest {
                transfer_type: sdtr.transfer_type.clone(),
                selection: sdtr.selection.clone(),
            };
            client.scheduled_dts.get_mut(&event_id).unwrap().push(dtr);
        }

        Ok(())
    }

    fn handle_single_address(server: &Server) {}

    pub fn handle_data_pull_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        {
            let use_compression = self.config.use_compression.clone();
            // let sim_model = server.sim_model.clone();
            let mut sim_instance = match &mut self.sim {
                SimConnection::Local(sim) => sim,
                SimConnection::ClusterCoord(coord) => unimplemented!(),
                SimConnection::ClusterWorker(worker) => unimplemented!(),
            };
            //TODO
            let dpr: DataPullRequest = msg.unpack_payload(client.connection.encoding())?;
            match dpr.data {
                PullRequestData::Typed(data) => {
                    //TODO handle errors
                    for (address, var) in data.strings {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_str_mut()? = var;
                    }
                    for (address, var) in data.ints {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_int_mut()? = var;
                    }
                    for (address, var) in data.floats {
                        let addr = Address::from_str(&address[1..])?;
                        *sim_instance.get_var_mut(&addr)?.as_float_mut()? = var;
                    }
                    for (address, var) in data.bools {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_bool_mut()? = var;
                    }
                    for (address, var) in data.string_lists {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_str_list_mut()? = var;
                    }
                    for (address, var) in data.int_lists {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_int_list_mut()? = var;
                    }
                    for (address, var) in data.float_lists {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_float_list_mut()? = var;
                    }
                    for (address, var) in data.bool_lists {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_bool_list_mut()? = var;
                    }
                    #[cfg(feature = "outcome/grids")]
                    for (address, var) in data.string_grids {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_str_grid_mut()? = var;
                    }
                    #[cfg(feature = "outcome/grids")]
                    for (address, var) in data.int_grids {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_int_grid_mut()? = var;
                    }
                    #[cfg(feature = "outcome/grids")]
                    for (address, var) in data.float_grids {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_float_grid_mut()? = var;
                    }
                    #[cfg(feature = "outcome/grids")]
                    for (address, var) in data.bool_grids {
                        let addr = Address::from_str(&address)?;
                        *sim_instance.get_var_mut(&addr)?.as_bool_grid_mut()? = var;
                    }
                }
                PullRequestData::Var(data) => {
                    //
                }
                PullRequestData::VarOrdered(order_idx, data) => {
                    if let Some(order) = client.order_store.get(&order_idx) {
                        if data.vars.len() != order.len() {
                            warn!("PullRequestData::VarOrdered: var list length doesn't match ({} vs {})", data.vars.len(), order.len());
                            panic!();
                        }
                        for (n, addr) in order.iter().enumerate() {
                            *sim_instance.get_var_mut(addr)? = data.vars[n].clone();
                        }
                    }
                }
            }
        }
        let resp = DataPullResponse {
            error: String::new(),
        };
        // send_message(message_from_payload(resp, false), stream, None);
        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_typed_data_pull_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let mut client = self.clients.get_mut(client_id).unwrap();
        {
            let use_compression = self.config.use_compression.clone();
            // let sim_model = server.sim_model.clone();
            let mut sim_instance = match &mut self.sim {
                SimConnection::Local(sim) => sim,
                SimConnection::ClusterCoord(coord) => unimplemented!(),
                SimConnection::ClusterWorker(worker) => unimplemented!(),
            };
            //TODO
            let dpr: TypedDataPullRequest = msg.unpack_payload(client.connection.encoding())?;
            let data = dpr.data;
            //TODO handle errors
            for (address, var) in data.strings {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_str_mut()? = var;
            }
            for (address, var) in data.ints {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_int_mut()? = var;
            }
            for (address, var) in data.floats {
                let addr = Address::from_str(&address[1..])?;
                *sim_instance.get_var_mut(&addr)?.as_float_mut()? = var;
            }
            for (address, var) in data.bools {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_bool_mut()? = var;
            }
            for (address, var) in data.string_lists {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_str_list_mut()? = var;
            }
            for (address, var) in data.int_lists {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_int_list_mut()? = var;
            }
            for (address, var) in data.float_lists {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_float_list_mut()? = var;
            }
            for (address, var) in data.bool_lists {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_bool_list_mut()? = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in data.string_grids {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_str_grid_mut()? = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in data.int_grids {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_int_grid_mut()? = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in data.float_grids {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_float_grid_mut()? = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in data.bool_grids {
                let addr = Address::from_str(&address)?;
                *sim_instance.get_var_mut(&addr)?.as_bool_grid_mut()? = var;
            }
        }

        let resp = DataPullResponse {
            error: String::new(),
        };
        // send_message(message_from_payload(resp, false), stream, None);
        client.connection.pack_send_msg_payload(resp, None)
    }

    pub fn handle_turn_advance_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let req: TurnAdvanceRequest =
            msg.unpack_payload(self.clients.get(client_id).unwrap().connection.encoding())?;

        let mut client_furthest_tick = 0;
        {
            let mut no_blocking_clients = true;
            let current_tick = match &self.sim {
                SimConnection::Local(s) => s.get_clock(),
                SimConnection::ClusterCoord(c) => c.central.clock,
                _ => unimplemented!(),
            };
            trace!("current_tick before: {}", current_tick);
            let mut common_furthest_tick = current_tick + 99999;
            for (id, _client) in &mut self.clients {
                if _client.id == *client_id {
                    trace!(
                        "({}) furthest_tick: {}, current_tick: {}",
                        _client.id,
                        _client.furthest_step,
                        current_tick
                    );
                    if _client.furthest_step < current_tick {
                        _client.furthest_step = current_tick;
                    }
                    if _client.furthest_step - current_tick < req.tick_count as usize {
                        _client.furthest_step = _client.furthest_step + req.tick_count as usize;
                    }
                    client_furthest_tick = _client.furthest_step.clone();
                }
                if !_client.is_blocking {
                    trace!("omit non-blocking client..");
                    continue;
                } else {
                    no_blocking_clients = false;
                }
                trace!(
                    "client_furthest_tick inside loop: {}",
                    _client.furthest_step
                );
                if _client.furthest_step == current_tick {
                    common_furthest_tick = current_tick;
                    break;
                }
                if _client.furthest_step < common_furthest_tick {
                    common_furthest_tick = _client.furthest_step;
                }
            }
            if no_blocking_clients {
                let t = self.clients.get(&client_id).unwrap().furthest_step;
                common_furthest_tick = t;
            }
            trace!("common_furthest_tick: {}", common_furthest_tick);
            if common_furthest_tick > current_tick {
                // let sim_model = server.sim_model.clone();
                match &mut self.sim {
                    SimConnection::Local(sim_instance) => {
                        for _ in 0..common_furthest_tick - current_tick {
                            sim_instance.step();
                            // let events = sim_instance.event_queue.clone();
                            trace!("processed single tick");
                            trace!(
                                "common_furthest_tick: {}, current_tick: {}",
                                common_furthest_tick,
                                current_tick
                            );

                            // advanced turn, check if any scheduled datatransfers need sending
                            for (_, client) in &mut self.clients {
                                for (event, dts_list) in &client.scheduled_dts.clone() {
                                    trace!("handling scheduled data transfer: event: {}", event);
                                    if sim_instance.event_queue.contains(&event) {
                                        for dtr in dts_list {
                                            info!(
                                                "handling scheduled data transfer: dtr: {:?}",
                                                dtr
                                            );
                                            handle_data_transfer_request_local(
                                                dtr,
                                                sim_instance,
                                                client,
                                            )?
                                        }
                                    }
                                }
                            }
                        }
                        trace!("current_tick after: {}", sim_instance.get_clock());
                    }
                    SimConnection::ClusterCoord(coord) => {
                        let mut event_queue = coord.central.event_queue.clone();

                        let step_event_name = arraystring::new_unchecked("step");
                        if !event_queue.contains(&step_event_name) {
                            event_queue.push(step_event_name);
                        }
                        coord.central.event_queue.clear();

                        // let network = &coord_lock.network;
                        // let central = &mut coord_lock.central;
                        coord.central.step_network(&mut coord.net, event_queue);
                        // coord_lock
                        //     .central
                        //     .step_network(&mut coord_lock.network, event_queue)?;
                        coord.central.clock += 1;

                        // let mut addr_book = HashMap::new();
                        // for node in &coord.nodes {
                        //     addr_book.insert(node.id.clone(), node.connection.try_clone().unwrap());
                        // }
                        //coord.main.step(&coord.entity_node_map, &mut addr_book);
                    }
                    _ => unimplemented!(),
                };
            }

            let client = self.clients.get_mut(client_id).unwrap();

            // responses
            if common_furthest_tick == current_tick {
                let resp = TurnAdvanceResponse {
                    error: "BlockedFully".to_string(),
                };
                trace!("BlockedFully");
                client.connection.pack_send_msg_payload(resp, None)?;
            } else if common_furthest_tick < client_furthest_tick {
                let resp = TurnAdvanceResponse {
                    error: "BlockedPartially".to_string(),
                };
                trace!("BlockedPartially");
                client.connection.pack_send_msg_payload(resp, None)?;
            //        } else if common_furthest_tick == client_furthest_tick {
            } else {
                let resp = TurnAdvanceResponse {
                    error: String::new(),
                };
                trace!("Didn't block");
                client.connection.pack_send_msg_payload(resp, None)?;
            }
        }
        Ok(())
    }

    pub fn handle_list_local_scenarios_request(
        &mut self,
        payload: Vec<u8>,
        client: &mut Client,
    ) -> Result<()> {
        let req: ListLocalScenariosRequest = unpack(&payload, client.connection.encoding())?;
        //TODO check `$working_dir/scenarios` for scenarios
        //
        //

        let resp = ListLocalScenariosResponse {
            scenarios: Vec::new(),
            error: String::new(),
        };
        client.connection.pack_send_msg_payload(resp, None)
    }
    pub fn handle_load_local_scenario_request(
        payload: Vec<u8>,
        server_arc: Arc<Mutex<Server>>,
        client: &mut Client,
    ) -> Result<()> {
        let req: LoadLocalScenarioRequest = unpack(&payload, client.connection.encoding())?;

        //TODO
        //

        let resp = LoadLocalScenarioResponse {
            error: String::new(),
        };
        client.connection.pack_send_msg_payload(resp, None)
    }
    pub fn handle_load_remote_scenario_request(
        payload: Vec<u8>,
        server_arc: Arc<Mutex<Server>>,
        client: &mut Client,
    ) -> Result<()> {
        let req: LoadRemoteScenarioRequest = unpack(&payload, client.connection.encoding())?;

        //TODO
        //

        let resp = LoadRemoteScenarioResponse {
            error: String::new(),
        };
        client.connection.pack_send_msg_payload(resp, None)
    }
}

fn handle_data_transfer_request_local(
    request: &DataTransferRequest,
    sim_instance: &Sim,
    client: &mut Client,
) -> Result<()> {
    let model = &sim_instance.model;
    match request.transfer_type.as_str() {
        "Full" => {
            let mut data_pack = TypedSimDataPack::empty();
            for (entity_uid, entity) in &sim_instance.entities {
                for ((comp_name, var_id), v) in entity.storage.map.iter() {
                    if v.is_float() {
                        data_pack.floats.insert(
                            format!(
                                ":{}:{}:{}:{}",
                                entity_uid,
                                comp_name,
                                VarType::Float.to_str(),
                                var_id
                            ),
                            // comp_name.to_string(),
                            *v.as_float().unwrap(),
                        );
                    }
                }
            }

            let response = DataTransferResponse {
                data: Some(TransferResponseData::Typed(data_pack)),
                error: String::new(),
            };
            client.connection.pack_send_msg_payload(response, None)
        }
        "Select" => {
            let mut data_pack = TypedSimDataPack::empty();
            let mut selected = Vec::new();
            selected.extend_from_slice(&request.selection);

            // todo handle asterrisk addresses
            // for address in &dtr.selection {
            //     if address.contains("*") {
            //         let addr = Address::from_str(address).unwrap();
            //         selected.extend(
            //             addr.expand(sim_instance)
            //                 .iter()
            //                 .map(|addr| addr.to_string()),
            //         );
            //     }
            // }
            for address in &selected {
                let address = match outcome::Address::from_str(&address) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                if let Ok(var) = sim_instance.get_var(&address) {
                    if var.is_float() {
                        data_pack
                            .floats
                            .insert(address.to_string(), *var.as_float().unwrap());
                    }
                }
            }

            let response = DataTransferResponse {
                data: Some(TransferResponseData::Typed(data_pack)),
                error: String::new(),
            };
            client.connection.pack_send_msg_payload(response, None)
        }
        // select using addresses but return data as ordered set without
        // address keys, order is stored on server under it's own unique id
        "SelectVarOrdered" => {
            let mut data = VarSimDataPackOrdered::default();
            let selection = &request.selection;

            // empty selection means reuse last ordering
            if selection.is_empty() {
                let order_id = 1;
                let order = client.order_store.get(&order_id).unwrap();
                for addr in order {
                    if let Ok(var) = sim_instance.get_var(&addr) {
                        data.vars.push(var.clone());
                    }
                }
                let response = DataTransferResponse {
                    data: Some(TransferResponseData::VarOrdered(order_id, data)),
                    error: String::new(),
                };
                client.connection.pack_send_msg_payload(response, None)
            } else {
                let mut order = Vec::new();

                for query in selection {
                    if query.contains("*") {
                        for (id, entity) in &sim_instance.entities {
                            if id == &0 || id == &1 {
                                continue;
                            }
                            let _query = query.replace("*", &id.to_string());
                            let addr = outcome::Address::from_str(&_query)?;
                            order.push(addr);
                            if let Ok(var) = sim_instance.get_var(&addr) {
                                data.vars.push(var.clone());
                            }
                        }
                    } else {
                        // TODO save the ordered list of addresses on the server for handling response
                        let addr = outcome::Address::from_str(query)?;
                        order.push(addr);
                        if let Ok(var) = sim_instance.get_var(&addr) {
                            data.vars.push(var.clone());
                        }
                    }
                }

                let order_id = client
                    .order_id_pool
                    .request_id()
                    .ok_or(Error::Other("failed getting new order id".to_string()))?;
                client.order_store.insert(order_id, order);

                let response = DataTransferResponse {
                    data: Some(TransferResponseData::VarOrdered(order_id, data)),
                    error: String::new(),
                };
                client.connection.pack_send_msg_payload(response, None)
            }
        }
        _ => Err(Error::Unknown),
    }
}
