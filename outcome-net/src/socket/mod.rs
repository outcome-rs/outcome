use crate::msg::{msg_bytes_from_payload, Message, Payload};
use crate::sig::Signal;
use crate::{sig, Error, Result, TaskId};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Display, Formatter};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

#[cfg(feature = "laminar_transport")]
pub mod laminar;
#[cfg(feature = "zmq_transport")]
pub mod zmq;

mod tcp;

#[derive(Copy, Clone)]
pub struct SocketConfig {
    /// Defines the possible behavior of the socket
    pub type_: SocketType,
    /// Encoding scheme used by the socket
    pub encoding: Encoding,
    pub try_timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub heartbeat_interval: Option<Duration>,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            type_: SocketType::Pair,
            encoding: Encoding::Bincode,
            try_timeout: None,
            idle_timeout: Some(Duration::from_secs(3)),
            heartbeat_interval: Some(Duration::from_secs(1)),
        }
    }
}

/// Main socket abstraction.
pub struct Socket {
    inner: InnerSocket,
    last_heartbeat: Instant,
}

/// Wrapper over different socket types by transport.
pub enum InnerSocket {
    SimpleTcp(tcp::TcpSocket),
    #[cfg(feature = "laminar_transport")]
    Laminar(laminar::LaminarSocket),
    #[cfg(feature = "zmq_transport")]
    Zmq(zmq::ZmqSocket),
}

impl Socket {
    pub fn transport(&self) -> Transport {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => Transport::Tcp,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => Transport::Laminar,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => match socket.transport {
                zmq::ZmqTransport::Tcp => Transport::ZmqTcp,
                zmq::ZmqTransport::Ipc => Transport::ZmqIpc,
            },
            _ => unimplemented!(),
        }
    }

    pub fn config(&self) -> SocketConfig {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.config,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.config,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.config,
            _ => unimplemented!(),
        }
    }

    /// Creates new socket based on provided transport, optionally binding
    /// a listener to the given address.
    pub fn new(addr: Option<SocketAddress>, transport: Transport) -> Result<Self> {
        Self::new_with_config(addr, transport, SocketConfig::default())
    }

    /// Creates new socket based on provided transport and config, optionally
    /// binding a listener to the given address.
    pub fn new_with_config(
        addr: Option<SocketAddress>,
        transport: Transport,
        config: SocketConfig,
    ) -> Result<Self> {
        let inner = match transport {
            Transport::Tcp => {
                InnerSocket::SimpleTcp(tcp::TcpSocket::new_with_config(addr, config)?)
            }
            #[cfg(feature = "laminar_transport")]
            Transport::Laminar => {
                InnerSocket::Laminar(laminar::LaminarSocket::new_with_config(addr, config)?)
            }
            #[cfg(feature = "zmq_transport")]
            Transport::ZmqTcp => InnerSocket::Zmq(zmq::ZmqSocket::new_with_config(
                addr,
                zmq::ZmqTransport::Tcp,
                config,
            )?),
            #[cfg(feature = "zmq_transport")]
            Transport::ZmqIpc => InnerSocket::Zmq(zmq::ZmqSocket::new_with_config(
                addr,
                zmq::ZmqTransport::Ipc,
                config,
            )?),
            _ => unimplemented!(),
        };
        Ok(Self {
            inner,
            last_heartbeat: Instant::now(),
        })
    }

    pub fn encoding(&self) -> &Encoding {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.encoding(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.encoding(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.encoding(),
            _ => unimplemented!(),
        }
    }

    pub fn listener_addr(&self) -> Result<SocketAddress> {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.listener_addr(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.listener_addr(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.listener_addr(),
            _ => unimplemented!(),
        }
    }

    pub fn manual_poll(&mut self) -> Result<()> {
        // send heartbeats
        if let Some(heartbeat) = self.config().heartbeat_interval {
            let now = Instant::now();
            let since_last_heartbeat = now - self.last_heartbeat;
            if since_last_heartbeat > heartbeat {
                self.last_heartbeat = now;
                let heartbeat = SocketEvent::new(SocketEventType::Heartbeat);
                self.send_event(heartbeat, None)?;
            }
        }
        Ok(())
    }

    /// Connects to a compatible socket at the provided address.
    ///
    /// # Multiple connections
    ///
    /// Some socket types allow for establishing multiple connections, while
    /// others don't.
    pub fn connect(&mut self, addr: SocketAddress) -> Result<()> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.connect(addr)?,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.connect(addr)?,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.connect(addr)?,
            _ => unimplemented!(),
        }
        Ok(())
    }

    pub fn bind(&mut self, addr: SocketAddress) -> Result<()> {
        match &mut self.inner {
            // InnerSocket::SimpleTcp(socket) => socket.bind(addr)?,
            // #[cfg(feature = "laminar_transport")]
            // InnerSocket::Laminar(socket) => socket.bind(addr)?,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.bind(addr)?,
            _ => unimplemented!(),
        }
        Ok(())
    }

    /// Terminates an already established connection.
    ///
    /// # Multiple connections
    ///
    /// For socket types where multiple connections from a single socket
    /// are supported, it's required to provide the address of the connection
    /// to be terminated.
    pub fn disconnect(&mut self, addr: Option<SocketAddress>) -> Result<()> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.disconnect(addr)?,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.disconnect(addr)?,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.disconnect(addr)?,
            _ => unimplemented!(),
        }
        Ok(())
    }

    /// Receives the newest socket event from the socket, blocking until
    /// something is received.
    ///
    /// # Multiple connections
    ///
    /// Return type is a tuple that includes the address of the socket where
    /// the received event came from.
    pub fn recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.recv(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.recv(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.recv(),
            _ => unimplemented!(),
        }
    }

    /// Receives the newest message from the socket, blocking until a message
    /// socket event is received.
    ///
    /// # Event backlog
    ///
    /// Any non-message socket events received during the course of this
    /// function will be placed in an internal event backlog. Events pushed
    /// to the backlog can still be read using the regular socket event
    /// receiving functions.
    pub fn recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(ref mut socket) => socket.recv_msg(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.recv_msg(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.recv_msg(),
            _ => unimplemented!(),
        }
    }

    /// Receives the newest signal from the socket, blocking until a signal
    /// socket event is received.
    ///
    /// # Event backlog
    ///
    /// Any non-signal socket events received during the course of this
    /// function will be placed in an internal event backlog. Events pushed
    /// to the backlog can still be read using the regular socket event
    /// receiving functions.
    pub fn recv_sig(&mut self) -> Result<(SocketAddress, Signal)> {
        match &mut self.inner {
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.recv_sig(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.recv_sig(),
            InnerSocket::SimpleTcp(sock) => sock.recv_sig(),
            _ => unimplemented!(),
        }
    }

    /// Tries to receive the newest event from the socket without blocking.
    /// If no event is currently available returns an error.
    pub fn try_recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(ref mut socket) => socket.try_recv(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.try_recv(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.try_recv(),
        }
    }

    /// Tries to receive the newest message from the socket without blocking.
    /// If no message is currently available returns an error.
    pub fn try_recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(ref mut socket) => socket.try_recv_msg(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.try_recv_msg(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.try_recv_msg(),
        }
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddress, Signal)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.try_recv_sig(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.try_recv_sig(),
            _ => unimplemented!(),
        }
    }

    /// Sends data over to a connected socket.
    ///
    /// # Multiple connections
    ///
    /// For socket types supporting multiple connections, the address of the
    /// target socket must be specified.
    pub fn send_bytes(&self, bytes: Vec<u8>, addr: Option<SocketAddress>) -> Result<()> {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.send_bytes(bytes, addr),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.send_bytes(bytes, addr),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.send_bytes(bytes, addr),
        }
    }

    pub fn send_event(&self, event: SocketEvent, addr: Option<SocketAddress>) -> Result<()> {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.send_event(event, addr),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.send_event(event, addr),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.send_event(event, addr),
        }
    }

    pub fn send_sig(&mut self, sig: sig::Signal, addr: Option<SocketAddress>) -> Result<()> {
        let bytes = sig.to_bytes(self.encoding())?;
        trace!("sending {} byte signal", bytes.len());
        self.send_bytes(bytes, addr)
    }

    pub fn send_payload<P: Payload + Serialize>(
        &self,
        payload: P,
        addr: Option<SocketAddress>,
    ) -> Result<()> {
        let msg_bytes = msg_bytes_from_payload(payload, 0, self.encoding())?;
        self.send_bytes(msg_bytes, addr)?;
        Ok(())
    }

    pub fn send_payload_with_task<P: Payload + Serialize>(
        &self,
        payload: P,
        task_id: TaskId,
        addr: Option<SocketAddress>,
    ) -> Result<()> {
        let msg_bytes = msg_bytes_from_payload(payload, task_id, self.encoding())?;
        self.send_bytes(msg_bytes, addr)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SocketEvent {
    pub type_: SocketEventType,
    pub bytes: Vec<u8>,
}

impl SocketEvent {
    pub fn new(type_: SocketEventType) -> Self {
        Self {
            type_,
            bytes: Default::default(),
        }
    }
    pub fn new_bytes(bytes: Vec<u8>) -> Self {
        Self {
            type_: SocketEventType::Bytes,
            bytes,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[repr(u8)]
pub enum SocketEventType {
    Bytes,
    Heartbeat,
    Connect,
    Disconnect,
    Timeout,
}

// /// Variant event type sent across the network sockets.
// #[derive(Debug, Clone, Deserialize, Serialize)]
// pub enum SocketEvent {
//     Bytes(Vec<u8>),
//     Heartbeat,
//     Connect,
//     Disconnect,
//     Timeout,
// }

#[derive(Copy, Clone)]
pub enum SocketType {
    Req,
    Rep,
    Pair,
    Stream,
    //Router,
    //Dealer,
}

// TODO perhaps make file variant use arraystring and then whole thing Copy
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SocketAddress {
    Net(SocketAddr),
    File(String),
    Unavailable,
}

impl SocketAddress {
    pub fn parse_composite(
        s: &str,
    ) -> Result<(Option<Encoding>, Option<Transport>, SocketAddress)> {
        if s.contains("://") {
            let split = s.split("://").collect::<Vec<&str>>();
            if split[0].contains("@") {
                let _split = split[0].split("@").collect::<Vec<&str>>();
                Ok((
                    Some(Encoding::from_str(_split[0])?),
                    Some(Transport::from_str(_split[1])?),
                    split[1].parse()?,
                ))
            } else {
                Ok((
                    None,
                    Some(Transport::from_str(split[0])?),
                    split[1].parse()?,
                ))
            }
        } else if s.contains("@") {
            let split = s.split("@").collect::<Vec<&str>>();
            Ok((Some(Encoding::from_str(split[0])?), None, split[1].parse()?))
        } else {
            Ok((None, None, s.parse()?))
        }
    }
}

impl FromStr for SocketAddress {
    type Err = Error;
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        if s == "unavailable" {
            Ok(Self::Unavailable)
        } else if s.contains("/") {
            Ok(Self::File(s.parse().unwrap()))
        } else {
            Ok(Self::Net(s.parse()?))
        }
    }
}

impl TryInto<SocketAddr> for SocketAddress {
    type Error = Error;
    fn try_into(self) -> core::result::Result<SocketAddr, Self::Error> {
        match self {
            SocketAddress::Net(net) => Ok(net),
            _ => Err(Error::WrongSocketAddressType),
        }
    }
}

// impl ToString for SocketAddress {
//     fn to_string(&self) -> String {
//         match self {
//             Self::Net(s) => s.to_string(),
//             Self::File(s) => s.to_string_lossy().to_string(),
//         }
//     }
// }

impl Display for SocketAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAddress::Net(net) => write!(f, "{}", net.to_string()),
            SocketAddress::File(path) => write!(f, "{}", path),
            SocketAddress::Unavailable => write!(f, "unavailable"),
        }
    }
}

/// List of possible network transports.
// TODO websockets
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    /// Basic TCP transport built with rust's standard library
    Tcp,
    /// UDP transport with customizable reliability using the laminar library
    #[cfg(feature = "laminar_transport")]
    Laminar,
    /// ZeroMQ based TCP transport
    #[cfg(feature = "zmq_transport")]
    ZmqTcp,
    /// ZeroMQ based IPC transport
    #[cfg(feature = "zmq_transport")]
    ZmqIpc,
    /// NNG (nanomsg-next-gen) based transport
    #[cfg(feature = "nng_transport")]
    Nng,
}

impl Transport {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Transport::Tcp),
            "zmq_tcp" | "zmq" | "zeromq" => {
                #[cfg(feature = "zmq_transport")]
                return Ok(Transport::ZmqTcp);
                #[cfg(not(feature = "zmq_transport"))]
                return Err(Error::Other(format!(
                    "trying to use transport: {}, but crate feature zmq_transport is not enabled",
                    s
                )));
            }
            "zmq_ipc" | "ipc" => {
                #[cfg(feature = "zmq_transport")]
                return Ok(Transport::ZmqIpc);
                #[cfg(not(feature = "zmq_transport"))]
                return Err(Error::Other(format!(
                    "trying to use transport: {}, but crate feature zmq_transport is not enabled",
                    s
                )));
            }
            "laminar" | "udp" => {
                #[cfg(feature = "laminar_transport")]
                return Ok(Transport::Laminar);
                #[cfg(not(feature = "laminar_transport"))]
                return Err(Error::Other(format!(
                    "trying to use transport: {}, but crate feature laminar_transport is not enabled",
                    s
                )));
            }
            _ => {
                return Err(Error::Other(format!(
                    "failed parsing transport from string: {}",
                    s
                )))
            }
        }
    }

    /// Checks if laminar transport is available, otherwise falls back on tcp.
    pub fn prefer_laminar() -> Self {
        #[cfg(feature = "laminar_transport")]
        return Self::Laminar;
        #[cfg(not(feature = "laminar_transport"))]
        return Self::Tcp;
    }
}

/// List of possible formats for encoding data sent over the network.
#[derive(Debug, Copy, Clone)]
pub enum Encoding {
    /// Fast binary format, useful for communicating directly between Rust apps
    Bincode,
    /// Binary format with implementations in many different languages
    #[cfg(feature = "msgpack_encoding")]
    MsgPack,
    /// Very common but more verbose format
    #[cfg(feature = "json_encoding")]
    Json,
}

impl Encoding {
    pub fn from_str(s: &str) -> Result<Self> {
        let e = match s.to_lowercase().as_str() {
            "bincode" => Self::Bincode,
            #[cfg(feature = "msgpack_encoding")]
            "msgpack" | "messagepack" | "rmp" => Self::MsgPack,
            #[cfg(feature = "json_encoding")]
            "json" => Self::MsgPack,
            _ => {
                return Err(Error::Other(format!(
                    "failed parsing encoding from string: {}",
                    s
                )))
            }
        };
        Ok(e)
    }
}

/// Packs serializable object to bytes based on selected encoding.
pub(crate) fn pack<S: Serialize>(obj: S, encoding: &Encoding) -> Result<Vec<u8>> {
    let packed: Vec<u8> = match encoding {
        Encoding::Bincode => bincode::serialize(&obj)?,
        #[cfg(feature = "msgpack_encoding")]
        Encoding::MsgPack => {
            use rmp_serde::config::StructMapConfig;
            let mut buf = Vec::new();
            obj.serialize(&mut rmp_serde::Serializer::new(&mut buf))?;
            buf
        }
        #[cfg(feature = "json_encoding")]
        Encoding::Json => unimplemented!(),
    };
    Ok(packed)
}

/// Unpacks object from bytes based on selected encoding.
pub fn unpack<'de, P: Deserialize<'de>>(bytes: &'de [u8], encoding: &Encoding) -> Result<P> {
    let unpacked = match encoding {
        Encoding::Bincode => bincode::deserialize(bytes)?,
        #[cfg(feature = "msgpack_encoding")]
        Encoding::MsgPack => {
            use rmp_serde::config::StructMapConfig;
            // println!("{:?}", bytes);
            let mut de = rmp_serde::Deserializer::new(bytes);

            Deserialize::deserialize(&mut de)?
        }
        #[cfg(feature = "json_encoding")]
        Encoding::Json => unimplemented!(),
    };
    Ok(unpacked)
}

// TODO allow for different compression modes
/// Compress bytes using lz4.
#[cfg(feature = "lz4")]
pub(crate) fn compress(bytes: &Vec<u8>) -> Result<Vec<u8>> {
    let compressed = lz4::block::compress(bytes.as_slice(), None, true)?;
    Ok(compressed)
}
