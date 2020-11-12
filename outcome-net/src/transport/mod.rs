//! This module is the home of network *drivers*. Multiple variants are
//! available, each using different underlying transport mechanism.
//!
//! A *driver* is defined as a wrapper around more specific transport that
//! provides construct-specific interface (e.g. worker interface).
//!
//! As this library only exports higher-level constructs, *drivers* are an
//! internal feature used only within the confines of this library.

#[cfg(feature = "transport_nng")]
pub(crate) mod nng;
#[cfg(feature = "transport_zmq")]
pub(crate) mod zmq;

use crate::msg::Message;
use crate::server::ClientId;
use crate::worker::WorkerId;
use crate::{error::Error, Result};

pub(crate) trait SocketInterface
where
    Self: Sized,
{
    fn bind(&self, addr: &str) -> Result<()>;
    fn connect(&self, addr: &str) -> Result<()>;
    fn disconnect(&self, addr: &str) -> Result<()>;
    fn read(&self) -> Result<Vec<u8>>;
    fn try_read(&self, timeout: Option<u32>) -> Result<Vec<u8>>;
    fn send(&self, bytes: Vec<u8>) -> Result<()>;

    fn read_msg(&self) -> Result<Message>;
    fn try_read_msg(&self, timeout: Option<u32>) -> Result<Message>;
    fn send_msg(&self, msg: Message) -> Result<()>;
}

pub(crate) trait ServerDriverInterface
where
    Self: Sized,
{
    fn new(addr: &str) -> Result<Self>;
    fn read(&self, client_id: &ClientId) -> Result<Message>;
    fn try_read(&self, client_id: &ClientId) -> Result<Message>;
    fn send(&mut self, client_id: &ClientId, message: Message) -> Result<()>;
    fn broadcast(&mut self, message: Message) -> Result<()>;
    fn accept(&mut self) -> Result<(ClientId, Message)>;
}

pub(crate) trait ClientDriverInterface
where
    Self: Sized,
{
    fn new() -> Result<Self>;
    fn my_addr(&self) -> String;
    fn dial_server(&self, addr: &str, msg: Message) -> Result<()>;
    fn read(&self) -> Result<Message>;
    fn send(&self, message: Message) -> Result<()>;
}

pub(crate) trait CoordDriverInterface
where
    Self: Sized,
{
    fn new(addr: &str) -> Result<Self>;
    fn accept(&mut self) -> Result<(WorkerId, Message)>;
    fn connect_to_worker(&self, addr: &str, msg: Message) -> Result<()>;

    fn msg_send_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<()>;
    fn msg_read_worker(&self, worker_id: &WorkerId, msg: Message) -> Result<()>;
}

pub(crate) trait WorkerDriverInterface
where
    Self: Sized,
{
    fn new(my_addr: &str) -> Result<Self>;
    fn accept(&self) -> Result<Message>;
    fn connect_to_coord(&mut self, coord_addr: &str, msg: Message) -> Result<()>;

    fn msg_read_central(&self) -> Result<Message>;
    fn msg_send_central(&self, msg: Message) -> Result<()>;
    fn msg_read_worker(&self, worker_id: WorkerId) -> Result<Message>;
    fn msg_send_worker(&self, worker_id: WorkerId, msg: Message) -> Result<()>;
}
