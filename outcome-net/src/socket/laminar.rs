use std::collections::VecDeque;
use std::net::SocketAddr;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Error, Result};

use super::SocketEvent;
use crate::msg::Message;
use crate::sig::Signal;
use crate::socket::{pack, unpack, Encoding, SocketAddress, SocketConfig, SocketEventType};

use crossbeam_channel::{Receiver, Sender};
use std::convert::TryInto;
use std::str::FromStr;

/// Opinionated wrapper over a laminar socket.
pub struct LaminarSocket {
    pub config: SocketConfig,
    listener_addr: Option<SocketAddress>,
    connections: Vec<SocketAddress>,
    /// Default reliability for packets
    reliability: ReliabilityType,
    //TODO replace handle with an atomically counted boolean for breaking poll?
    /// Handle for socket's polling thread
    poll_handle: Option<JoinHandle<()>>,
    /// Receiver end of a channel for incoming events
    receiver: Receiver<laminar::SocketEvent>,
    /// Sender end of a channel for outgoing packets
    sender: Sender<laminar::Packet>,
    /// Queue of events that were received but not yet read.
    /// When functions expecting certain event types like read_msg() are
    /// called, events that are not of required type will end up here.
    event_backlog: VecDeque<(SocketAddress, SocketEvent)>,
}

pub enum ReliabilityType {
    ReliableSequenced,
    UnreliableSequenced,
    UnreliableOrdered,
}

impl Default for ReliabilityType {
    fn default() -> Self {
        ReliabilityType::ReliableSequenced
    }
}

impl LaminarSocket {
    pub fn new(addr: Option<SocketAddress>) -> Result<Self> {
        let config = SocketConfig {
            encoding: Encoding::Bincode,
            ..Default::default()
        };
        Self::new_with_config(addr, config)
    }

    pub fn new_with_config(addr: Option<SocketAddress>, config: SocketConfig) -> Result<Self> {
        let lam_config = laminar::Config {
            idle_connection_timeout: config.idle_timeout.unwrap_or(Duration::from_secs(5)),
            heartbeat_interval: config.heartbeat_interval,
            socket_polling_timeout: Some(Duration::from_secs(2)),
            ..Default::default()
        };
        // if let SocketAddress::Net(socket_address) = addr {
        // } else {
        //     unimplemented!()
        // }
        let mut socket = laminar::Socket::bind_with_config::<SocketAddr>(
            addr.clone()
                .map(|a| a.try_into().unwrap())
                .unwrap_or(SocketAddr::from_str("0.0.0.0:0")?),
            lam_config,
        )?;

        let receiver = socket.get_event_receiver();
        let sender = socket.get_packet_sender();

        let socket_addr = socket.local_addr()?;

        // Starts the socket, which will start a poll mechanism to receive and send messages.
        let handle = std::thread::spawn(move || socket.start_polling());

        // TODO allow setting reliability from addr, e.g. udp_unrel_seq://127.0.0.1:5152
        let reliability = ReliabilityType::ReliableSequenced;

        Ok(Self {
            config,
            listener_addr: Some(SocketAddress::Net(socket_addr)),
            connections: Vec::new(),
            reliability,
            poll_handle: Some(handle),
            receiver,
            sender,
            event_backlog: VecDeque::new(),
        })
    }

    pub fn listen(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn connect(&mut self, addr: SocketAddress) -> Result<()> {
        //self.endpoint_addr = Some(addr.parse().unwrap());
        self.connections.push(addr);
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddress>) -> Result<()> {
        if let Some(address) = addr {
            // self.send_event(SocketEvent::new(SocketEventType::Disconnect), None);
            if let Some(idx) = self.connections.iter().position(|a| a == &address) {
                self.connections.remove(idx);
            }
            // if let Some(idx) = self.connections.iter().position(|a| a == &address) {
            //     self.connections.remove(idx);
            // }
        }
        //if let Some(handle) = self.poll_handle.take() {
        //handle.join().expect("failed to join thread");
        //}

        Ok(())
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn listener_addr(&self) -> Result<SocketAddress> {
        self.listener_addr.clone().ok_or(crate::Error::Other(
            "socket not bound to address".to_string(),
        ))
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        // Waits until a socket event occurs
        let laminar_event = self.receiver.recv().unwrap();
        let (sock_addr, event) = self.match_event(laminar_event)?;
        Ok((sock_addr, event))
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            let laminar_event = self.receiver.recv().unwrap();
            if let Ok((addr, socket_event)) = self.match_event(laminar_event) {
                match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            Message::from_bytes(socket_event.bytes, self.encoding())?,
                        ))
                    }
                    _ => {
                        self.event_backlog.push_back(((addr), socket_event));
                        continue;
                    }
                }
            }
        }
    }

    pub fn recv_sig(&mut self) -> Result<(SocketAddress, Signal)> {
        loop {
            let laminar_event = self.receiver.recv().unwrap();
            if let Ok((addr, socket_event)) = self.match_event(laminar_event) {
                match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            Signal::from_bytes(&socket_event.bytes, &Encoding::Bincode)?,
                        ))
                    }
                    SocketEventType::Disconnect => return Err(crate::Error::HostUnreachable),
                    _ => {
                        warn!("receiving signal, expected bytes, got: {:?}", socket_event);
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                }
            }
        }
    }

    /// Tries receiving next socket event, returning immediately if there are
    /// none available.
    pub fn try_recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        let laminar_event = match self.receiver.try_recv() {
            Ok(event) => event,
            Err(_) => return Err(crate::Error::WouldBlock),
        };
        self.match_event(laminar_event)
    }

    pub fn try_recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            Message::from_bytes(socket_event.bytes, self.encoding())?,
                        ))
                    }
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddress, Signal)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            Signal::from_bytes(&socket_event.bytes, &Encoding::Bincode)?,
                        ))
                    }
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    fn match_event(
        &mut self,
        laminar_event: laminar::SocketEvent,
    ) -> Result<(SocketAddress, SocketEvent)> {
        let (addr, event) = match laminar_event {
            laminar::SocketEvent::Connect(addr) => {
                println!("pushing new connection");
                self.connections.push(SocketAddress::Net(addr));
                (
                    SocketAddress::Net(addr),
                    SocketEvent::new(SocketEventType::Connect),
                )
            }
            laminar::SocketEvent::Disconnect(addr) => {
                if let Some(idx) = self
                    .connections
                    .iter()
                    .position(|a| TryInto::<SocketAddr>::try_into(a.clone()).unwrap() == addr)
                {
                    self.connections.remove(idx);
                }
                (
                    SocketAddress::Net(addr),
                    SocketEvent::new(SocketEventType::Disconnect),
                )
            }
            laminar::SocketEvent::Packet(packet) => (
                SocketAddress::Net(packet.addr()),
                unpack(packet.payload(), self.encoding())?,
            ),

            laminar::SocketEvent::Timeout(addr) => (
                SocketAddress::Net(addr),
                SocketEvent::new(SocketEventType::Timeout),
            ),
        };
        Ok((addr, event))
    }

    pub fn send_bytes(&self, bytes: Vec<u8>, addr: Option<SocketAddress>) -> Result<()> {
        let bytes_event = SocketEvent::new_bytes(bytes);
        self.send_event(bytes_event, addr)
    }

    pub fn send_event(&self, event: SocketEvent, addr: Option<SocketAddress>) -> Result<()> {
        match event.type_ {
            SocketEventType::Connect | SocketEventType::Disconnect | SocketEventType::Timeout => {
                return Err(Error::Other(format!("not supported")));
            }
            _ => (),
        }
        let sock_addr = match addr {
            Some(a) => a,
            None => match self.connections.last() {
                Some(c) => c.clone().try_into().unwrap(),
                None => return Err(crate::Error::SocketNotConnected),
            },
        };
        let sock_addr: SocketAddr = sock_addr.try_into()?;
        let bytes = pack(event, self.encoding())?;
        let packet =
            //laminar::Packet::unreliable_sequenced(self.endpoint_addr.unwrap(), bytes, Some(1));
            //laminar::Packet::reliable_sequenced(self.connections[0], bytes, None);
            laminar::Packet::reliable_ordered(sock_addr, bytes, None);
        self.sender.send(packet).unwrap();
        Ok(())
    }
}
