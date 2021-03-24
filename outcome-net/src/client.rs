use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::msg::{
    DataTransferRequest, DataTransferResponse, Heartbeat, Message, PingRequest,
    RegisterClientRequest, RegisterClientResponse, ScheduledDataTransferRequest, StatusRequest,
    StatusResponse, TransferResponseData, TurnAdvanceRequest, TypedSimDataPack,
};
use crate::socket::{Encoding, Socket, SocketConfig, SocketType, Transport};
use crate::{error::Error, Result};

#[derive(Debug)]
pub enum CompressionPolicy {
    /// Compress all outgoing traffic
    Everything,
    /// Only compress messages larger than given size in bytes
    LargerThan(usize),
    /// Only compress data-heavy messages
    OnlyDataTransfers,
    /// Don't use compression
    Nothing,
}

impl CompressionPolicy {
    pub fn from_str(s: &str) -> Result<Self> {
        if s.starts_with("bigger_than_") || s.starts_with("larger_than_") {
            let split = s.split('_').collect::<Vec<&str>>();
            let number = split[2]
                .parse::<usize>()
                .map_err(|e| Error::Other(e.to_string()))?;
            return Ok(Self::LargerThan(number));
        }
        let c = match s {
            "all" | "everything" => Self::Everything,
            "data" | "only_data" => Self::OnlyDataTransfers,
            "none" | "nothing" => Self::Nothing,
            _ => {
                return Err(Error::Other(format!(
                    "failed parsing compression policy from string: {}",
                    s
                )))
            }
        };
        Ok(c)
    }
}

#[derive(Debug)]
pub struct ClientConfig {
    /// Self-assigned name
    pub name: String,
    /// Heartbeat frequency
    pub heartbeat: Option<Duration>,
    /// Blocking client requires server to wait for it's explicit step advance
    pub is_blocking: bool,
    /// Compression policy for outgoing messages
    pub compress: CompressionPolicy,
    /// Supported encodings, first is most preferred
    pub encodings: Vec<Encoding>,
    /// Supported transports
    pub transports: Vec<Transport>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            name: "default_client".to_string(),
            heartbeat: Some(Duration::from_secs(1)),
            is_blocking: false,
            compress: CompressionPolicy::OnlyDataTransfers,
            encodings: vec![Encoding::Bincode],
            transports: vec![Transport::Tcp],
        }
    }
}

/// Represents a connection to the server.
///
/// # Blocking client
///
/// A blocking client is one that has to explicitly signal it's ready to
/// proceed to next.
///
/// Blocking is handled on two levels - first on the level of a server, which
/// may have multiple blocking clients connected to it, and second on the level
/// of the coordinator, which has the ultimate authority when it comes to
/// advancing the simulation clock.
pub struct Client {
    /// Configuration struct
    config: ClientConfig,
    /// Connection to server
    pub connection: Socket,
    /// Current connection status
    connected: bool,
    /// Public ip address of the client, `None` if behind a firewall
    public_addr: Option<String>,
}

impl Client {
    pub fn new() -> Result<Self> {
        Self::new_with_config(None, ClientConfig::default())
    }

    pub fn new_with_config(addr: Option<String>, config: ClientConfig) -> Result<Self> {
        let transport = config
            .transports
            .first()
            .ok_or(Error::Other(
                "client config has to provide at least one transport option".to_string(),
            ))?
            .clone();
        let encoding = config
            .encodings
            .first()
            .ok_or(Error::Other(
                "client config has to provide at least one encoding option".to_string(),
            ))?
            .clone();
        let socket_config = SocketConfig {
            type_: SocketType::Pair,
            encoding,
            ..Default::default()
        };
        let connection = match addr {
            Some(a) => Socket::bind_with_config(&a, transport, socket_config)?,
            None => Socket::bind_any_with_config(transport, socket_config)?,
        };
        let client = Self {
            config,
            connection,
            connected: false,
            public_addr: None,
        };
        Ok(client)
    }

    /// Connects to server at the given address.
    ///
    /// # Redirection
    ///
    /// In it's response to client registration message, the server can specify
    /// at what address the target two-way connection shall take place. This
    /// prompts the client to reconnect at that new address.
    pub fn connect(&mut self, addr: String, password: Option<String>) -> Result<()> {
        debug!("client public_addr: {:?}", self.public_addr);
        info!("dialing server at: {}", addr);

        self.connection.connect(&addr)?;

        self.connection.pack_send_msg_payload(
            RegisterClientRequest {
                name: self.config.name.clone(),
                addr: self.public_addr.clone(),
                is_blocking: self.config.is_blocking,
                passwd: password,
            },
            None,
        )?;

        let resp: RegisterClientResponse = self
            .connection
            .recv_msg()?
            .1
            .unpack_payload(self.connection.encoding())?;

        debug!("got response from server: {:?}", resp);

        // perform redirection if server specified an alternative address
        if !resp.redirect.is_empty() {
            self.connection.disconnect(None)?;
            self.connection.connect(&resp.redirect)?;
        }

        if !resp.error.is_empty() {
            return Err(Error::Other(resp.error));
        }

        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.connection.disconnect(None)
    }

    pub fn server_status(&mut self) -> Result<HashMap<String, String>> {
        self.connection.pack_send_msg_payload(
            StatusRequest {
                format: "".to_string(),
            },
            None,
        )?;
        debug!("sent server status request to server");
        let (_, msg) = self.connection.recv_msg()?;
        let resp: StatusResponse = msg.unpack_payload(self.connection.encoding())?;
        let mut out_map = HashMap::new();
        out_map.insert("uptime".to_string(), format!("{}", resp.uptime));
        out_map.insert("current_tick".to_string(), format!("{}", resp.current_tick));
        Ok(out_map)
    }

    pub fn server_step_request(&mut self, steps: u32) -> Result<Message> {
        self.connection
            .pack_send_msg_payload(TurnAdvanceRequest { tick_count: steps }, None)?;
        let (_, resp) = self.connection.recv_msg()?;
        Ok(resp)
    }

    // data querying
    pub fn get_var_as_string(&self, addr: &str) -> Result<String> {
        unimplemented!();
    }
    pub fn get_vars_as_strings(&self, addrs: &Vec<String>) -> Result<Vec<String>> {
        unimplemented!();
    }

    pub fn get_vars(&mut self) -> Result<Option<TransferResponseData>> {
        self.connection.pack_send_msg_payload(
            DataTransferRequest {
                transfer_type: "Full".to_string(),
                selection: vec![],
            },
            None,
        )?;
        let resp: DataTransferResponse = self
            .connection
            .recv_msg()?
            .1
            .unpack_payload(self.connection.encoding())?;

        Ok(resp.data)
    }

    pub fn reg_scheduled_transfer(&mut self) -> Result<()> {
        self.connection.pack_send_msg_payload(
            ScheduledDataTransferRequest {
                event_triggers: vec!["step".to_string()],
                transfer_type: "SelectVarOrdered".to_string(),
                selection: vec!["*:position:float:x".to_string()],
            },
            None,
        )
    }
}
