extern crate outcome_core as outcome;
extern crate rmp_serde as rmps;
extern crate serde;

use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use self::rmps::{Deserializer, Serializer};
use self::serde::{Deserialize, Serialize};

use outcome::{Address, Sim, SimModel, VarType};

use crate::msg::*;
use crate::transport::{ServerDriverInterface, SocketInterface};
use crate::{Coord, PairSocket, ServerDriver, Worker};

use crate::coord::CoordNetwork;
use crate::{error::Error, Result};
use outcome_core::StringId;
use std::convert::TryInto;
use std::ops::{Deref, DerefMut};
use std::thread::{current, sleep};
use zmq::PollEvents;

pub const SERVER_ADDRESS: &str = "0.0.0.0:9124";
pub const GREETER_ADDRESS: &str = "0.0.0.0:9123";

pub type ClientId = u32;

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
    /// Name of the server
    pub name: String,
    /// Description of the server
    pub description: String,
    /// IP address of the server
    pub address: String,

    /// List of clients
    pub clients: HashMap<ClientId, Client>,
    /// Network driver
    driver: ServerDriver,

    /// Ues a password to authorize connecting clients
    pub use_auth: bool,
    /// Passwords used for new client authorization
    pub passwd_list: Vec<String>,
    /// Compress outgoing messages
    pub use_compression: bool,

    /// Connection point with the simulation
    pub sim: SimConnection,

    /// Uptime in milliseconds
    pub uptime: usize,
    /// Time since last message in milliseconds
    pub time_since_last_msg: usize,
}
impl Server {
    pub fn new(sim: SimConnection, my_addr: &str) -> Result<Server> {
        Ok(Server {
            name: "".to_string(),
            description: "".to_string(),
            address: my_addr.split(":").collect::<Vec<&str>>()[0].to_string(),
            clients: HashMap::new(),
            driver: ServerDriver::new(my_addr)?,
            use_auth: false,
            passwd_list: vec![],
            sim,
            uptime: 0,
            // time_since_last_msg: HashMap::new(),
            use_compression: false,
            time_since_last_msg: 0,
        })
    }
    fn prune_clients(&mut self) {
        let mut buf = [0; 1024];
        let mut bad: Vec<ClientId> = Vec::new();
        for (client_id, client) in self.clients.iter() {
            // TODO
            // if client.stream.is_none() {
            //     println!("client stream is none");
            //     bad.push(client_id.clone());
            //     continue;
            // }
            // match client.stream.as_ref().unwrap().peek(&mut buf) {
            //     Ok(0) => {
            //         println!(
            //             "connection with client was lost: {}",
            //             client.addr.to_string()
            //         );
            //         bad.push(client_id.clone());
            //     }
            //     Ok(_) => {
            //         //
            //     }
            //     Err(e) => {
            //         //
            //     }
            // }
        }
        for b in bad {
            self.clients.remove(&b);
        }
        println!("remaining clients: {}", self.clients.len());
        //        let mut good: HashMap<u32, Client> = HashMap::new();
        //        for n in 0..self.clients.len() {
        //            let client = self.clients.pop().unwrap();
        //            if client.stream.is_none() {
        //                println!("client stream is none");
        //                continue;
        //            }
        //            match client.stream.as_ref().unwrap().peek(&mut buf) {
        //                Ok(0) => println!("connection with client was lost: {}", client.ip_addr.to_string()),
        //                Ok(_) => {
        //                    good.push(client);
        //                }
        //                Err(e) => {
        //                    good.push(client);
        //                },
        //            }
        //        }
        //        println!("remaining clients: {}", good.len());
        //        self.clients = good;
    }
}
impl Server {
    pub fn try_accept_client(&mut self, redirect: bool) -> Result<u32> {
        let msg = self.driver.greeter.try_read_msg(None)?;
        let req: RegisterClientRequest = msg.unpack_payload()?;
        self.driver.port_count += 1;
        let newport = format!("{}:{}", self.address, self.driver.port_count);
        debug!("newport: {}", newport);
        let mut client_socket = self.driver.new_connection()?;
        client_socket.bind(&newport)?;
        // let client_socket = client_socket;
        debug!("req.addr: {:?}", req.addr);

        let resp = RegisterClientResponse {
            //redirect: format!("192.168.2.106:{}", client_id),
            redirect: newport,
            error: String::new(),
        };
        self.driver
            .greeter
            .send_msg(Message::from_payload(resp, false)?)?;
        debug!("responded to client: {}", self.driver.port_count);
        debug!("client is blocking? {}", req.is_blocking);
        let client = Client {
            id: self.driver.port_count,
            addr: "".to_string(),
            // connection: client_socket.clone(),
            connection: client_socket,
            is_blocking: req.is_blocking,
            event_trigger: "".to_string(),
            passwd: "".to_string(),
            name: "".to_string(),
            furthest_tick: match &self.sim {
                SimConnection::Local(sim) => sim.get_clock(),
                SimConnection::ClusterCoord(coord, net) => {
                    coord.lock().unwrap().central.get_clock()
                }
                _ => unimplemented!(),
            },
            scheduled_dts: Default::default(),
        };

        self.clients.insert(self.driver.port_count, client);
        Ok(self.driver.port_count)
    }

    pub fn try_handle_client(&mut self, client_id: &ClientId) -> Result<()> {
        let msg = self
            .clients
            .get(client_id)
            .unwrap()
            .connection
            .try_read_msg(None)?;
        self.handle_message(msg, client_id)
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
}
pub struct MsgChannel {
    pub title: String,
    pub password: String,
    pub messages: Vec<String>,
}

pub enum SimConnection {
    Local(Sim),
    ClusterCoord(Arc<Mutex<Coord>>, Arc<Mutex<CoordNetwork>>),
    ClusterWorker(Worker),
}

/// Representation of a connected client.
pub struct Client {
    /// Unique id assigned at registration.
    pub id: ClientId,
    /// IP address of the client.
    pub addr: String,
    /// Connection interface
    // pub connection: Arc<Mutex<PairSocket>>,
    pub connection: PairSocket,
    /// Blocking client has to explicitly agree to let server continue to next turn,
    /// non-blocking client is more of a passive observer.
    pub is_blocking: bool,
    /// Simulation tick event the client is interested in.
    pub event_trigger: String,
    /// Password used by the client.
    pub passwd: String,
    /// Client self-assigned name.
    pub name: String,
    /// Furthest tick client is ready to jump to.
    /// If this is bigger than the current tick that client
    /// counts as ready for processing to next common furthest tick.
    pub furthest_tick: usize,
    /// List of scheduled data transfers.
    pub scheduled_dts: HashMap<StringId, Vec<DataTransferRequest>>,
}
impl Server {
    /// Handle message, delegating further processing to a specialized function.
    fn handle_message(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        debug!("handling message: {}", msg.kind.clone());
        match msg.kind.as_str() {
            // TODO enabling compression for incoming requests would require
            // rewriting this bit, sending whole msg to the handler instead of
            // just the payload
            PING_REQUEST => self.handle_ping_request(msg, client_id)?,
            STATUS_REQUEST => self.handle_status_request(msg, client_id)?,
            TURN_ADVANCE_REQUEST => self.handle_turn_advance_request(msg, client_id)?,
            DATA_TRANSFER_REQUEST => self.handle_data_transfer_request(msg, client_id)?,
            SCHEDULED_DATA_TRANSFER_REQUEST => {
                self.handle_scheduled_data_transfer_request(msg, client_id)?
            }
            DATA_PULL_REQUEST => self.handle_data_pull_request(msg, client_id)?,
            // LIST_LOCAL_SCENARIOS_REQUEST => {
            //     handle_list_local_scenarios_request(payload, server_arc, client_id)
            // }
            // LOAD_LOCAL_SCENARIO_REQUEST => {
            //     handle_load_local_scenario_request(payload, server_arc, client_id)
            // }
            // LOAD_REMOTE_SCENARIO_REQUEST => {
            //     handle_load_remote_scenario_request(payload, server_arc, client_id)
            // }
            HEARTBEAT => (),
            _ => println!("unknown message type: {}", msg.kind.as_str()),
        }

        Ok(())
        // println!("handled");
    }

    pub fn handle_ping_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let req: PingRequest = msg.unpack_payload()?;
        let resp = PingResponse { bytes: req.bytes };
        self.clients
            .get(client_id)
            .unwrap()
            .connection
            .send_msg(Message::from_payload(resp, false)?)
    }
    pub fn handle_status_request(&mut self, msg: Message, client_id: &ClientId) -> Result<()> {
        let req: StatusRequest = msg.unpack_payload()?;
        let model_scenario = match &self.sim {
            SimConnection::Local(sim) => sim.model.scenario.clone(),
            SimConnection::ClusterCoord(coord, coord_net) => {
                coord.lock().unwrap().central.model.scenario.clone()
            }
            _ => unimplemented!(),
        };
        let resp = StatusResponse {
            name: self.name.clone(),
            description: self.description.clone(),
            address: self.address.clone(),
            connected_clients: self.clients.iter().map(|(id, c)| c.name.clone()).collect(),
            //TODO
            endgame_version: outcome_core::VERSION.to_owned(),
            uptime: self.uptime,
            current_tick: match &self.sim {
                SimConnection::Local(sim) => sim.get_clock(),
                SimConnection::ClusterCoord(coord, coord_net) => {
                    coord.lock().unwrap().central.get_clock()
                }
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
        self.clients
            .get(client_id)
            .unwrap()
            .connection
            .send_msg(Message::from_payload(resp, false)?)
    }

    pub fn handle_data_transfer_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let dtr: DataTransferRequest = match msg.unpack_payload() {
            Ok(r) => r,
            Err(e) => {
                let response = DataTransferResponse {
                    data: None,
                    error: "FailedUnpackingPayload".to_string(),
                };
                self.clients
                    .get(client_id)
                    .unwrap()
                    .connection
                    .send_msg(Message::from_payload(response, false)?);
                // if let Ok(ms) = msg_size {
                //     println!("sent DataTransferResponse ({} KB)", ms as f32 / 1000.0);
                // }
                panic!("failed unpacking payload: {}", e);
                // return Ok(());
            }
        };
        let mut data_pack = SimDataPack::empty();
        match &self.sim {
            SimConnection::ClusterCoord(coord, net) => {
                let coord = coord.lock().unwrap();
                let net = net.lock().unwrap();
                let mut collection = Vec::new();
                match dtr.transfer_type.as_str() {
                    "Full" => {
                        for (worker_id, worker) in &net.workers {
                            worker.pair_sock.send(
                                crate::sig::Signal::from(outcome::distr::Signal::DataRequestAll)
                                    .to_bytes()?,
                            )?
                        }
                        for (worker_id, worker) in &net.workers {
                            let bytes = worker.pair_sock.read()?;
                            let sig = crate::sig::Signal::from_bytes(&bytes)?.inner();
                            match sig {
                                outcome::distr::Signal::DataResponse(data) => {
                                    collection.extend(data)
                                }
                                _ => unimplemented!(),
                            }
                        }
                        let mut sdp = SimDataPack::empty();
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
                            data: Some(sdp),
                            error: String::new(),
                        };
                        self.clients
                            .get(client_id)
                            .unwrap()
                            .connection
                            .send_msg(Message::from_payload(response, self.use_auth)?);
                    }
                    _ => unimplemented!(),
                }
            }
            SimConnection::Local(sim_instance) => handle_data_transfer_request_local(
                &dtr,
                sim_instance,
                &self.clients.get(client_id).unwrap().connection,
            )?,
            _ => unimplemented!(),
        };

        Ok(())
    }

    pub fn handle_scheduled_data_transfer_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let sdtr: ScheduledDataTransferRequest = msg.unpack_payload()?;
        let mut client = self.clients.get_mut(client_id).unwrap();
        for event_trigger in sdtr.event_triggers {
            let event_id = StringId::from(&event_trigger)?;
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
        {
            let use_compression = self.use_compression.clone();
            // let sim_model = server.sim_model.clone();
            let mut sim_instance = match &mut self.sim {
                SimConnection::Local(s) => s,
                SimConnection::ClusterCoord(c, net) => unimplemented!(),
                _ => unimplemented!(),
            };
            //TODO
            let dpr: DataPullRequest = msg.unpack_payload()?;
            //TODO handle errors
            for (address, var) in dpr.data.strings {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_str_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.ints {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_int_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.floats {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_float_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.bools {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_bool_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.string_lists {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_str_list_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.int_lists {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_int_list_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.float_lists {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_float_list_mut(&addr).unwrap() = var;
            }
            for (address, var) in dpr.data.bool_lists {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_bool_list_mut(&addr).unwrap() = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in dpr.data.string_grids {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_str_grid_mut(&addr).unwrap() = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in dpr.data.int_grids {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_int_grid_mut(&addr).unwrap() = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in dpr.data.float_grids {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_float_grid_mut(&addr).unwrap() = var;
            }
            #[cfg(feature = "outcome/grids")]
            for (address, var) in dpr.data.bool_grids {
                let addr = Address::from_str(&address).unwrap();
                *sim_instance.get_bool_grid_mut(&addr).unwrap() = var;
            }
        }
        let resp = DataPullResponse {
            error: String::new(),
        };
        // send_message(message_from_payload(resp, false), stream, None);
        self.clients
            .get(client_id)
            .unwrap()
            .connection
            .send_msg(Message::from_payload(resp, false)?)
    }

    pub fn handle_turn_advance_request(
        &mut self,
        msg: Message,
        client_id: &ClientId,
    ) -> Result<()> {
        let req: TurnAdvanceRequest = msg.unpack_payload()?;

        let mut client_furthest_tick = 0;
        {
            let mut no_blocking_clients = true;
            let current_tick = match &self.sim {
                SimConnection::Local(s) => s.get_clock(),
                SimConnection::ClusterCoord(c, net) => c.lock().unwrap().central.clock,
                _ => unimplemented!(),
            };
            trace!("current_tick before: {}", current_tick);
            let mut common_furthest_tick = current_tick + 99999;
            for (id, client) in &mut self.clients {
                if &client.id == client_id {
                    trace!(
                        "({}) furthest_tick: {}, current_tick: {}",
                        client.id,
                        client.furthest_tick,
                        current_tick
                    );
                    if client.furthest_tick < current_tick {
                        client.furthest_tick = current_tick;
                    }
                    if client.furthest_tick - current_tick < req.tick_count as usize {
                        client.furthest_tick = client.furthest_tick + req.tick_count as usize;
                    }
                    client_furthest_tick = client.furthest_tick.clone();
                }
                if !client.is_blocking {
                    trace!("omit non-blocking client..");
                    continue;
                } else {
                    no_blocking_clients = false;
                }
                trace!("client_furthest_tick inside loop: {}", client.furthest_tick);
                if client.furthest_tick == current_tick {
                    common_furthest_tick = current_tick;
                    break;
                }
                if client.furthest_tick < common_furthest_tick {
                    common_furthest_tick = client.furthest_tick;
                }
            }
            if no_blocking_clients {
                let t = self.clients.get(&client_id).unwrap().furthest_tick;
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
                            for (_, client) in &self.clients {
                                for (event, dts_list) in &client.scheduled_dts {
                                    trace!("handling scheduled data transfer: event: {}", event);
                                    if sim_instance.event_queue.contains(&event) {
                                        for dtr in dts_list {
                                            trace!(
                                                "handling scheduled data transfer: dtr: {:?}",
                                                dtr
                                            );
                                            handle_data_transfer_request_local(
                                                dtr,
                                                sim_instance,
                                                &client.connection,
                                            )?
                                        }
                                    }
                                }
                            }
                        }
                        trace!("current_tick after: {}", sim_instance.get_clock());
                    }
                    SimConnection::ClusterCoord(coord, net) => {
                        let mut coord_lock = coord.lock().unwrap();
                        let mut net_lock = net.lock().unwrap();
                        let mut event_queue = coord_lock.central.event_queue.clone();

                        let step_event_name = StringId::from_unchecked("step");
                        if !event_queue.contains(&step_event_name) {
                            event_queue.push(step_event_name);
                        }
                        coord_lock.central.event_queue.clear();

                        // let network = &coord_lock.network;
                        // let central = &mut coord_lock.central;
                        coord_lock
                            .central
                            .step_network(net_lock.deref_mut(), event_queue);
                        // coord_lock
                        //     .central
                        //     .step_network(&mut coord_lock.network, event_queue)?;
                        coord_lock.central.clock += 1;

                        // let mut addr_book = HashMap::new();
                        // for node in &coord.nodes {
                        //     addr_book.insert(node.id.clone(), node.connection.try_clone().unwrap());
                        // }
                        //coord.main.step(&coord.entity_node_map, &mut addr_book);
                    }
                    _ => unimplemented!(),
                };
            }

            let client_conn = &self.clients.get(client_id).unwrap().connection;
            // responses
            if common_furthest_tick == current_tick {
                let resp = TurnAdvanceResponse {
                    error: "BlockedFully".to_string(),
                };
                trace!("BlockedFully");
                client_conn.send_msg(Message::from_payload(resp, false)?);
            } else if common_furthest_tick < client_furthest_tick {
                let resp = TurnAdvanceResponse {
                    error: "BlockedPartially".to_string(),
                };
                trace!("BlockedPartially");
                client_conn.send_msg(Message::from_payload(resp, false)?);
            //        } else if common_furthest_tick == client_furthest_tick {
            } else {
                let resp = TurnAdvanceResponse {
                    error: String::new(),
                };
                trace!("Didn't block");
                client_conn.send_msg(Message::from_payload(resp, false)?);
            }
        }
        Ok(())
    }

    pub fn handle_list_local_scenarios_request(
        &mut self,
        payload: Vec<u8>,
        client_id: &ClientId,
    ) -> Result<()> {
        let req: ListLocalScenariosRequest = unpack_payload(&payload, false, None)?;
        //TODO check `$working_dir/scenarios` for scenarios
        //
        //

        let resp = ListLocalScenariosResponse {
            scenarios: Vec::new(),
            error: String::new(),
        };
        self.clients
            .get(client_id)
            .unwrap()
            .connection
            .send_msg(Message::from_payload(resp, false)?)
    }
    pub fn handle_load_local_scenario_request(
        payload: Vec<u8>,
        server_arc: Arc<Mutex<Server>>,
        client_id: &ClientId,
        client_conn: &PairSocket,
    ) -> Result<()> {
        let req: LoadLocalScenarioRequest = unpack_payload(&payload, false, None)?;

        //TODO
        //

        let resp = LoadLocalScenarioResponse {
            error: String::new(),
        };
        client_conn.send_msg(Message::from_payload(resp, false)?)
    }
    pub fn handle_load_remote_scenario_request(
        payload: Vec<u8>,
        server_arc: Arc<Mutex<Server>>,
        client_id: &ClientId,
        client_conn: &PairSocket,
    ) -> Result<()> {
        let req: LoadRemoteScenarioRequest = unpack_payload(&payload, false, None)?;

        //TODO
        //

        let resp = LoadRemoteScenarioResponse {
            error: String::new(),
        };
        client_conn.send_msg(Message::from_payload(resp, false)?)
    }
}

fn handle_data_transfer_request_local(
    dtr: &DataTransferRequest,
    sim_instance: &Sim,
    client_conn: &PairSocket,
) -> Result<()> {
    let mut data_pack = SimDataPack::empty();
    let model = &sim_instance.model;
    match dtr.transfer_type.as_str() {
        "Full" => {
            for (entity_uid, entity) in &sim_instance.entities {
                for ((comp_name, var_id), v) in entity.storage.get_all_var() {
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
                // for ((comp_name, var_id), v) in entity.storage.get_all_str() {
                //     data_pack.strings.insert(
                //         format!(
                //             "/{}/{}/{}/{}",
                //             entity_uid,
                //             comp_name,
                //             VarType::Str.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_name, var_id), v) in entity.storage.get_all_int() {
                //     data_pack.ints.insert(
                //         format!(
                //             "/{}/{}/{}/{}",
                //             entity_uid,
                //             comp_name,
                //             VarType::Int.to_str(),
                //             var_id
                //         ),
                //         *v,
                //     );
                // }
                // for ((comp_name, var_id), v) in entity.storage.get_all_float() {
                //     data_pack.floats.insert(
                //         format!(
                //             "/{}/{}/{}/{}",
                //             entity_uid,
                //             comp_name,
                //             VarType::Float.to_str(),
                //             var_id
                //         ),
                //         *v,
                //     );
                // }
                // for ((comp_name, var_id), v) in entity.storage.get_all_bool() {
                //     data_pack.bools.insert(
                //         format!(
                //             "/{}/{}/{}/{}",
                //             entity_uid,
                //             comp_name,
                //             VarType::Bool.to_str(),
                //             var_id
                //         ),
                //         *v,
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_str_list() {
                //     data_pack.string_lists.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::StrList.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_int_list() {
                //     data_pack.int_lists.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::IntList.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_float_list() {
                //     data_pack.float_lists.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::FloatList.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_bool_list() {
                //     data_pack.bool_lists.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::BoolList.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_str_grid() {
                //     data_pack.string_grids.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::StrGrid.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_int_grid() {
                //     data_pack.int_grids.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::IntGrid.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_float_grid() {
                //     data_pack.float_grids.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::FloatGrid.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
                // for ((comp_type, comp_id, var_id), v) in entity.storage.get_all_bool_grid() {
                //     data_pack.bool_grids.insert(
                //         format!(
                //             "/{}/{}/{}/{}/{}",
                //             ent_suid,
                //             comp_type,
                //             comp_id,
                //             VarType::BoolGrid.to_str(),
                //             var_id
                //         ),
                //         v.to_owned(),
                //     );
                // }
            }
        }
        "Selected" => {
            let mut selected = Vec::new();
            selected.extend_from_slice(&dtr.selection);

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
                if let Some(var) = sim_instance.get_var(&address) {
                    if var.is_float() {
                        data_pack
                            .floats
                            .insert(address.to_string(), *var.as_float().unwrap());
                    }
                }
                // match address.var_type {
                //     VarType::Str => {
                //         if let Some(s) = sim_instance.get_str(&address) {
                //             data_pack.strings.insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::Int => {
                //         if let Some(s) = sim_instance.get_int(&address) {
                //             data_pack.ints.insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::Float => {
                //         if let Some(s) = sim_instance.get_float(&address) {
                //             data_pack.floats.insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::Bool => {
                //         if let Some(s) = sim_instance.get_bool(&address) {
                //             data_pack.bools.insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::StrList => {
                //         if let Some(s) = sim_instance.get_str_list(&address) {
                //             data_pack
                //                 .string_lists
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::IntList => {
                //         if let Some(s) = sim_instance.get_int_list(&address) {
                //             data_pack
                //                 .int_lists
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::FloatList => {
                //         if let Some(s) = sim_instance.get_float_list(&address) {
                //             data_pack
                //                 .float_lists
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     VarType::BoolList => {
                //         if let Some(s) = sim_instance.get_bool_list(&address) {
                //             data_pack
                //                 .bool_lists
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     #[cfg(feature = "grids")]
                //     VarType::StrGrid => {
                //         if let Some(s) = sim_instance.get_str_grid(&address) {
                //             data_pack
                //                 .string_grids
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     #[cfg(feature = "grids")]
                //     VarType::IntGrid => {
                //         if let Some(s) = sim_instance.get_int_grid(&address) {
                //             data_pack
                //                 .int_grids
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     #[cfg(feature = "grids")]
                //     VarType::FloatGrid => {
                //         if let Some(s) = sim_instance.get_float_grid(&address) {
                //             data_pack
                //                 .float_grids
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                //     #[cfg(feature = "grids")]
                //     VarType::BoolGrid => {
                //         if let Some(s) = sim_instance.get_bool_grid(&address) {
                //             data_pack
                //                 .bool_grids
                //                 .insert(address.to_string(), s.to_owned());
                //         }
                //     }
                // };
            }
        }
        _ => (),
    }
    let response = DataTransferResponse {
        data: Some(data_pack),
        error: String::new(),
    };
    client_conn.send_msg(Message::from_payload(response, false)?);
    Ok(())
}
