use crate::msg::{msg_bytes_from_payload, Message, Payload};
use crate::sig::Signal;
use crate::{Error, Result};
use serde::Serialize;
use std::convert::TryFrom;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::time::Duration;

#[cfg(feature = "laminar_transport")]
pub mod laminar;
#[cfg(feature = "messageio_transport")]
pub mod messageio;
#[cfg(feature = "zmq_transport")]
pub mod zmq;

mod tcp;

#[derive(Copy, Clone)]
pub struct SocketConfig {
    /// Defines the possible behavior of the socket
    pub type_: SocketType,
    /// Currently used encoding scheme
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
            //idle_timeout: Some(Duration::from_secs(5)),
            idle_timeout: Some(Duration::from_secs(3)),
            heartbeat_interval: Some(Duration::from_secs(1)),
        }
    }
}

// pub struct SocketAddr {
//     protocol: Option<Protocol>,
//
// }
// pub enum Protocol {
//     Tcp,
//     Udp,
//     Inproc,
//     Ipc,
//     // Websocket
// }
//

/// Main socket abstraction.
pub struct Socket {
    inner: InnerSocket,
}

/// Wrapper over different socket types by transport.
pub enum InnerSocket {
    SimpleTcp(tcp::TcpSocket),
    #[cfg(feature = "laminar_transport")]
    Laminar(laminar::LaminarSocket),
    #[cfg(feature = "zmq_transport")]
    Zmq(zmq::ZmqSocket),
    //#[cfg(feature = "messageio_socket")]
    //Messageio(messageio::MessageioSocket),
}

impl Socket {
    pub fn transport(&self) -> Transport {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => Transport::Tcp,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => Transport::Laminar,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => Transport::Zmq,
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

    /// Creates and binds new socket to the given address using specific
    /// transport type.
    pub fn bind(addr: &str, transport: Transport) -> Result<Self> {
        let config = SocketConfig::default();
        Self::bind_with_config(addr, transport, config)
    }

    /// Like `bind` but uses localhost with a random port.
    pub fn bind_any(transport: Transport) -> Result<Self> {
        let config = SocketConfig::default();
        Self::bind_any_with_config(transport, config)
    }

    /// Creates and binds new socket to the given address using specific
    /// transport type, and sets it up based on provided config struct.
    pub fn bind_with_config(
        addr: &str,
        transport: Transport,
        config: SocketConfig,
    ) -> Result<Self> {
        let inner = match transport {
            Transport::Tcp => {
                InnerSocket::SimpleTcp(tcp::TcpSocket::bind_with_config(addr, config)?)
            }
            #[cfg(feature = "laminar_transport")]
            Transport::Laminar => {
                InnerSocket::Laminar(laminar::LaminarSocket::bind_with_config(addr, config)?)
            }
            #[cfg(feature = "zmq_transport")]
            Transport::Zmq => InnerSocket::Zmq(zmq::ZmqSocket::bind_with_config(addr, config)?),
            //#[cfg(feature = "messageio_socket")]
            //Transport::Messageio => InnerSocket::Messageio(messageio::MessageioSocket::bind(addr)?),
            _ => unimplemented!(),
        };
        Ok(Self { inner })
    }

    /// Like `bind_with_config` but uses localhost with a random port.
    pub fn bind_any_with_config(transport: Transport, config: SocketConfig) -> Result<Self> {
        Self::bind_with_config("0.0.0.0:0", transport, config)
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

    /// Returns the last address this socket was bound to.
    pub fn last_endpoint(&self) -> Result<SocketAddr> {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.last_endpoint(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.last_endpoint(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.last_endpoint(),
            _ => unimplemented!(),
        }
    }

    /// Connects to a compatible socket at the provided address.
    ///
    /// # Multiple connections
    ///
    /// Some socket types allow for establishing multiple connections, while
    /// others don't.
    ///
    /// For single-connection socket types, calling this function twice will
    /// return an error.
    pub fn connect(&mut self, addr: &str) -> Result<()> {
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

    /// Terminates an already established connection.
    ///
    /// # Multiple connections
    ///
    /// For socket types where multiple connections from a single socket
    /// are supported, it's required to provide the address of the connection
    /// to be terminated.
    pub fn disconnect(&mut self, addr: Option<SocketAddr>) -> Result<()> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.disconnect(None)?,
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
    pub fn recv(&mut self) -> Result<(SocketAddr, SocketEvent)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(socket) => socket.recv(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.recv(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.recv(),
            //#[cfg(feature = "messageio_socket")]
            //InnerSocket::Messageio(socket) => socket.recv(),
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
    pub fn recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
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
    pub fn recv_sig(&mut self) -> Result<(SocketAddr, Signal)> {
        match &mut self.inner {
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.recv_sig(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.recv_sig(),
            //#[cfg(feature = "messageio_socket")]
            //InnerSocket::Messageio(socket) => socket.recv(),
            InnerSocket::SimpleTcp(sock) => sock.recv_sig(),
            _ => unimplemented!(),
        }
    }

    /// Tries to receive the newest event from the socket without blocking.
    /// If no event is currently available returns an error.
    pub fn try_recv(&mut self) -> Result<(SocketAddr, SocketEvent)> {
        match &mut self.inner {
            InnerSocket::SimpleTcp(ref mut socket) => socket.try_recv(),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.try_recv(),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.try_recv(),
            _ => unimplemented!(),
        }
    }

    /// Tries to receive the newest message from the socket without blocking.
    /// If no message is currently available returns an error.
    pub fn try_recv_msg(&mut self) -> Result<Message> {
        let (addr, msg) = match &mut self.inner {
            InnerSocket::SimpleTcp(ref mut socket) => socket.try_recv_msg()?,
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.try_recv_msg()?,
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.try_recv_msg()?,
            _ => unimplemented!(),
        };
        Ok(msg)
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddr, Signal)> {
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
    pub fn send(&self, bytes: Vec<u8>, addr: Option<SocketAddr>) -> Result<()> {
        match &self.inner {
            InnerSocket::SimpleTcp(socket) => socket.send(bytes, addr),
            #[cfg(feature = "laminar_transport")]
            InnerSocket::Laminar(socket) => socket.send(bytes),
            #[cfg(feature = "zmq_transport")]
            InnerSocket::Zmq(socket) => socket.send(bytes),
            _ => unimplemented!(),
        }
    }

    pub fn send_sig(
        &mut self,
        signal: outcome::distr::Signal,
        addr: Option<SocketAddr>,
    ) -> Result<()> {
        let sig = Signal::from(signal);
        let bytes = sig.to_bytes(self.encoding())?;
        trace!("sending {} byte signal", bytes.len());
        self.send(bytes, addr)
    }

    //pub fn send_msg(&mut self, msg: Message) -> Result<()> {
    //match &mut self.inner {
    //InnerSocket::SimpleTcp(socket) => socket.send_msg()
    ////InnerSocket::Zmq(socket) => socket.send(bytes),
    ////InnerSocket::Laminar(socket) => socket.send(bytes),
    //_ => unimplemented!(),
    //}
    //}
    //
    // fn read_msg(&self) -> Result<Message>;
    // fn try_read_msg(&self, timeout: Option<u32>) -> Result<Message>;
    // fn send_msg(&self, msg: Message) -> Result<()>;

    pub fn pack_send_msg_payload<P: Payload + Serialize>(
        &self,
        payload: P,
        addr: Option<SocketAddr>,
    ) -> Result<()> {
        let msg_bytes = msg_bytes_from_payload(payload, self.encoding())?;
        self.send(msg_bytes, addr)?;
        Ok(())
    }
}

/// Variant event type that is to be sent across the network sockets.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SocketEvent {
    Bytes(Vec<u8>),
    /// Depending on the transport, incoming event might be in a `Message`
    /// form.
    Message(Message),
    Heartbeat,
    Connect,
    Disconnect,
    Timeout,
}

#[derive(Copy, Clone)]
pub enum SocketType {
    Req,
    Rep,
    Pair,
    Stream,
    //Router,
    //Dealer,
}

// TODO websockets
/// List of possible network transports.
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    /// Basic TCP transport built with rust's standard library
    Tcp,
    /// UDP transport with customizable reliability using the laminar library
    #[cfg(feature = "laminar_transport")]
    Laminar,
    /// ZeroMQ based transport, mostly tcp but also supports inproc and ipc
    #[cfg(feature = "zmq_transport")]
    Zmq,
    /// NNG (nanomsg-next-gen) based transport
    #[cfg(feature = "nng_transport")]
    Nng,
    /// TCP/UDP transport using message-io library
    #[cfg(feature = "messageio_transport")]
    Messageio,
}

impl Transport {
    pub fn from_str(s: &str) -> Result<Self> {
        let t = match s {
            "tcp" => Transport::Tcp,
            #[cfg(feature = "zmq_transport")]
            "zmq" => Transport::Zmq,
            #[cfg(feature = "laminar_transport")]
            "laminar" => Transport::Laminar,
            _ => {
                return Err(Error::Other(format!(
                    "failed parsing transport from string: {}",
                    s
                )))
            }
        };
        Ok(t)
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
        let e = match s {
            "bincode" => Self::Bincode,
            #[cfg(feature = "msgpack_encoding")]
            "msgpack" | "messagepack" | "MessagePack" => Self::MsgPack,
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
