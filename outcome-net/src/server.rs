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

use crate::{error::Error, Result};
use std::thread::sleep;
use zmq::PollEvents;

pub const SERVER_ADDRESS: &str = "0.0.0.0:9124";
pub const GREETER_ADDRESS: &str = "0.0.0.0:9123";

pub type ClientId = u32;

/// Connection entry point for clients.
///
/// ## Network interface overview
///
/// Server's main job is keeping track of the connected `Client`s and handling
/// any requests they may send it's way. It also provides a pipe-like, one-way
/// communication for fast transport of queried data. All of these requirements
/// are reflected in how the interface is designed.
///
/// ### Listening to client connections
///
/// Server exposes a single stable listener at a known port. Any clients that
/// wish to connect have to send a proper request to this main address. The
/// `accept` function is used to accept new incoming client connections.
/// Here the client is assigned a unique id. Response includes a new address
/// to which client should connect.
///
/// ### Initiating client connections
///
/// Server is not only able to receive, but also to initiate connections with
/// clients. Sent connection request includes the socket address that the
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
    /// Entry point for incoming connections
    driver: ServerDriver,

    /// Whether the server uses a password to authorize connecting clients
    pub use_auth: bool,
    /// Passwords used for new client authorization
    pub passwd_list: Vec<String>,

    /// Connection point with the simulation
    pub sim: SimConnection,

    /// Uptime in seconds.
    pub uptime: f32,
    /// Time since last message
    pub time_since_last_msg: f32,

    /// Use lz4 compression for all messages if true.
    pub use_compression: bool,
}
impl Server {
    pub fn new(my_addr: &str, sim: SimConnection) -> Result<Server> {
        Ok(Server {
            name: "".to_string(),
            description: "".to_string(),
            address: "".to_string(),
            clients: HashMap::new(),
            driver: ServerDriver::new(my_addr)?,
            use_auth: false,
            passwd_list: vec![],
            sim,
            uptime: 0.0,
            time_since_last_msg: 0.0,
            use_compression: false,
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
    // pub fn accept_client(&mut self) -> Result<(u32, Message)> {
    //     self.driver.accept()
    // }
    pub fn try_accept_client(&mut self, redirect: bool) -> Result<(u32, PairSocket)> {
        let msg = self.driver.greeter.try_read()?;
        let req: RegisterClientRequest = msg.unpack_payload()?;
        self.driver.port_count += 1;
        let newport = format!("127.0.0.1:{}", self.driver.port_count);
        println!("newport: {}", newport);
        let mut client_socket = self.driver.new_connection()?;
        client_socket.bind(&newport)?;
        // let client_socket = client_socket;
        println!("req.addr: {:?}", req.addr);

        let resp = RegisterClientResponse {
            //redirect: format!("192.168.2.106:{}", client_id),
            redirect: newport,
            error: String::new(),
        };
        self.driver
            .greeter
            .send(Message::from_payload(resp, false)?)?;
        println!("responded to client: {}", self.driver.port_count);

        let client = Client {
            id: self.driver.port_count,
            addr: "".to_string(),
            // connection: client_socket.clone(),
            is_blocking: false,
            event_trigger: "".to_string(),
            passwd: "".to_string(),
            name: "".to_string(),
            furthest_tick: match &self.sim {
                SimConnection::Local(sim) => sim.get_clock(),
                _ => unimplemented!(),
            },
        };
        self.clients.insert(self.driver.port_count, client);
        Ok((self.driver.port_count, client_socket))
    }
    /// Handle new client connection.
    pub fn handle_new_client_connection(
        server: Arc<Mutex<Server>>,
        client_id: &ClientId,
        client_socket: &mut PairSocket,
    ) -> Result<()> {
        // let _server = server.clone();
        loop {
            // println!("start loop");
            // sleep a little to make this thread less expensive
            // sleep(Duration::from_millis(10));
            let msg = match client_socket.read() {
                Ok(m) => m,
                Err(e) => {
                    // println!("{:?}", e);
                    continue;
                }
            };
            println!("about to handle");
            handle_message(msg, &server, client_id, client_socket)?;
        }
        // TODO
        // stream.shutdown(Shutdown::Both);
    }
}
pub struct MsgChannel {
    pub title: String,
    pub password: String,
    pub messages: Vec<String>,
}

pub enum SimConnection {
    Local(Sim),
    ClusterCoord(Coord),
    ClusterWorker(Worker),
}

/// Representation of a connected client.
pub struct Client {
    /// Unique id assigned at registration.
    pub id: ClientId,
    /// IP address of the client.
    pub addr: String,
    // /// Connection interface
    // pub connection: Arc<Mutex<PairSocket>>,
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
}

/// Handle message, delegating further processing to a specialized function.
fn handle_message(
    msg: Message,
    server: &Arc<Mutex<Server>>,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    let mut server = server.lock().unwrap();
    server.time_since_last_msg = 0.0;
    let payload = msg.payload.clone();
    match msg.kind.as_str() {
        // TODO enabling compression for incoming requests would require
        // rewriting this bit, sending whole msg to the handler instead of
        // just the payload
        PING_REQUEST => handle_ping_request(payload, &mut server, client_id, client_conn)?,
        STATUS_REQUEST => handle_status_request(payload, &mut server, client_id, client_conn)?,
        TURN_ADVANCE_REQUEST => {
            handle_turn_advance_request(payload, &mut server, client_id, client_conn)?
        }
        DATA_TRANSFER_REQUEST => {
            handle_data_transfer_request(payload, &mut server, client_id, client_conn)?
        }
        DATA_PULL_REQUEST => {
            handle_data_pull_request(payload, &mut server, client_id, client_conn)?
        }
        // LIST_LOCAL_SCENARIOS_REQUEST => {
        //     handle_list_local_scenarios_request(payload, server_arc, client_id)
        // }
        // LOAD_LOCAL_SCENARIO_REQUEST => {
        //     handle_load_local_scenario_request(payload, server_arc, client_id)
        // }
        // LOAD_REMOTE_SCENARIO_REQUEST => {
        //     handle_load_remote_scenario_request(payload, server_arc, client_id)
        // }
        _ => (),
    }
    println!("handled message: {}", &msg.kind);
    drop(server);
    Ok(())
    // println!("handled");
}

pub fn handle_ping_request(
    payload: Vec<u8>,
    server: &mut Server,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    let req: PingRequest = unpack_payload(&payload, false, None)?;
    let resp = PingResponse { bytes: req.bytes };
    client_conn.send(Message::from_payload(resp, false)?)
}
pub fn handle_status_request(
    payload: Vec<u8>,
    server: &mut Server,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    // println!("handling status request");
    let req: StatusRequest = unpack_payload(&payload, false, None)?;
    let model = match &server.sim {
        SimConnection::Local(sim) => &sim.model,
        SimConnection::ClusterCoord(sim) => &sim.main.model,
        _ => unimplemented!(),
    };
    let resp = StatusResponse {
        name: server.name.clone(),
        description: server.description.clone(),
        address: server.address.clone(),
        connected_clients: server
            .clients
            .iter()
            .map(|(id, c)| c.name.clone())
            .collect(),
        //TODO
        endgame_version: outcome_core::VERSION.to_owned(),
        uptime: server.uptime,
        current_tick: match &server.sim {
            SimConnection::Local(sim) => sim.get_clock(),
            _ => unimplemented!(),
        },
        scenario_name: model.scenario.manifest.name.clone(),
        scenario_title: model
            .scenario
            .manifest
            .title
            .clone()
            .unwrap_or("".to_string()),
        scenario_desc: model
            .scenario
            .manifest
            .desc
            .clone()
            .unwrap_or("".to_string()),
        scenario_desc_long: model
            .scenario
            .manifest
            .desc_long
            .clone()
            .unwrap_or("".to_string()),
        scenario_author: model
            .scenario
            .manifest
            .author
            .clone()
            .unwrap_or("".to_string()),
        scenario_website: model
            .scenario
            .manifest
            .website
            .clone()
            .unwrap_or("".to_string()),
        scenario_version: model.scenario.manifest.version.clone(),
        scenario_engine: model.scenario.manifest.engine.clone(),
        scenario_mods: model
            .scenario
            .manifest
            .mods
            .clone()
            .iter()
            .map(|smd| format!("{} ({})", smd.name, smd.version_req))
            .collect(),
        scenario_settings: model
            .scenario
            .manifest
            .settings
            .clone()
            .iter()
            .map(|(k, v)| format!("{} = {:?}", k, v))
            .collect(),
    };
    println!("sent status response");
    client_conn.send(Message::from_payload(resp, false)?)
}

pub fn handle_data_transfer_request(
    payload: Vec<u8>,
    server: &mut Server,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    println!("in handle_data_transfer_request");
    let dtr: DataTransferRequest = match unpack_payload(&payload, false, None) {
        Ok(r) => r,
        Err(e) => {
            let response = DataTransferResponse {
                data: None,
                error: "FailedUnpackingPayload".to_string(),
            };
            client_conn.send(Message::from_payload(response, false)?);
            // if let Ok(ms) = msg_size {
            //     println!("sent DataTransferResponse ({} KB)", ms as f32 / 1000.0);
            // }
            panic!("failed unpacking payload");
            // return Ok(());
        }
    };
    let mut data_pack = SimDataPack::empty();
    let sim_instance = match &server.sim {
        SimConnection::Local(s) => s,
        SimConnection::ClusterCoord(c) => unimplemented!(),
        _ => unimplemented!(),
    };
    let model = &sim_instance.model;
    match dtr.transfer_type.as_str() {
        "Full" => {
            for (entity_uid, entity) in &sim_instance.entities {
                for ((comp_name, var_id), v) in entity.storage.get_all_str() {
                    data_pack.strings.insert(
                        format!(
                            "/{}/{}/{}/{}",
                            entity_uid,
                            comp_name,
                            VarType::Str.to_str(),
                            var_id
                        ),
                        v.to_owned(),
                    );
                }
                for ((comp_name, var_id), v) in entity.storage.get_all_int() {
                    data_pack.ints.insert(
                        format!(
                            "/{}/{}/{}/{}",
                            entity_uid,
                            comp_name,
                            VarType::Int.to_str(),
                            var_id
                        ),
                        *v,
                    );
                }
                for ((comp_name, var_id), v) in entity.storage.get_all_float() {
                    data_pack.floats.insert(
                        format!(
                            "/{}/{}/{}/{}",
                            entity_uid,
                            comp_name,
                            VarType::Float.to_str(),
                            var_id
                        ),
                        *v,
                    );
                }
                for ((comp_name, var_id), v) in entity.storage.get_all_bool() {
                    data_pack.bools.insert(
                        format!(
                            "/{}/{}/{}/{}",
                            entity_uid,
                            comp_name,
                            VarType::Bool.to_str(),
                            var_id
                        ),
                        *v,
                    );
                }
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
            let sim_instance = match &server.sim {
                SimConnection::Local(s) => s,
                SimConnection::ClusterCoord(c) => unimplemented!(),
                _ => unimplemented!(),
            };
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
                match address.var_type {
                    VarType::Str => {
                        if let Some(s) = sim_instance.get_str(&address) {
                            data_pack.strings.insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::Int => {
                        if let Some(s) = sim_instance.get_int(&address) {
                            data_pack.ints.insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::Float => {
                        if let Some(s) = sim_instance.get_float(&address) {
                            data_pack.floats.insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::Bool => {
                        if let Some(s) = sim_instance.get_bool(&address) {
                            data_pack.bools.insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::StrList => {
                        if let Some(s) = sim_instance.get_str_list(&address) {
                            data_pack
                                .string_lists
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::IntList => {
                        if let Some(s) = sim_instance.get_int_list(&address) {
                            data_pack
                                .int_lists
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::FloatList => {
                        if let Some(s) = sim_instance.get_float_list(&address) {
                            data_pack
                                .float_lists
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::BoolList => {
                        if let Some(s) = sim_instance.get_bool_list(&address) {
                            data_pack
                                .bool_lists
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::StrGrid => {
                        if let Some(s) = sim_instance.get_str_grid(&address) {
                            data_pack
                                .string_grids
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::IntGrid => {
                        if let Some(s) = sim_instance.get_int_grid(&address) {
                            data_pack
                                .int_grids
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::FloatGrid => {
                        if let Some(s) = sim_instance.get_float_grid(&address) {
                            data_pack
                                .float_grids
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                    VarType::BoolGrid => {
                        if let Some(s) = sim_instance.get_bool_grid(&address) {
                            data_pack
                                .bool_grids
                                .insert(address.to_string(), s.to_owned());
                        }
                    }
                };
            }
        }
        _ => (),
    }
    let response = DataTransferResponse {
        data: Some(data_pack),
        error: String::new(),
    };
    // server
    //     .driver
    client_conn.send(Message::from_payload(response, server.use_auth)?)
    // let msg_size = send_message(message_from_payload(response, true), stream, Some(512000));
    // if let Ok(ms) = msg_size {
    //     println!("sent DataTransferResponse ({} KB)", ms as f32 / 1000.0);
    // }
}
fn handle_single_address(server: &Server) {}
pub fn handle_data_pull_request(
    payload: Vec<u8>,
    server: &mut Server,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    {
        let use_compression = server.use_compression.clone();
        // let sim_model = server.sim_model.clone();
        let mut sim_instance = match &mut server.sim {
            SimConnection::Local(s) => s,
            SimConnection::ClusterCoord(c) => unimplemented!(),
            _ => unimplemented!(),
        };
        //TODO
        let dpr: DataPullRequest = unpack_payload(&payload, use_compression, None)?;
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
        for (address, var) in dpr.data.string_grids {
            let addr = Address::from_str(&address).unwrap();
            *sim_instance.get_str_grid_mut(&addr).unwrap() = var;
        }
        for (address, var) in dpr.data.int_grids {
            let addr = Address::from_str(&address).unwrap();
            *sim_instance.get_int_grid_mut(&addr).unwrap() = var;
        }
        for (address, var) in dpr.data.float_grids {
            let addr = Address::from_str(&address).unwrap();
            *sim_instance.get_float_grid_mut(&addr).unwrap() = var;
        }
        for (address, var) in dpr.data.bool_grids {
            let addr = Address::from_str(&address).unwrap();
            *sim_instance.get_bool_grid_mut(&addr).unwrap() = var;
        }
    }
    let resp = DataPullResponse {
        error: String::new(),
    };
    // send_message(message_from_payload(resp, false), stream, None);
    client_conn.send(Message::from_payload(resp, false)?)
}

pub fn handle_turn_advance_request(
    payload: Vec<u8>,
    server: &mut Server,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    let req: TurnAdvanceRequest = unpack_payload(&payload, false, None)?;

    let mut client_furthest_tick = 0;
    {
        let mut no_blocking_clients = true;
        let current_tick = match &server.sim {
            SimConnection::Local(s) => s.get_clock(),
            SimConnection::ClusterCoord(c) => c.main.clock,
            _ => unimplemented!(),
        };
        println!("current_tick before: {}", current_tick);
        let mut common_furthest_tick = current_tick + 99999;
        for (id, client) in &mut server.clients {
            if &client.id == client_id {
                println!(
                    "client.furthest_tick: {}, current_tick: {}",
                    client.furthest_tick, current_tick
                );
                if client.furthest_tick - current_tick < req.tick_count as usize {
                    client.furthest_tick = client.furthest_tick + req.tick_count as usize;
                }
                client_furthest_tick = client.furthest_tick.clone();
            }
            if !client.is_blocking {
                println!("omit non-blocking client..");
                continue;
            } else {
                no_blocking_clients = false;
            }
            println!("client_furthest_tick inside loop: {}", client.furthest_tick);
            if client.furthest_tick == current_tick {
                common_furthest_tick = current_tick;
                break;
            }
            if client.furthest_tick < common_furthest_tick {
                common_furthest_tick = client.furthest_tick;
            }
        }
        if no_blocking_clients {
            let t = server.clients.get(&client_id).unwrap().furthest_tick;
            common_furthest_tick = t;
        }
        println!("common_furthest_tick: {}", common_furthest_tick);
        if common_furthest_tick > current_tick {
            // let sim_model = server.sim_model.clone();
            match &mut server.sim {
                SimConnection::Local(sim_instance) => {
                    for _ in 0..common_furthest_tick - current_tick {
                        sim_instance.step();
                        println!("processed single tick");
                        println!(
                            "common_furthest_tick: {}, current_tick: {}",
                            common_furthest_tick, current_tick
                        );
                    }
                    println!("current_tick after: {}", sim_instance.get_clock());
                }
                SimConnection::ClusterCoord(coord) => {
                    // TODO
                    // let mut addr_book = HashMap::new();
                    // for node in &coord.nodes {
                    //     addr_book.insert(node.id.clone(), node.connection.try_clone().unwrap());
                    // }
                    //coord.main.step(&coord.entity_node_map, &mut addr_book);
                }
                _ => unimplemented!(),
            };
        }

        // responses
        if common_furthest_tick == current_tick {
            let resp = TurnAdvanceResponse {
                error: "BlockedFully".to_string(),
            };
            println!("BlockedFully");
            client_conn.send(Message::from_payload(resp, false)?);
        } else if common_furthest_tick < client_furthest_tick {
            let resp = TurnAdvanceResponse {
                error: "BlockedPartially".to_string(),
            };
            println!("BlockedPartially");
            client_conn.send(Message::from_payload(resp, false)?);
        //        } else if common_furthest_tick == client_furthest_tick {
        } else {
            let resp = TurnAdvanceResponse {
                error: String::new(),
            };
            println!("Didn't block");
            client_conn.send(Message::from_payload(resp, false)?);
        }
    }
    Ok(())
}

pub fn handle_list_local_scenarios_request(
    payload: Vec<u8>,
    server_arc: Arc<Mutex<Server>>,
    client_id: &ClientId,
    client_conn: &PairSocket,
) -> Result<()> {
    let req: ListLocalScenariosRequest = unpack_payload(&payload, false, None)?;
    //TODO check `$working_dir/scenarios` for scenarios
    //
    //

    let resp = ListLocalScenariosResponse {
        scenarios: Vec::new(),
        error: String::new(),
    };
    client_conn.send(Message::from_payload(resp, false)?)
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
    client_conn.send(Message::from_payload(resp, false)?)
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
    client_conn.send(Message::from_payload(resp, false)?)
}
