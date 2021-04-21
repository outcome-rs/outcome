use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::io::{ErrorKind, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use fnv::FnvHashMap;
use id_pool::IdPool;
use outcome::{arraystring, Address, EventName, Sim, SimModel, StringId, VarType};

use crate::msg::*;
use crate::service::Service;

use crate::coord::CoordTask;
use crate::msg::TransferResponseData::AddressedVar;
use crate::socket::{
    pack, unpack, Encoding, Socket, SocketAddress, SocketConfig, SocketEvent, SocketEventType,
    SocketType, Transport,
};
use crate::{error::Error, Result, TaskId};
use crate::{Coord, Worker};
use outcome::distr::{CentralCommunication, NodeCommunication, Signal};

mod pull;
mod query;
mod turn;

pub type ClientId = u32;

pub enum ServerTask {
    WaitForCoordQueryResponse(ClientId),
}

/// High-level representation of the simulation interface.
pub enum SimConnection {
    Local(Sim),
    ClusterCoord(Coord),
    ClusterWorker(Worker),
}

/// Connected client as seen by the server.
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
    pub last_event: Instant,

    /// Authentication pair used by the client
    pub auth_pair: Option<(String, String)>,
    /// Client self-assigned name
    pub name: String,

    /// List of scheduled data transfers
    pub scheduled_dts: FnvHashMap<EventName, Vec<DataTransferRequest>>,
    /// List of scheduled queries
    pub scheduled_queries: FnvHashMap<EventName, Vec<(TaskId, outcome::Query)>>,

    pub order_store: FnvHashMap<u32, Vec<Address>>,
    pub order_id_pool: IdPool,
}

impl Client {
    pub fn push_event_triggered_query(
        &mut self,
        event: EventName,
        task_id: TaskId,
        query: outcome::Query,
    ) -> Result<()> {
        if !self.scheduled_queries.contains_key(&event) {
            self.scheduled_queries.insert(event, Vec::new());
        }
        self.scheduled_queries
            .get_mut(&event)
            .unwrap()
            .push((task_id, query));

        Ok(())
    }
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
                Transport::ZmqTcp,
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
    last_accept_time: Instant,

    pub services: Vec<Service>,

    pub tasks: HashMap<TaskId, ServerTask>,
}

impl Server {
    /// Creates new server using provided address and default config.
    pub fn new(addr: &str, sim: SimConnection) -> Result<Self> {
        Self::new_with_config(addr, ServerConfig::default(), sim)
    }

    /// Creates new server at available localhost port using default config.
    pub fn new_at_any(sim: SimConnection) -> Result<Self> {
        Self::new_at_any_with_config(ServerConfig::default(), sim)
    }

    /// Creates a new server at available localhost port using provided config.
    pub fn new_at_any_with_config(config: ServerConfig, sim: SimConnection) -> Result<Self> {
        Self::new_with_config("0.0.0.0:0", config, sim)
    }

    /// Creates a new server using provided address and config.
    pub fn new_with_config(addr: &str, config: ServerConfig, sim: SimConnection) -> Result<Self> {
        let (encoding, transport, address) = SocketAddress::parse_composite(addr)?;
        println!(
            "encoding: {:?}, transport: {:?}, address: {:?}",
            encoding, transport, address
        );
        let mut greeters = Vec::new();

        let mut greeter_config = SocketConfig {
            type_: SocketType::Pair,
            ..Default::default()
        };

        if let Some(_transport) = transport {
            if let Some(_encoding) = encoding {
                greeter_config.encoding = _encoding;
                println!("binding socket");
                let sock =
                    Socket::new_with_config(Some(address.clone()), _transport, greeter_config)?;
                greeters.push(sock);
            }
        }

        for encoding in &config.encodings {
            let greeter_config = SocketConfig {
                type_: SocketType::Pair,
                encoding: *encoding,
                ..Default::default()
            };

            if let Some(trans) = transport {
                let sock = Socket::new_with_config(Some(address.clone()), trans, greeter_config)?;
                println!("sock binded");
                greeters.push(sock);
            } else {
                for transport in &config.transports {
                    match &address {
                        SocketAddress::Net(addr) => {
                            let _address = if addr.port() != 0 {
                                SocketAddress::Net(*addr)
                            } else {
                                SocketAddress::Net(SocketAddr::new(addr.ip(), 0))
                            };
                            info!(
                                "starting listener on: {} (transport: {:?}, encoding: {:?})",
                                address, transport, encoding
                            );
                            greeters.push(Socket::new_with_config(
                                Some(_address),
                                *transport,
                                greeter_config,
                            )?);
                        }
                        // port = port + 1;
                        SocketAddress::File(path) => greeters.push(Socket::new_with_config(
                            Some(address.clone()),
                            *transport,
                            greeter_config,
                        )?),
                        _ => unimplemented!(),
                    }
                }
            }
        }

        Ok(Self {
            sim,
            config,
            // TODO select transport based on config's transport list
            greeters,
            // TODO make this easier to find by defining it as a constant
            port_count: 0,
            clients: Default::default(),
            uptime: Default::default(),
            time_since_last_msg: Default::default(),
            last_accept_time: Instant::now(),
            services: vec![],
            tasks: Default::default(),
        })
    }

    /// Initializes services based on the available model.
    ///
    /// # New services with model changes
    ///
    /// Can be called repeatedly to initialize services following model
    /// changes.
    pub fn initialize_services(&mut self) -> Result<()> {
        match &mut self.sim {
            SimConnection::Local(sim) => {
                // start the service processes
                for service_model in &sim.model.services {
                    if self
                        .services
                        .iter()
                        .find(|s| s.name == service_model.name)
                        .is_none()
                    {
                        info!("starting service: {}", service_model.name);
                        let service = Service::start_from_model(
                            service_model.clone(),
                            self.greeters.first().unwrap().listener_addr()?.to_string(),
                        )?;
                        self.services.push(service);
                    }
                }
            }
            SimConnection::ClusterWorker(worker) => {
                if let Some(node) = &worker.sim_node {
                    for service_model in &node.model.services {
                        if self
                            .services
                            .iter()
                            .find(|s| s.name == service_model.name)
                            .is_none()
                        {
                            info!("starting service: {}", service_model.name);
                            let service = Service::start_from_model(
                                service_model.clone(),
                                self.greeters.first().unwrap().listener_addr()?.to_string(),
                            )?;
                            self.services.push(service);
                        }
                    }
                }
            }
            SimConnection::ClusterCoord(coord) => {
                // warn!("not starting any services since it's a coordinator-backed server");
            }
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

        // initialize services that might be missing
        self.initialize_services();

        // TODO implement time setting for monitoring every n-th poll
        // monitor services
        for service in &mut self.services {
            service.monitor();
        }

        // handle new incoming clients
        let time_since_last_accept = Instant::now() - self.last_accept_time;
        if time_since_last_accept >= self.config.accept_delay {
            self.last_accept_time = Instant::now();
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
            let time_since_last_event = Instant::now() - client.last_event;
            // println!(
            //     "handling idle clients, client: {}, time since last event: {}ms",
            //     client_id,
            //     client.time_since_last_event.as_millis()
            // );
            if let Some(keepalive) = client.keepalive {
                if time_since_last_event > keepalive {
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
            // perform the manual poll
            coord.manual_poll()?;
            // handle any tasks that might have been finished
            Server::handle_coord_tasks(&mut self.tasks, &self.clients, coord)?;
        }

        // handle worker poll if applicable
        if let SimConnection::ClusterWorker(worker) = &mut self.sim {
            worker.manual_poll()?;
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
            if let Some(client) = self.clients.get_mut(&client_id) {
                client.last_event = Instant::now();
                if client.addr != addr.to_string() {
                    client.addr = addr.to_string();
                }
            }
            self.time_since_last_msg = Duration::from_millis(0);
            if let Err(e) = self.handle_event(event, &client_id) {
                error!("{}", e);
            }
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
    pub fn start_polling(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        loop {
            // terminate loop if the `running` bool gets flipped to false
            if !running.load(Ordering::SeqCst) {
                break;
            }

            // wait a little to reduce polling overhead
            thread::sleep(self.config.poll_wait);
            self.uptime += self.config.poll_wait;
            self.last_accept_time += self.config.poll_wait;

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

    /// Tries to accept a single new client connection.spa
    ///
    /// On success returns a newly assigned client id.
    ///
    /// # Redirection
    ///
    /// Help
    pub fn try_accept_client(&mut self, redirect: bool) -> Result<u32> {
        for mut greeter in &mut self.greeters {
            let (peer_addr, msg) = match greeter.try_recv_msg() {
                Ok(msg) => msg,
                Err(e) => {
                    match e {
                        Error::WouldBlock => (),
                        _ => {
                            error!("{:?}", e);
                        }
                    }
                    continue;
                }
            };
            info!("got new client message: {:?}", msg);

            let req: RegisterClientRequest = msg.unpack_payload(greeter.encoding())?;
            println!("req: {:?}", req);
            self.port_count += 1;

            let _address = match greeter.listener_addr()? {
                SocketAddress::Net(addr) => SocketAddress::Net(SocketAddr::new(addr.ip(), 0)),
                SocketAddress::File(path) => {
                    SocketAddress::File(format!("{}{}", path, self.port_count.to_string()))
                }
                _ => unimplemented!(),
            };

            let socket = Socket::new_with_config(
                Some(_address.clone()),
                greeter.transport(),
                greeter.config(),
            )?;

            let socket_addr = socket.listener_addr().unwrap();
            debug!("redirect address: {}", socket_addr);

            let resp = RegisterClientResponse {
                //redirect: format!("192.168.2.106:{}", client_id),
                redirect: socket_addr.to_string(),
                error: String::new(),
            };

            println!("peer_addr: {:?}", peer_addr);
            greeter.send_payload(resp, Some(peer_addr.clone()))?;
            greeter.disconnect(Some(greeter.listener_addr()?));
            // greeter.send_payload(resp, None)?;

            debug!("responded to client: {}", self.port_count);

            debug!("client is blocking? {}", req.is_blocking);
            let client = Client {
                id: self.port_count,
                addr: peer_addr.to_string(),
                connection: socket,
                is_blocking: req.is_blocking,
                keepalive: self.config.client_keepalive,
                last_event: Instant::now(),
                auth_pair: None,
                name: "".to_string(),
                furthest_step: match &self.sim {
                    SimConnection::Local(sim) => sim.get_clock(),
                    SimConnection::ClusterCoord(coord) => coord.central.get_clock(),
                    SimConnection::ClusterWorker(worker) => {
                        if let Some(node) = &worker.sim_node {
                            node.clock
                        } else {
                            unimplemented!()
                        }
                    }
                },
                scheduled_dts: Default::default(),
                scheduled_queries: Default::default(),
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
        debug!("handling event: {:?}", event);
        let encoding = self
            .clients
            .get(client_id)
            .unwrap()
            .connection
            .encoding()
            .clone();
        match event.type_ {
            SocketEventType::Heartbeat => (),
            SocketEventType::Bytes => {
                self.handle_message(Message::from_bytes(event.bytes, &encoding)?, client_id)?
            }
            SocketEventType::Connect => println!("new connection event from client: {}", client_id),
            SocketEventType::Disconnect => {
                println!("disconnected event from client: {}", client_id)
            }
            _ => unimplemented!(),
        }
        trace!("handled");
        Ok(())
    }

    fn handle_message(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        match msg.type_ {
            // MessageKind::Heartbeat => (),
            MessageType::PingRequest => self.handle_ping_request(msg, client_id)?,
            MessageType::StatusRequest => self.handle_status_request(msg, client_id)?,
            MessageType::TurnAdvanceRequest => self.handle_turn_advance_request(msg, client_id)?,

            MessageType::QueryRequest => self.handle_query_request(msg, client_id)?,
            MessageType::NativeQueryRequest => self.handle_native_query_request(msg, client_id)?,
            MessageType::JsonPullRequest => self.handle_json_pull_request(msg, client_id)?,
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
            MessageType::SpawnEntitiesRequest => {
                self.handle_spawn_entities_request(msg, client_id)?
            }
            MessageType::ExportSnapshotRequest => {
                self.handle_export_snapshot_request(msg, client_id)?
            }
            _ => println!("unknown message type: {:?}", msg.type_),
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

        client.connection.send_payload(resp, None)
    }

    pub fn handle_spawn_entities_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let client = self.clients.get(client_id).unwrap();
        let mut out_names = Vec::new();
        let mut error = String::new();
        let req: SpawnEntitiesRequest = msg.unpack_payload(client.connection.encoding())?;

        for (i, prefab) in req.entity_prefabs.iter().enumerate() {
            trace!("handling prefab: {}", prefab);
            let entity_name = match req.entity_names[i].as_str() {
                "" => None,
                _ => Some(arraystring::new_truncate(&req.entity_names[i])),
            };
            match &mut self.sim {
                SimConnection::Local(sim) => {
                    match sim.spawn_entity(
                        Some(&outcome::arraystring::new_truncate(&prefab)),
                        entity_name,
                    ) {
                        Ok(entity_id) => out_names.push(entity_id.to_string()),
                        Err(e) => error = e.to_string(),
                    }
                }
                _ => unimplemented!(),
            }
        }
        let resp = SpawnEntitiesResponse {
            entity_names: out_names,
            error,
        };

        client.connection.send_payload(resp, None)
    }

    pub fn handle_ping_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let client = self.clients.get_mut(client_id).unwrap();
        let req: PingRequest = msg.unpack_payload(client.connection.encoding())?;
        let resp = PingResponse { bytes: req.bytes };
        client.connection.send_payload(resp, None)
    }

    pub fn handle_status_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let connected_clients = self.clients.iter().map(|(id, c)| c.name.clone()).collect();
        let mut client = self.clients.get_mut(client_id).unwrap();
        let req: StatusRequest = msg.unpack_payload(client.connection.encoding())?;
        let model_scenario = match &self.sim {
            SimConnection::Local(sim) => sim.model.scenario.clone(),
            SimConnection::ClusterCoord(coord) => coord.central.model.scenario.clone(),
            SimConnection::ClusterWorker(worker) => {
                if let Some(node) = &worker.sim_node {
                    node.model.scenario.clone()
                } else {
                    unimplemented!()
                }
            }
        };
        let resp = StatusResponse {
            name: self.config.name.clone(),
            description: self.config.description.clone(),
            // address: self.greeters.first().unwrap().local_addr()?.to_string(),
            connected_clients,
            engine_version: outcome_core::VERSION.to_owned(),
            uptime: self.uptime.as_millis() as usize,
            current_tick: match &self.sim {
                SimConnection::Local(sim) => sim.get_clock(),
                SimConnection::ClusterCoord(coord) => coord.central.get_clock(),
                SimConnection::ClusterWorker(worker) => worker.sim_node.as_ref().unwrap().clock,
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
        trace!("sending status response");
        trace!("client addr string: {}", client.addr);
        client
            .connection
            .send_payload(resp, Some(client.addr.parse()?))
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
                panic!("failed unpacking payload: {}", e);
            }
        };
        let mut data_pack = TypedSimDataPack::empty();
        match &mut self.sim {
            SimConnection::Local(sim_instance) => {
                handle_data_transfer_request_local(&dtr, sim_instance, client)?
            }
            SimConnection::ClusterCoord(coord) => {
                let mut collection = Vec::new();
                match dtr.transfer_type.as_str() {
                    "Full" => {
                        for (worker_id, worker) in &mut coord.net.workers {
                            worker.connection.send_sig(
                                crate::sig::Signal::from(0, outcome::distr::Signal::DataRequestAll),
                                None,
                            )?
                        }
                        for (worker_id, worker) in &mut coord.net.workers {
                            let (_, sig) = worker.connection.recv_sig()?;
                            match sig.into_inner().1 {
                                outcome::distr::Signal::DataResponse(data) => {
                                    collection.extend(data)
                                }
                                s => warn!("unhandled signal: {:?}", s),
                            }
                        }
                        let mut sdp = TypedSimDataPack::empty();
                        for (addr, var) in collection {
                            match addr.var_type {
                                VarType::String => {
                                    sdp.strings.insert(addr.into(), var.to_string());
                                }
                                VarType::Int => {
                                    sdp.ints.insert(addr.into(), var.to_int());
                                }
                                VarType::Float => {
                                    sdp.floats.insert(addr.into(), var.to_float());
                                }
                                _ => (),
                            }
                        }

                        let response = DataTransferResponse {
                            data: TransferResponseData::Typed(sdp),
                        };
                        client.connection.send_payload(response, None)?;
                    }
                    _ => unimplemented!(),
                }
            }
            SimConnection::ClusterWorker(worker) => {
                //TODO
                // categorize worker connection to the cluster, whether it's only connected
                // to the coordinator, to coord and to all workers, or some other way
                worker.network.sig_send_central(0, Signal::DataRequestAll)?;

                // for (worker_id, worker) in &mut worker.network.comrades {
                //     let (_, sig) = worker.connection.recv_sig()?;
                //     match sig.into_inner() {
                //         outcome::distr::Signal::DataResponse(data) => {
                //             collection.extend(data)
                //         }
                //         _ => unimplemented!(),
                //     }
                // }

                let (task_id, resp) = worker.network.sig_read_central()?;
                if let Signal::DataResponse(data_vec) = resp {
                    let mut data_pack = VarSimDataPack::default();
                    for (addr, var) in data_vec {
                        data_pack
                            .vars
                            .insert((addr.entity, addr.component, addr.var_id), var);
                    }
                    for (entity_id, entity) in &worker.sim_node.as_ref().unwrap().entities {
                        for ((comp_name, var_name), var) in &entity.storage.map {
                            data_pack.vars.insert(
                                (
                                    outcome::EntityName::from(&entity_id.to_string()).unwrap(),
                                    *comp_name,
                                    *var_name,
                                ),
                                var.clone(),
                            );
                        }
                    }

                    let response = DataTransferResponse {
                        data: TransferResponseData::Var(data_pack),
                    };
                    client.connection.send_payload(response, None)?;
                }
            }
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
                panic!("failed unpacking payload: {}", e);
            }
        };
        let mut data_pack = TypedSimDataPack::empty();
        match &mut self.sim {
            SimConnection::Local(sim_instance) => {
                let model = &sim_instance.model;
                match dtr.transfer_type.as_str() {
                    "Full" => {
                        // let mut data_pack = outcome::query::AddressedTypedMap::default();
                        let mut data_pack = TypedSimDataPack::empty();
                        for (entity_uid, entity) in &sim_instance.entities {
                            for ((comp_name, var_id), v) in entity.storage.map.iter() {
                                if v.is_float() {
                                    data_pack.floats.insert(
                                        // format!(
                                        //     ":{}:{}:{}:{}",
                                        //     // get entity string id if available
                                        //     sim_instance
                                        //         .entities_idx
                                        //         .iter()
                                        //         .find(|(e_id, e_idx)| e_idx == &entity_uid)
                                        //         .map(|(e_id, _)| e_id.as_str())
                                        //         .unwrap_or(entity_uid.to_string().as_str()),
                                        //     comp_name,
                                        //     VarType::Float.to_str(),
                                        //     var_id
                                        // ),
                                        Address {
                                            // get entity string id if available
                                            entity: sim_instance
                                                .entity_idx
                                                .iter()
                                                .find(|(e_id, e_idx)| e_idx == &entity_uid)
                                                .map(|(e_id, _)| *e_id)
                                                .unwrap_or(
                                                    outcome::EntityName::from(
                                                        &entity_uid.to_string(),
                                                    )
                                                    .unwrap(),
                                                ),
                                            // entity: entity_uid.parse().unwrap(),
                                            component: *comp_name,
                                            var_type: VarType::Float,
                                            var_id: *var_id,
                                        }
                                        .into(),
                                        // comp_name.to_string(),
                                        *v.as_float().unwrap(),
                                    );
                                }
                            }
                        }

                        let response = TypedDataTransferResponse {
                            data: data_pack,
                            error: String::new(),
                        };
                        client.connection.send_payload(response, None);
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
        client.connection.send_payload(resp, None)
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
        client.connection.send_payload(resp, None)
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
        client.connection.send_payload(resp, None)
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
            let mut data_pack = VarSimDataPack::default();
            for (entity_uid, entity) in &sim_instance.entities {
                for ((comp_name, var_id), v) in entity.storage.map.iter() {
                    data_pack.vars.insert(
                        // format!(
                        //     "{}:{}:{}:{}",
                        //     entity_uid,
                        //     comp_name,
                        //     v.get_type().to_str(),
                        //     var_id
                        // ),
                        (
                            arraystring::new_truncate(&entity_uid.to_string()),
                            *comp_name,
                            *var_id,
                        ),
                        v.clone(),
                    );
                }
            }

            let response = DataTransferResponse {
                data: TransferResponseData::Var(data_pack),
            };
            client.connection.send_payload(response, None)
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
                            .insert(address.into(), *var.as_float().unwrap());
                    }
                }
            }

            let response = DataTransferResponse {
                data: TransferResponseData::Typed(data_pack),
            };
            client.connection.send_payload(response, None)
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
                    data: TransferResponseData::VarOrdered(order_id, data),
                };
                client.connection.send_payload(response, None)
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
                    data: TransferResponseData::VarOrdered(order_id, data),
                };
                client.connection.send_payload(response, None)
            }
        }
        _ => Err(Error::Unknown),
    }
}

impl Server {
    // assumes the same task id for task on coord and server level
    fn handle_coord_tasks(
        tasks: &mut HashMap<TaskId, ServerTask>,
        clients: &HashMap<ClientId, Client>,
        coord: &mut Coord,
    ) -> Result<()> {
        let mut finished_tasks = Vec::new();
        for (task_id, coord_task) in &mut coord.tasks {
            if coord_task.is_finished() {
                finished_tasks.push(*task_id);
            }
        }
        for task_id in finished_tasks {
            if let Some(coord_task) = coord.tasks.remove(&task_id) {
                if coord_task.is_finished() {
                    println!("task {} is finished", task_id);
                    if let Some(server_task) = tasks.get(&task_id) {
                        match server_task {
                            ServerTask::WaitForCoordQueryResponse(client_id) => {
                                if let Some(client) = clients.get(client_id) {
                                    match coord_task {
                                        CoordTask::WaitForQueryResponses { products, .. } => {
                                            let qp =
                                                outcome::query::QueryProduct::combine(products);
                                            // if let outcome::query::QueryProduct::AddressedTyped(
                                            //     atm,
                                            // ) = qp
                                            // {}
                                            client.connection.send_payload(
                                                TypedDataTransferResponse {
                                                    data: TypedSimDataPack::from_query_product(qp),
                                                    error: "".to_string(),
                                                },
                                                // NativeQueryResponse {
                                                //     query_product: qp,
                                                //     error: None,
                                                // },
                                                None,
                                            )?;
                                        }
                                        _ => (),
                                    }
                                }
                            }
                        }
                    }
                }
            }
            tasks.remove(&task_id);
        }
        Ok(())
    }
}
