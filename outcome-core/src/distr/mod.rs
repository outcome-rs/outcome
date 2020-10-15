//! Provides functionality related to distributed storage and computation.
//!
//! Definitions are kept generic to allow implementation using different
//! transports and network topographies.

extern crate image;

pub mod central;
pub mod node;

pub use central::SimCentral;
pub use node::SimNode;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fnv::FnvHashMap;

#[cfg(feature = "machine_dynlib")]
use libloading::Library;
#[cfg(feature = "machine")]
use rayon::prelude::*;
#[cfg(feature = "machine_lua")]
use rlua::Lua;

use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::error::{Error, Result};
use crate::model::{DataEntry, DataImageEntry, Scenario};
use crate::sim::interface::SimInterface;
use crate::sim::step;
use crate::StringId;
use crate::{model, EntityId, SimModel, VarType};
use crate::{CompId, Var};

#[cfg(feature = "machine")]
use crate::machine::{
    cmd::CentralExtCommand, cmd::Command, cmd::CommandResult, cmd::ExtCommand, ExecutionContext,
};

/// Definition encompassing all possible messages available for node<>node
/// and node<>central communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signal {
    InitializeNode((SimModel, Vec<EntityId>)),
    /// Request node to start processing step, includes event_queue vec
    StartProcessStep(Vec<StringId>),
    /// Sent by node to central to signal it's done processing tick
    ProcessStepFinished,
    EndOfRequests,
    EndOfResponses,
    EndOfMessages,
    /// External cmd to be executed on a node
    #[cfg(feature = "machine")]
    ExecuteExtCmd((ExecutionContext, ExtCommand)),
    /// Central external cmd to be executed on central
    #[cfg(feature = "machine")]
    ExecuteCentralExtCmd((ExecutionContext, CentralExtCommand)),
}

/// Trait representing central orchestrator's ability to send and receive
/// messages over the wire.
pub trait CentralCommunication {
    /// Read a single incoming signal
    fn sig_read(&mut self) -> Result<(String, Signal)>;
    /// Read incoming signal from a specific node
    fn sig_read_from(&mut self, node_id: &str) -> Result<Signal>;

    /// Send a signal to node
    fn sig_send_to_node(&mut self, node_id: &str, signal: Signal) -> Result<()>;
    /// Send a signal to node where the specified entity lives
    fn sig_send_to_entity(&mut self, entity_uid: EntityId) -> Result<()>;

    /// Send a signal to all the nodes
    fn sig_broadcast(&mut self, signal: Signal) -> Result<()>;
}

/// Trait representing node's ability to send and receive messages over the
/// network.
pub trait NodeCommunication {
    /// Read a single signal coming from central orchestrator
    fn sig_read_central(&mut self) -> Result<Signal>;
    /// Send a signal to the central orchestrator
    fn sig_send_central(&mut self, signal: Signal) -> Result<()>;

    /// Read a single signal coming from another node. Result contains either
    /// a tuple of node id and the received signal, or an error.
    fn sig_read(&mut self) -> Result<(String, Signal)>;
    /// Read incoming signal from a specific node
    fn sig_read_from(&mut self, node_id: &str) -> Result<Signal>;

    /// Send a signal to node
    fn sig_send_to_node(&mut self, node_id: &str, signal: Signal) -> Result<()>;
    /// Send a signal to node where the specified entity lives
    fn sig_send_to_entity(&mut self, entity_uid: EntityId) -> Result<()>;

    /// Send a signal to all the nodes
    fn sig_broadcast(&mut self, signal: Signal) -> Result<()>;

    /// Get ids of all the connected nodes
    fn get_nodes(&mut self) -> Vec<String>;
}

pub trait DistrError {
    fn would_block(&self) -> bool;
    fn timed_out(&self) -> bool;
}

pub enum EntityAssignMethod {
    Random,
    Complexity,
    VarCount,
    MemorySize,
}
