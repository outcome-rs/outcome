//! Protocol used by union coordinator and workers.
//!
//! # Overview
//!
//! On union initialization, coordinator connects to listed worker
//! addresses and sends introductory messages. Each worker creates
//! a list of all the other workers in the union. This way all
//! the workers can exchange information with each other without
//! the need for centralized broker. Each worker keeps a map of entities
//! and their current node location.
//!
//! Simulation initialization is signalled to workers by the coordinator.
//! Necessary data (sim model) is sent over the network to each of the
//! workers.
//!
//! ## Non-machine processing
//!
//! Processing a step requires handling incoming client chatter, which is
//! mostly event invokes and step process requests (client blocking mechanism).
//!
//! ## Machine processing
//!
//! Runtime-level machine step processing consists of two phases: local and
//! external.
//!
//! Local phase is performed in isolation by each of the workers.
//! During this phase any external commands that were invoked are collected
//! and stored.
//!
//! During the external phase each worker sends messages to other workers based
//! on what has been collected in the previous phase. Messages are sent to
//! proper peer nodes, since each external command is addressed to a specific
//! entity, and worker keeps a map of entities and nodes owning them. It also
//! has a map of nodes with I/O sockets, 2 sockets for each node.

#![allow(unused)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

pub use crate::msg::{Message, Payload};

use crate::msg::MessageType;
use serde::{Deserialize, Serialize};

pub enum SignalType {}

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
pub struct IntroduceWorkerToOrganizerRequest {
    /// By default organizer will use the connection initiated by the worker.
    pub worker_addr: Option<String>,
    pub worker_passwd: String,
}

impl Payload for IntroduceWorkerToOrganizerRequest {
    fn type_(&self) -> MessageType {
        MessageType::IntroduceWorkerToCoordRequest
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntroduceWorkerToCoordResponse {
    pub redirect: String,
    pub error: String,
}

impl Payload for IntroduceWorkerToCoordResponse {
    fn type_(&self) -> MessageType {
        MessageType::IntroduceWorkerToCoordResponse
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntroduceCoordRequest {
    pub ip_addr: String,
    pub passwd: String,
}

impl Payload for IntroduceCoordRequest {
    fn type_(&self) -> MessageType {
        MessageType::IntroduceCoordRequest
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntroduceCoordResponse {
    //    pub clients: Vec<String>,
    pub conn_socket: String,
    pub error: String,
}

impl Payload for IntroduceCoordResponse {
    fn type_(&self) -> MessageType {
        MessageType::IntroduceCoordResponse
    }
}
