use std::collections::VecDeque;
use std::net::SocketAddr;
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use zmq::SocketType;

use crate::error::{Error, Result};
use crate::msg::{prefix_with_msg_code, Message, MessageType, Payload};
use crate::sig::Signal;
use crate::socket::{Encoding, SocketConfig, SocketEvent};
use crate::util;
use crate::util::tcp_endpoint;

/// Opinionated wrapper over a ZeroMQ socket.
pub struct ZmqSocket {
    config: SocketConfig,

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
        let socket = context.socket(zmq::SocketType::PAIR).unwrap();
        socket.bind(&util::tcp_endpoint(addr))?;
        Ok(Self {
            config,
            endpoint_addr: None,
            ctx: context,
            inner: socket,
            event_backlog: VecDeque::new(),
        })
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn last_endpoint(&self) -> Result<SocketAddr> {
        let endpoint = self
            .inner
            .get_last_endpoint()
            .unwrap()
            .unwrap()
            .parse()
            .unwrap();
        Ok(endpoint)
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
        let msg = Message::from_bytes(bytes)?;
        Ok((self.endpoint_addr.unwrap(), self.match_event(msg)?))
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event {
                SocketEvent::Bytes(bytes) => return Ok((addr, Message::from_bytes(bytes)?)),
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
            zmq::PollEvents::POLLIN,
            self.config
                .try_timeout
                .unwrap_or(Duration::from_millis(0))
                .as_millis() as i64,
        )?;
        let bytes = match poll {
            0 => return Err(Error::WouldBlock),
            _ => self.inner.recv_bytes(0)?,
        };
        let msg = Message::from_bytes(bytes)?;
        Ok((self.endpoint_addr.unwrap(), self.match_event(msg)?))
    }

    pub fn send(&mut self, bytes: Vec<u8>) -> Result<()> {
        self.inner.send(bytes, 0)?;
        Ok(())
    }

    fn match_event(&self, msg: Message) -> Result<SocketEvent> {
        let event = match msg.type_ {
            MessageType::Heartbeat => SocketEvent::Heartbeat,
            MessageType::Disconnect => SocketEvent::Disconnect,
            MessageType::Connect => SocketEvent::Connect,
            _ => SocketEvent::Message(msg),
        };
        Ok(event)
    }
}
