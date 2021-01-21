use std::thread::JoinHandle;

use message_io::events::EventQueue;
use message_io::network::{NetEvent, Network};

use crate::error::{Error, Result};
use crate::msg::Message;
use crate::sig::Signal;
use crate::socket::{SocketConfig, SocketEvent};

pub struct MessageioSocket {
    mode: MessageioSocketMode,
    endpoint_addr: Option<std::net::SocketAddr>,
    network: Network,
    event_queue: EventQueue<Event>,
    config: SocketConfig,
}

pub enum MessageioSocketMode {
    Tcp,
    Udp,
}

impl Default for MessageioSocketMode {
    fn default() -> Self {
        MessageioSocketMode::Tcp
    }
}

enum Event {
    Network(NetEvent<SocketEvent>),
    Bytes(Vec<u8>),
}

impl MessageioSocket {
    pub fn bind(addr: &str) -> Result<Self> {
        Self::bind_with_config(
            addr,
            SocketConfig::default(),
            MessageioSocketMode::default(),
        )
    }

    pub fn bind_with_config(
        addr: &str,
        config: SocketConfig,
        mode: MessageioSocketMode,
    ) -> Result<Self> {
        let mut event_queue = EventQueue::new();

        let sender = event_queue.sender().clone();
        let mut network = Network::new(move |net_event| sender.send(Event::Network(net_event)));

        match mode {
            MessageioSocketMode::Tcp => {
                network.listen_tcp(addr)?;
            }
            MessageioSocketMode::Udp => {
                network.listen_udp(addr)?;
            }
        }

        let socket = Self {
            mode,
            endpoint_addr: None,
            network,
            event_queue,
            config: Default::default(),
        };
    }

    pub fn send(&mut self, bytes: Vec<u8>) -> Result<()> {
        self.event_queue.sender().send(Event::Bytes(bytes));
        Ok(())
    }

    pub fn recv(&mut self) -> Result<SocketEvent> {
        Self::match_event(self.event_queue.receive())
    }

    pub fn recv_sig(&mut self) -> Result<Signal> {
        unimplemented!()
    }

    fn match_event(event: Event) -> Result<SocketEvent> {
        match event {
            Event::Network(net_event) => match net_event {
                NetEvent::Message(endpoint, event) => Ok(event),
                NetEvent::AddedEndpoint(endpoint) => Ok(SocketEvent::Connect(endpoint.addr())),
                NetEvent::RemovedEndpoint(endpoint) => Ok(SocketEvent::Disconnect(endpoint.addr())),
                NetEvent::DeserializationError(endpoint) => {
                    Err(Error::Other("deserialization error".to_string()))
                }
            },
            Event::Bytes(msg) => Ok(SocketEvent::Bytes(msg)),
            _ => unimplemented!(),
        }
    }
}
