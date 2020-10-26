use crate::msg::{
    DataTransferRequest, DataTransferResponse, Heartbeat, Message, PingRequest,
    RegisterClientRequest, RegisterClientResponse, SimDataPack, StatusRequest, StatusResponse,
    TurnAdvanceRequest,
};
use crate::transport::{ClientDriverInterface, SocketInterface};
use crate::ClientDriver;
use crate::{error::Error, Result};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Connects to a server.
///
/// ## Blocking
///
/// A blocking client is one that has to explicitly signal it's ready to
/// proceed to next.
///
/// Blocking is handled on two levels - first on the level of a server, which
/// may have multiple blocking clients connected to it, and second on the level
/// of the coordinator, which has the ultimate authority when it comes to
/// advancing the simulation clock.
pub struct Client {
    /// Self-assigned name
    name: String,
    /// Networking context struct
    driver: Arc<Mutex<ClientDriver>>,
    /// Current connection status
    connected: bool,
    /// Blocking client requires server to wait for it's explicit step advance
    blocking: bool,
    /// Default compression setting
    compressing: bool,
    /// Public ip address of the client, `None` if behind a firewall
    public_addr: Option<String>,
    /// Frequency (millis) setting for heartbeat messages, `None` for no heartbeat
    heartbeat: Option<usize>,
}

impl Client {
    pub fn new(
        name: &str,
        blocking: bool,
        compressing: bool,
        public_addr: Option<String>,
        heartbeat: Option<usize>,
    ) -> Result<Client> {
        let client = Client {
            name: name.to_string(),
            driver: Arc::new(Mutex::new(ClientDriver::new()?)),
            connected: false,
            blocking,
            compressing,
            public_addr,
            heartbeat,
        };
        Ok(client)
    }
    pub fn is_blocking(&self) -> bool {
        self.blocking
    }
    pub fn is_compressing(&self) -> bool {
        self.compressing
    }
    pub fn is_connected(&self) -> bool {
        self.connected
    }
    /// Connects to server at the given address.
    ///
    /// Registration
    pub fn connect(&mut self, addr: String, password: Option<String>) -> Result<()> {
        // let my_addr = self.driver.my_addr();
        println!("public_addr: {:?}", self.public_addr);
        let msg = Message::from_payload(
            RegisterClientRequest {
                name: self.name.clone(),
                addr: self.public_addr.clone(),
                is_blocking: self.blocking,
                passwd: password,
            },
            false,
        )?;
        println!("attempting to dial server at: {}", addr);
        // self.driver.dial_server(addr, msg)?;
        let temp_client = self.driver.lock().unwrap().req_socket()?;
        //thread::sleep(Duration::from_millis(100));
        temp_client.connect(&crate::transport::zmq::tcp_endpoint(&addr));
        temp_client.send(msg)?;
        println!("dialed server at: {}", addr);

        let resp: RegisterClientResponse = temp_client.read()?.unpack_payload()?;
        match resp.redirect.as_str() {
            "" => (),
            _ => self
                .driver
                .lock()
                .unwrap()
                .connect_to_server(&resp.redirect, None)?,
        }
        match resp.error.as_str() {
            "" => (),
            _ => return Err(Error::Other(resp.error)),
        };
        self.connected = true;

        if let Some(heartbeat) = self.heartbeat {
            let driver_clone = self.driver.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(heartbeat as u64));
                let msg = Message::from_payload(Heartbeat {}, false).unwrap();
                driver_clone.lock().unwrap().send(msg).unwrap();
            });
        }

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.driver.lock().unwrap().disconnect()
    }

    pub fn server_status(&self) -> Result<HashMap<String, String>> {
        let req_msg = Message::from_payload(
            StatusRequest {
                format: "".to_string(),
            },
            false,
        )?;
        self.driver.lock().unwrap().send(req_msg)?;
        println!("sent server status request to server");
        let msg = self.driver.lock().unwrap().read()?;
        let resp: StatusResponse = msg.unpack_payload()?;
        let mut out_map = HashMap::new();
        out_map.insert("uptime".to_string(), format!("{}", resp.uptime));
        out_map.insert("current_tick".to_string(), format!("{}", resp.current_tick));
        Ok(out_map)
    }
    pub fn server_step_request(&self, steps: u32) -> Result<()> {
        let msg = Message::from_payload(TurnAdvanceRequest { tick_count: steps }, false)?;
        self.driver.lock().unwrap().send(msg)?;
        self.driver.lock().unwrap().read()?;
        Ok(())
        // unimplemented!();
    }

    // data querying
    pub fn get_var_as_string(&self, addr: &str) -> Result<String> {
        unimplemented!();
    }
    pub fn get_vars_as_strings(&self, addrs: &Vec<String>) -> Result<Vec<String>> {
        unimplemented!();
    }

    pub fn get_vars(&self) -> Result<SimDataPack> {
        let msg = Message::from_payload(
            DataTransferRequest {
                transfer_type: "Full".to_string(),
                selection: vec![],
            },
            false,
        )?;
        self.driver.lock().unwrap().send(msg)?;
        let resp: DataTransferResponse = self.driver.lock().unwrap().read()?.unpack_payload()?;
        if let Some(data_pack) = resp.data {
            return Ok(data_pack);
        }
        Ok(SimDataPack::empty())
    }
}
