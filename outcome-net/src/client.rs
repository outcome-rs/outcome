use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::msg::{
    DataTransferRequest, DataTransferResponse, Message, PingRequest, RegisterClientRequest,
    RegisterClientResponse, ScheduledDataTransferRequest, StatusRequest, StatusResponse,
    TransferResponseData, TurnAdvanceRequest, TypedSimDataPack,
};
use crate::socket::{
    CompositeSocketAddress, Encoding, Socket, SocketAddress, SocketConfig, SocketType, Transport,
};
use crate::{error::Error, Result};

/// List of available compression policies for outgoing messages.
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

/// Configuration settings for client.
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
}

impl Client {
    pub fn new() -> Result<Self> {
        Self::new_with_config(ClientConfig::default())
    }

    pub fn new_with_config(config: ClientConfig) -> Result<Self> {
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
        let connection = Socket::new_with_config(None, transport, socket_config)?;
        let client = Self {
            config,
            connection,
            connected: false,
        };
        Ok(client)
    }

    /// Connects to server at the given address.
    ///
    /// # Redirection
    ///
    /// In it's response to client registration message, the server specifies
    /// a new address at which it started a listener socket. New connection
    /// to that address is then initiated by the client.
    pub fn connect(&mut self, greeter_addr: &str, password: Option<String>) -> Result<()> {
        info!("dialing server greeter at: {}", greeter_addr);

        let greeter_composite: CompositeSocketAddress = greeter_addr.parse()?;

        let mut socket_config = SocketConfig {
            type_: SocketType::Pair,
            ..Default::default()
        };
        if let Some(_encoding) = greeter_composite.encoding {
            socket_config.encoding = _encoding;
        }
        let transport = greeter_composite.transport.unwrap_or(Transport::Tcp);
        self.connection = Socket::new_with_config(None, transport, socket_config)?;
        self.connection.connect(greeter_composite.address.clone())?;
        self.connection.send_payload(
            RegisterClientRequest {
                name: self.config.name.clone(),
                is_blocking: self.config.is_blocking,
                auth_pair: None,
                encodings: self.config.encodings.clone(),
                transports: self.config.transports.clone(),
            },
            None,
        )?;
        debug!("sent client registration request");

        let resp: RegisterClientResponse = self
            .connection
            .recv_msg()?
            .1
            .unpack_payload(self.connection.encoding())?;
        debug!("got response from server: {:?}", resp);

        // perform redirection using address provided by the server
        if !resp.address.is_empty() {
            self.connection.disconnect(None)?;
            // std::thread::sleep(Duration::from_millis(100));
            // self.connection.disconnect(Some(address))?;
            let composite = CompositeSocketAddress {
                encoding: Some(resp.encoding),
                transport: Some(resp.transport),
                address: resp.address.parse()?,
            };
            if let Some(_encoding) = composite.encoding {
                socket_config.encoding = _encoding;
            }
            if let Some(_transport) = composite.transport {
                self.connection = Socket::new_with_config(None, _transport, socket_config)?;
            }
            self.connection.connect(composite.address)?;
        }

        // if !resp.error.is_empty() {
        //     return Err(Error::Other(resp.error));
        // }

        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.connection.disconnect(None)
    }

    pub fn server_status(&mut self) -> Result<StatusResponse> {
        self.connection.send_payload(
            StatusRequest {
                format: "".to_string(),
            },
            None,
        )?;
        debug!("sent server status request to server");
        let (_, msg) = self.connection.recv_msg()?;
        let resp: StatusResponse = msg.unpack_payload(self.connection.encoding())?;
        Ok(resp)
    }

    pub fn server_step_request(&mut self, steps: u32) -> Result<Message> {
        self.connection.send_payload(
            TurnAdvanceRequest {
                step_count: steps,
                wait: false,
            },
            None,
        )?;
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

    pub fn get_vars(&mut self) -> Result<TransferResponseData> {
        self.connection.send_payload(
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
        self.connection.send_payload(
            ScheduledDataTransferRequest {
                event_triggers: vec!["step".to_string()],
                transfer_type: "SelectVarOrdered".to_string(),
                selection: vec!["*:position:float:x".to_string()],
            },
            None,
        )
    }
}
