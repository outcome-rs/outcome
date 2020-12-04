//! Distributed storage and computation functionality.
//!
//! Definitions are kept generic to allow implementation using different
//! transports and network topographies.

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
use crate::{model, CompId, EntityId, EntityUid, SimModel, StringId, Var, VarType};

#[cfg(feature = "machine")]
use crate::machine::{
    cmd::CentralExtCommand, cmd::Command, cmd::CommandResult, cmd::ExtCommand, ExecutionContext,
};

//TODO
// investigate separate signal structures for communication between two nodes
// and between node and central
/// Definition encompassing all possible messages available for communication
/// between two nodes and between node and central.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signal {
    /// Request node to start initialization using given model and list of entities
    InitializeNode(SimModel),
    // uid, prefab string_id, target string_id
    SpawnEntities(Vec<(EntityUid, Option<EntityId>, Option<EntityId>)>),
    /// Request node to start processing step, includes event_queue vec
    StartProcessStep(Vec<StringId>),

    /// Shutdown imminent
    ShuttingDown,

    /// Node has finished processing step
    ProcessStepFinished,
    /// There are no more request queued
    EndOfRequests,
    /// There are no more responses queued
    EndOfResponses,
    /// There are no more messages queued
    EndOfMessages,

    UpdateModel(SimModel),

    /// Request all data from the node
    DataRequestAll,
    /// Request selected data from the node
    DataRequestSelect(Vec<Address>),
    //TODO investigate responses with typed data packs
    /// Response containing the requested data
    DataResponse(Vec<(Address, Var)>),

    /// Request pulling the provided data
    DataPullRequest(Vec<(Address, Var)>),

    /// External command to be executed on a node
    #[cfg(feature = "machine")]
    ExecuteExtCmd((ExecutionContext, ExtCommand)),
    /// Central-external command to be executed on central
    #[cfg(feature = "machine")]
    ExecuteCentralExtCmd((ExecutionContext, CentralExtCommand)),
}

/// Trait representing central orchestrator's ability to send and receive
/// messages over the wire.
pub trait CentralCommunication {
    /// Reads a single incoming signal.
    fn sig_read(&self) -> Result<(String, Signal)>;
    /// Reads incoming signal from a specific node.
    fn sig_read_from(&self, node_id: u32) -> Result<Signal>;

    /// Sends a signal to node.
    fn sig_send_to_node(&self, node_id: u32, signal: Signal) -> Result<()>;
    /// Sends a signal to node where the specified entity lives.
    fn sig_send_to_entity(&self, entity_uid: EntityUid) -> Result<()>;

    /// Sends a signal to all the nodes.
    fn sig_broadcast(&self, signal: Signal) -> Result<()>;
}

/// Trait representing node's ability to send and receive messages over the
/// network.
pub trait NodeCommunication {
    /// Reads a single signal coming from central orchestrator.
    fn sig_read_central(&mut self) -> Result<Signal>;
    /// Sends a signal to the central orchestrator.
    fn sig_send_central(&mut self, signal: Signal) -> Result<()>;

    /// Reads a single signal coming from another node. Result contains either
    /// a tuple of node id and the received signal, or an error.
    fn sig_read(&mut self) -> Result<(String, Signal)>;
    /// Reads incoming signal from a specific node.
    fn sig_read_from(&mut self, node_id: u32) -> Result<Signal>;

    /// Sends a signal to node.
    fn sig_send_to_node(&mut self, node_id: u32, signal: Signal) -> Result<()>;
    /// Sends a signal to node where the specified entity lives.
    fn sig_send_to_entity(&mut self, entity_uid: EntityUid) -> Result<()>;

    /// Sends a signal to all the nodes.
    fn sig_broadcast(&mut self, signal: Signal) -> Result<()>;

    /// Gets ids of all the connected nodes.
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
