use std::collections::VecDeque;
use std::net::SocketAddr;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::Result;

use super::SocketEvent;
use crate::msg::Message;
use crate::sig::Signal;
use crate::socket::{Encoding, SocketConfig};

use crossbeam_channel::{Receiver, Sender};

/// Opinionated wrapper over a laminar socket.
pub struct LaminarSocket {
    config: SocketConfig,
    address: Option<SocketAddr>,
    connections: Vec<SocketAddr>,
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
    event_backlog: VecDeque<(SocketAddr, SocketEvent)>,
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
    pub fn bind(addr: &str) -> Result<Self> {
        let config = SocketConfig {
            encoding: Encoding::Bincode,
            ..Default::default()
        };
        Self::bind_with_config(addr, config)
    }

    pub fn bind_with_config(addr: &str, config: SocketConfig) -> Result<Self> {
        let address = addr.parse().unwrap();
        let mut socket = laminar::Socket::bind(address).unwrap();

        let receiver = socket.get_event_receiver();
        let sender = socket.get_packet_sender();

        // Starts the socket, which will start a poll mechanism to receive and send messages.
        let handle = std::thread::spawn(move || socket.start_polling());

        // TODO allow setting reliability from addr, e.g. udp_unrel_seq://127.0.0.1:5152
        let reliability = ReliabilityType::ReliableSequenced;

        Ok(Self {
            config,
            address: Some(address),
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

    pub fn connect(&mut self, addr: &str) -> Result<()> {
        //self.endpoint_addr = Some(addr.parse().unwrap());
        self.connections.push(addr.parse().unwrap());
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddr>) -> Result<()> {
        if let Some(address) = addr {
            if let Some(idx) = self.connections.iter().position(|a| a == &address) {
                self.connections.remove(idx);
            }
        }
        //if let Some(handle) = self.poll_handle.take() {
        //handle.join().expect("failed to join thread");
        //}

        Ok(())
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn last_endpoint(&self) -> Result<SocketAddr> {
        self.address.ok_or(crate::Error::Other(
            "socket not bound to address".to_string(),
        ))
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&self) -> Result<(SocketAddr, SocketEvent)> {
        // Waits until a socket event occurs
        let laminar_event = self.receiver.recv().unwrap();
        Self::match_event(laminar_event)
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            let laminar_event = self.receiver.recv().unwrap();
            if let Ok((addr, socket_event)) = Self::match_event(laminar_event) {
                match socket_event {
                    SocketEvent::Bytes(bytes) => return Ok((addr, Message::from_bytes(bytes)?)),
                    SocketEvent::Message(msg) => return Ok((addr, msg)),
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                }
            }
        }
    }

    pub fn recv_sig(&mut self) -> Result<(SocketAddr, Signal)> {
        loop {
            let laminar_event = self.receiver.recv().unwrap();
            if let Ok((addr, socket_event)) = Self::match_event(laminar_event) {
                match socket_event {
                    SocketEvent::Bytes(bytes) => {
                        return Ok((addr, Signal::from_bytes(&bytes, &Encoding::Bincode)?))
                    }
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                }
            }
        }
    }

    /// Tries receiving next socket event, returning immediately if there are
    /// none available.
    pub fn try_recv(&self) -> Result<(SocketAddr, SocketEvent)> {
        let laminar_event = match self.receiver.try_recv() {
            Ok(event) => event,
            Err(_) => return Err(crate::Error::WouldBlock),
        };
        Self::match_event(laminar_event)
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddr, Signal)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event {
                    SocketEvent::Bytes(bytes) => {
                        return Ok((addr, Signal::from_bytes(&bytes, &Encoding::Bincode)?))
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

    fn match_event(laminar_event: laminar::SocketEvent) -> Result<(SocketAddr, SocketEvent)> {
        let (addr, event) = match laminar_event {
            laminar::SocketEvent::Connect(addr) => (addr, SocketEvent::Connect),
            laminar::SocketEvent::Disconnect(addr) => (addr, SocketEvent::Disconnect),
            laminar::SocketEvent::Packet(packet) => {
                (packet.addr(), SocketEvent::Bytes(packet.payload().to_vec()))
            }
            laminar::SocketEvent::Timeout(addr) => (addr, SocketEvent::Timeout),
        };
        Ok((addr, event))
    }

    pub fn send(&mut self, bytes: Vec<u8>) -> Result<()> {
        debug!(
            "laminar send bytes count: {}, sending to port: {}",
            bytes.len(),
            self.connections[0].port()
        );
        let packet =
            //laminar::Packet::unreliable_sequenced(self.endpoint_addr.unwrap(), bytes, Some(1));
            //laminar::Packet::reliable_sequenced(self.connections[0], bytes, None);
            laminar::Packet::reliable_ordered(self.connections[0], bytes, None);
        self.sender.send(packet).unwrap();
        Ok(())
    }
}
