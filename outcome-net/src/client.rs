use crate::msg::{
    Message, RegisterClientRequest, RegisterClientResponse, StatusRequest, StatusResponse,
    TurnAdvanceRequest,
};
use crate::transport::{ClientDriverInterface, SocketInterface};
use crate::ClientDriver;
use crate::{error::Error, Result};

use std::collections::HashMap;
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
    driver: ClientDriver,
    /// Defines whether this client is blocking or not
    blocking: bool,
    /// Default compression setting
    compress: bool,
    /// Public ip address of the client, none if it's behind a firewall
    public_addr: Option<String>,
}

impl Client {
    pub fn new(
        name: &str,
        blocking: bool,
        compress: bool,
        public_addr: Option<String>,
    ) -> Result<Client> {
        let client = Client {
            driver: ClientDriver::new()?,
            name: name.to_string(),
            blocking,
            compress,
            public_addr,
        };
        Ok(client)
    }
    pub fn is_blocking(&self) -> bool {
        self.blocking
    }
    pub fn is_compress(&self) -> bool {
        self.compress
    }
    /// Connects to server at the given address.
    ///
    /// Registration
    pub fn connect(&mut self, addr: String, password: Option<String>) -> Result<()> {
        // let my_addr = self.driver.my_addr();
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
        let temp_client = self.driver.req_socket()?;
        //thread::sleep(Duration::from_millis(100));
        temp_client.connect(&crate::transport::zmq::tcp_endpoint(&addr));
        temp_client.send(msg)?;
        println!("dialed server at: {}", addr);

        let resp: RegisterClientResponse = temp_client.read()?.unpack_payload()?;
        match resp.redirect.as_str() {
            "" => (),
            _ => self.driver.connect_to_server(&resp.redirect, None)?,
        }
        match resp.error.as_str() {
            "" => Ok(()),
            _ => Err(Error::Other(resp.error)),
        }
    }

    pub fn server_status(&self) -> Result<HashMap<String, String>> {
        let req_msg = Message::from_payload(
            StatusRequest {
                format: "".to_string(),
            },
            false,
        )?;
        self.driver.send(req_msg)?;
        println!("sent server status request to server");
        let msg = self.driver.read()?;
        //let mut msg;
        //loop {
        //if let Ok(_m) = self.driver.try_read() {
        //msg = _m;
        //} else {
        //continue;
        //}
        //}
        let resp: StatusResponse = msg.unpack_payload()?;
        let mut out_map = HashMap::new();
        out_map.insert("uptime".to_string(), format!("{}", resp.uptime));
        out_map.insert("current_tick".to_string(), format!("{}", resp.current_tick));
        Ok(out_map)
    }
    pub fn server_step_request(&self, steps: u32) -> Result<()> {
        let msg = Message::from_payload(TurnAdvanceRequest { tick_count: steps }, false)?;
        self.driver.send(msg)?;
        self.driver.read()?;
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

    pub fn get_vars_of_type(&self) -> Result<()> {
        unimplemented!();
    }
}
