//! # Internal API
//!
//! Protocol used by cluster coordinator and workers.
//!
//! ## Protocol overview
//!
//! On cluster initialization, coordinator connects to listed worker
//! addresses and sends introductory messages. Each worker creates
//! a list of all the other workers in the cluster. This way all
//! the workers can exchange information with each other without
//! the need for centralized broker. Each worker keeps a map of entities
//! and their current node location.
//!
//! Simulation initialization is signalled to workers by the coordinator.
//! Necessary data (sim model) is sent over the network to each of the
//! workers.
//!
//! Tick processing consists of two phases: `loc`, `ext`.
//! Each phase is signalled to workers by the coordinator.
//!
//! `loc` (local) phase is performed in isolation by each of the workers.
//! During `loc` phase `ext` commands are collected for processing during
//! `ext` phase.
//!
//! During the `ext` phase each worker sends messages to other workers based
//! on the collected `ext` commands. Messages are sent to proper peer nodes,
//! since each `ext` command is addressed to a single entity, and worker
//! keeps a map of entities and nodes owning them. It also has a map of
//! nodes with I/O sockets, 2 sockets for each node.
//!

#![allow(unused)]

extern crate rmp_serde;
extern crate serde;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

pub use crate::msg::{Message, Payload};

use self::rmp_serde::{Deserializer, Serializer};
use self::serde::{Deserialize, Serialize};

// universal
pub const PING_REQUEST: &str = "PingRequest";
pub const PING_RESPONSE: &str = "PingResponse";

// coord<>worker
pub const INTRODUCE_COORD_REQUEST: &str = "RegisterCoordRequest";
pub const INTRODUCE_COORD_RESPONSE: &str = "RegisterCoordResponse";

pub const SIGNAL_REQUEST: &str = "DistrMsgRequest";
pub const SIGNAL_RESPONSE: &str = "DistrMsgResponse";

// worker<>worker
pub const REGISTER_COMRADE_REQUEST: &str = "RegisterComradeRequest";
pub const REGISTER_COMRADE_RESPONSE: &str = "RegisterComradeRequest";

pub const DATA_TRANSFER_REQUEST: &str = "DataTransferRequest";
pub const DATA_TRANSFER_RESPONSE: &str = "DataTransferResponse";
pub const DATA_PULL_REQUEST: &str = "DataPullRequest";
pub const DATA_PULL_RESPONSE: &str = "DataPullResponse";

pub const GET_REQUEST: &str = "GetRequest";
pub const GET_RESPONSE: &str = "GetResponse";
pub const SET_REQUEST: &str = "SetRequest";
pub const SET_RESPONSE: &str = "SetResponse";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntroduceCoordRequest {
    pub ip_addr: String,
    pub passwd: String,
}
impl Payload for IntroduceCoordRequest {
    fn kind_str(&self) -> &str {
        "RegisterCoordRequest"
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntroduceCoordResponse {
    //    pub clients: Vec<String>,
    pub error: String,
}
impl Payload for IntroduceCoordResponse {
    fn kind_str(&self) -> &str {
        "RegisterCoordResponse"
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignalRequest {
    pub signal: outcome_core::distr::Signal,
}
impl Payload for SignalRequest {
    fn kind_str(&self) -> &str {
        SIGNAL_REQUEST
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignalResponse {
    pub distr_msg: outcome_core::distr::Signal,
}
impl Payload for SignalResponse {
    fn kind_str(&self) -> &str {
        SIGNAL_RESPONSE
    }
}
