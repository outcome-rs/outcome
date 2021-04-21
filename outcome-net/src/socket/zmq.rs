use std::collections::VecDeque;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use zmq::SocketType;

use crate::error::{Error, Result};
use crate::msg::{Message, MessageType, Payload};
use crate::sig::Signal;
use crate::socket::{
    pack, unpack, Encoding, Socket, SocketAddress, SocketConfig, SocketEvent, SocketEventType,
};
use crate::{msg, util};

pub enum ZmqTransport {
    Tcp,
    Ipc,
}

/// Wrapper over a ZeroMQ socket.
pub struct ZmqSocket {
    pub config: SocketConfig,
    pub transport: ZmqTransport,
    listener_addr: Option<SocketAddress>,
    ctx: zmq::Context,
    inner: zmq::Socket,

    event_backlog: VecDeque<(SocketAddress, SocketEvent)>,
}

impl ZmqSocket {
    pub fn new(addr: Option<SocketAddress>, transport: ZmqTransport) -> Result<Self> {
        let config = SocketConfig {
            #[cfg(feature = "msgpack_encoding")]
            encoding: Encoding::MsgPack,
            ..Default::default()
        };
        Self::new_with_config(addr, transport, config)
    }

    pub fn new_with_config(
        addr: Option<SocketAddress>,
        transport: ZmqTransport,
        config: SocketConfig,
    ) -> Result<Self> {
        let context = zmq::Context::new();
        let socket_type = match config.type_ {
            super::SocketType::Req => SocketType::REQ,
            super::SocketType::Rep => SocketType::REP,
            super::SocketType::Pair => SocketType::PAIR,
            _ => unimplemented!(),
        };
        println!("socket_type: {:?}", socket_type);
        let socket = context.socket(socket_type)?;
        let mut listener_addr = None;

        if let Some(_addr) = &addr {
            let mut string_addr = prepend_transport(_addr.to_string().as_str(), &transport);
            println!("binding to string_addr: {}", string_addr);
            socket.bind(&string_addr)?;
            listener_addr = Some(socket.get_last_endpoint().unwrap().unwrap().parse()?);
            // TODO tweak timeout values
            // socket.set_rcvtimeo(10)?;
            // socket.set_sndtimeo(10)?;
        }
        Ok(Self {
            config,
            transport,
            listener_addr,
            ctx: context,
            inner: socket,
            event_backlog: VecDeque::new(),
        })
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn listener_addr(&self) -> Result<SocketAddress> {
        self.listener_addr
            .clone()
            .ok_or(Error::Other("socket not bound to address".to_string()))
    }

    pub fn connect(&mut self, addr: SocketAddress) -> Result<()> {
        let conn_addr = prepend_transport(&addr.to_string(), &self.transport);
        println!("conn_addr: {}", conn_addr);
        self.inner.connect(&conn_addr)?;
        // self.inner.set_rcvtimeo(10)?;
        // self.inner.set_sndtimeo(10)?;
        self.send_event(SocketEvent::new(SocketEventType::Connect), None)?;
        // self.listener_addr = Some(addr);
        Ok(())
    }

    pub fn bind(&mut self, addr: SocketAddress) -> Result<()> {
        let bind_addr = prepend_transport(&addr.to_string(), &self.transport);
        println!("bind addr: {}", bind_addr);
        self.inner.bind(&bind_addr)?;
        // self.inner.set_rcvtimeo(10)?;
        // self.inner.set_sndtimeo(10)?;
        // self.send_event(SocketEvent::new(SocketEventType::Connect), None)?;
        // self.listener_addr = Some(addr);
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddress>) -> Result<()> {
        self.inner.set_sndtimeo(100);
        self.send_event(SocketEvent::new(SocketEventType::Disconnect), None)?;

        let disco_addr = match addr {
            Some(a) => prepend_transport(&a.to_string(), &self.transport),
            None => prepend_transport(
                &self.inner.get_last_endpoint().unwrap().unwrap(),
                &self.transport,
            ),
        };

        self.inner.disconnect(&disco_addr)?;
        // std::thread::sleep(Duration::from_millis(100));

        // if it's a listening pair socket, make sure it rebinds after disconnecting
        if let Some(listen_addr) = &self.listener_addr {
            self.inner.bind(&prepend_transport(
                &listen_addr.to_string(),
                &self.transport,
            ))?;
        }
        self.inner.set_sndtimeo(-1);
        Ok(())
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&self) -> Result<(SocketAddress, SocketEvent)> {
        self.inner.set_rcvtimeo(-1)?;
        let bytes = self.inner.recv_bytes(0)?;
        let event: SocketEvent = unpack(&bytes, &self.config.encoding)?;

        let lep = self.inner.get_last_endpoint();
        Ok((
            lep.map_err(|_| Error::SocketNotConnected)?
                .unwrap()
                .parse()?,
            event,
        ))
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event.type_ {
                SocketEventType::Bytes => {
                    return Ok((addr, unpack(&event.bytes, &self.config.encoding)?))
                }
                _ => {
                    self.event_backlog.push_back((addr, event));
                    continue;
                }
            };
        }
    }

    pub fn recv_sig(&self) -> Result<(SocketAddress, Signal)> {
        let (addr, event) = self.recv()?;
        match event.type_ {
            SocketEventType::Bytes => Ok((
                addr,
                Signal::from_bytes(&event.bytes, &self.config.encoding)?,
            )),
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
    pub fn try_recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        let events = self.inner.get_events()?;
        let poll = self.inner.poll(
            events,
            // zmq::PollEvents::POLLIN,
            self.config
                .try_timeout
                .unwrap_or(Duration::from_millis(1))
                .as_millis() as i64,
        )?;
        if !events.contains(zmq::POLLIN) {
            return Err(Error::WouldBlock);
        }
        let bytes = match poll {
            0 => return Err(Error::WouldBlock),
            _ => self.inner.recv_bytes(0)?,
        };
        let event: SocketEvent = unpack(&bytes, &self.config.encoding)?;
        // let msg: Message = unpack(&bytes, &self.config.encoding)?;
        // let event = self.match_event(msg)?;

        trace!("got event: {:?}", event);

        // let lep = self.inner().unwrap().unwrap();
        self.handle_internally(None, &event)?;
        // Ok((lep.as_str().parse()?, event))
        Ok((SocketAddress::Unavailable, event))
        // Ok((self.listener_addr.clone().unwrap(), event))
    }

    pub fn try_recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((addr, unpack(&socket_event.bytes, &self.config.encoding)?))
                    }
                    // SocketEvent::Message(msg) => return Ok((addr, msg)),
                    _ => {
                        println!("pushing back: {:?}", (&addr, &socket_event));
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    pub fn send_bytes(&self, bytes: Vec<u8>, addr: Option<SocketAddress>) -> Result<()> {
        let bytes_event = SocketEvent::new_bytes(bytes);
        self.send_event(bytes_event, addr)
    }

    pub fn send_event(&self, event: SocketEvent, addr: Option<SocketAddress>) -> Result<()> {
        trace!("sending event: {:?}", event);
        let bytes = pack(event, self.encoding())?;
        self.inner.send(bytes, 0)?;
        Ok(())
    }

    fn handle_internally(
        &mut self,
        addr: Option<SocketAddress>,
        event: &SocketEvent,
    ) -> Result<()> {
        match event.type_ {
            // SocketEvent::Connect => self.
            SocketEventType::Disconnect => {
                // self.inner.set_sndtimeo(0);
                let lep = self.inner.get_last_endpoint().unwrap().unwrap();
                // self.inner.disconnect(&lep)?;
                self.inner.bind(&lep)?;
            }
            _ => (),
        }
        Ok(())
    }

    // fn match_event(&self, msg: Message) -> Result<SocketEvent> {
    //     let event = match msg.type_ {
    //         MessageType::Heartbeat => SocketEvent::new(SocketEventType::Heartbeat),
    //         MessageType::Disconnect => SocketEvent::new(SocketEventType::Disconnect),
    //         MessageType::Connect => SocketEvent::new(SocketEventType::Connect),
    //         MessageType::Connect => SocketEvent::new(SocketEventType::Connect),
    //         _ => unimplemented!("{:?}", msg),
    //     };
    //     Ok(event)
    // }
}

/// Create a valid tcp address that includes the prefix.
pub(crate) fn prepend_transport(s: &str, transport: &ZmqTransport) -> String {
    if s.contains("://") {
        s.to_string()
    } else {
        match transport {
            ZmqTransport::Tcp => format!("tcp://{}", s),
            ZmqTransport::Ipc => format!("ipc://{}", s),
        }
    }
}
