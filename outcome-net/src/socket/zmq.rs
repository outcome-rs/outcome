use std::collections::VecDeque;
use std::net::SocketAddr;
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use zmq::SocketType;

use crate::error::{Error, Result};
use crate::msg::{prefix_with_msg_code, Message, MessageType, Payload};
use crate::sig::Signal;
use crate::socket::{Encoding, Socket, SocketConfig, SocketEvent};
use crate::util::tcp_endpoint;
use crate::{msg, util};
use std::convert::TryFrom;

/// Opinionated wrapper over a ZeroMQ socket.
pub struct ZmqSocket {
    pub config: SocketConfig,

    endpoint_addr: Option<std::net::SocketAddr>,
    ctx: zmq::Context,
    inner: zmq::Socket,

    event_backlog: VecDeque<(SocketAddr, SocketEvent)>,
}

impl ZmqSocket {
    pub fn bind(addr: &str) -> Result<Self> {
        let config = SocketConfig {
            #[cfg(feature = "msgpack_encoding")]
            encoding: Encoding::MsgPack,
            ..Default::default()
        };
        Self::bind_with_config(addr, config)
    }

    pub fn bind_with_config(addr: &str, config: SocketConfig) -> Result<Self> {
        let context = zmq::Context::new();
        let socket_type = match config.type_ {
            super::SocketType::Req => SocketType::REQ,
            super::SocketType::Rep => SocketType::REP,
            super::SocketType::Pair => SocketType::PAIR,
            _ => unimplemented!(),
        };
        let socket = context.socket(socket_type).unwrap();
        socket.bind(&util::tcp_endpoint(addr))?;
        // socket.bind(addr)?;
        Ok(Self {
            config,
            endpoint_addr: Some(SocketAddr::from_str(addr).unwrap()),
            ctx: context,
            inner: socket,
            event_backlog: VecDeque::new(),
        })
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn last_endpoint(&self) -> Result<SocketAddr> {
        self.endpoint_addr
            .ok_or(Error::Other("socket not bound to address".to_string()))
    }

    pub fn connect(&mut self, addr: &str) -> Result<()> {
        self.endpoint_addr = Some(SocketAddr::from_str(addr).unwrap());
        self.inner.connect(&tcp_endpoint(addr))?;
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddr>) -> Result<()> {
        self.send(prefix_with_msg_code(Vec::new(), MessageType::Disconnect))?;
        self.inner
            .disconnect(util::tcp_endpoint(&self.endpoint_addr.unwrap().to_string()).as_str())
            .unwrap();
        Ok(())
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&self) -> Result<(SocketAddr, SocketEvent)> {
        // Waits until a socket event occurs
        let bytes = self.inner.recv_bytes(0).unwrap();
        let msg: Message = msg::unpack(&bytes, &self.config.encoding)?;
        let event = self.match_event(msg)?;
        Ok((
            self.endpoint_addr
                .unwrap_or(SocketAddr::from_str("127.0.0.1:5151").unwrap()),
            event,
        ))
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event {
                SocketEvent::Bytes(bytes) => {
                    return Ok((addr, msg::unpack(&bytes, &self.config.encoding)?))
                }
                SocketEvent::Message(msg) => return Ok((addr, msg)),
                _ => {
                    self.event_backlog.push_back((addr, event));
                    continue;
                }
            };
        }
    }

    pub fn recv_sig(&self) -> Result<(SocketAddr, Signal)> {
        let (addr, event) = self.recv()?;
        match event {
            SocketEvent::Bytes(bytes) => {
                Ok((addr, Signal::from_bytes(&bytes, &self.config.encoding)?))
            }
            _ => unimplemented!(),
        }
    }

    /// Waits for the next message, blocking until one is available.
    pub fn recv_raw(&self) -> Result<Vec<u8>> {
        let bytes = self.inner.recv_bytes(0)?;
        Ok(bytes)
    }

    /// Tries receiving next socket event, returning immediately if there are
    /// none available.
    pub fn try_recv(&self) -> Result<(SocketAddr, SocketEvent)> {
        let events = self.inner.get_events().unwrap();
        let poll = self.inner.poll(
            events,
            // zmq::PollEvents::POLLIN,
            self.config
                .try_timeout
                .unwrap_or(Duration::from_millis(0))
                .as_millis() as i64,
        )?;
        if !events.contains(zmq::POLLIN) {
            return Err(Error::WouldBlock);
        }
        let bytes = match poll {
            0 => return Err(Error::WouldBlock),
            _ => self.inner.recv_bytes(0)?,
        };
        // let msg = Message::from_bytes(bytes)?;
        let msg: Message = msg::unpack(&bytes, &self.config.encoding)?;
        let event = self.match_event(msg)?;

        Ok((self.endpoint_addr.unwrap(), event))
    }

    pub fn try_recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event {
                    SocketEvent::Bytes(bytes) => {
                        return Ok((addr, msg::unpack(&bytes, &self.config.encoding)?))
                    }
                    SocketEvent::Message(msg) => return Ok((addr, msg)),
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    pub fn send(&self, bytes: Vec<u8>) -> Result<()> {
        self.inner.send(bytes, 0)?;
        Ok(())
    }

    fn match_event(&self, msg: Message) -> Result<SocketEvent> {
        let event = match MessageType::try_from(msg.type_)? {
            MessageType::Heartbeat => SocketEvent::Heartbeat,
            MessageType::Disconnect => SocketEvent::Disconnect,
            MessageType::Connect => SocketEvent::Connect,
            _ => SocketEvent::Message(msg),
        };
        Ok(event)
    }
}
