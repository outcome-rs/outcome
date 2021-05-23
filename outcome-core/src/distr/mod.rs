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
use crate::sim::step;
use crate::{
    model, CompName, EntityId, EntityName, PrefabName, Query, QueryProduct, SimModel, StringId,
    Var, VarType,
};

#[cfg(feature = "machine")]
use crate::machine::{
    cmd::CentralRemoteCommand, cmd::Command, cmd::CommandResult, cmd::ExtCommand, ExecutionContext,
};

pub type NodeId = u32;
pub type TaskId = u32;

/// Definition encompassing all possible messages available for communication
/// between two nodes and between node and central.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signal {
    /// Request node to start initialization using given model and list of entities
    InitializeNode(SimModel),
    /// Request node to spawn a set of entities.
    SpawnEntities(Vec<(EntityId, Option<PrefabName>, Option<EntityName>)>),
    /// Request node to start processing step, includes event_queue vec
    StartProcessStep(Vec<StringId>),

    SnapshotRequest,

    WorkerConnected,

    WorkerStepAdvanceRequest(u32),
    WorkerReady,
    WorkerNotReady,

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

    QueryRequest(Query),
    QueryResponse(QueryProduct),

    /// Request all data from the node
    DataRequestAll,
    /// Request selected data from the node
    DataRequestSelect(Vec<Address>),
    /// Response containing the requested data
    // DataResponse(Vec<(Address, Var)>),
    DataResponse(FnvHashMap<(crate::EntityName, crate::CompName, crate::VarName), Var>),

    /// Request pulling the provided data
    DataPullRequest(Vec<(Address, Var)>),

    /// External command to be executed on a node
    #[cfg(feature = "machine")]
    ExecuteExtCmd((ExecutionContext, ExtCommand)),
    /// Central-external command to be executed on central
    #[cfg(feature = "machine")]
    ExecuteCentralExtCmd((ExecutionContext, CentralRemoteCommand)),
    #[cfg(feature = "machine")]
    ExecuteCentralExtCmds(Vec<(ExecutionContext, CentralRemoteCommand)>),
}

/// Trait representing central coordinator's ability to send and receive
/// data over the network.
pub trait CentralCommunication {
    fn request_task_id(&mut self) -> Result<TaskId>;
    fn return_task_id(&mut self, task_id: TaskId) -> Result<()>;

    /// Gets ids of all the currently connected nodes.
    fn get_node_ids(&self) -> Result<Vec<NodeId>>;

    /// Tries to read a single incoming signal from any node.
    /// On success returns node id, task id and signal.
    fn try_recv_sig(&mut self) -> Result<(NodeId, TaskId, Signal)>;
    /// Reads incoming signal from a specific node.
    /// On success returns task id and signal.
    fn try_recv_sig_from(&mut self, node_id: NodeId) -> Result<(TaskId, Signal)>;

    /// Sends a signal to specified node.
    fn send_sig_to_node(&mut self, node_id: NodeId, task_id: TaskId, signal: Signal) -> Result<()>;
    /// Sends a signal to node where the specified entity is currently stored.
    fn send_sig_to_entity(
        &mut self,
        entity_uid: EntityId,
        task_id: TaskId,
        signal: Signal,
    ) -> Result<()>;

    /// Sends a signal to all nodes.
    fn broadcast_sig(&mut self, task_id: TaskId, signal: Signal) -> Result<()>;
}

/// Trait representing node's ability to send and receive data over the
/// network.
pub trait NodeCommunication {
    fn request_task_id(&mut self) -> Result<TaskId>;
    fn return_task_id(&mut self, task_id: TaskId) -> Result<()>;

    /// Reads a single signal coming from central orchestrator.
    fn sig_read_central(&mut self) -> Result<(TaskId, Signal)>;
    /// Sends a signal to the central orchestrator.
    fn sig_send_central(&mut self, task_id: TaskId, signal: Signal) -> Result<()>;

    /// Reads a single signal coming from another node. Result contains either
    /// a tuple of node id, task id and the received signal, or an error.
    fn sig_read(&mut self) -> Result<(NodeId, TaskId, Signal)>;
    /// Reads incoming signal from a specific node.
    fn sig_read_from(&mut self, node_id: NodeId) -> Result<(TaskId, Signal)>;

    /// Sends a signal to node.
    fn sig_send_to_node(&mut self, node_id: NodeId, task_id: TaskId, signal: Signal) -> Result<()>;
    /// Sends a signal to node where the specified entity lives.
    fn sig_send_to_entity(
        &mut self,
        entity_uid: EntityId,
        task_id: TaskId,
        signal: Signal,
    ) -> Result<()>;

    /// Sends a signal to all the nodes.
    fn sig_broadcast(&mut self, task_id: TaskId, signal: Signal) -> Result<()>;

    /// Gets ids of all the connected nodes.
    fn get_nodes(&mut self) -> Vec<String>;
}

/// Entity distribution policy.
///
/// # Distribution optimization at runtime
///
/// Some policies define a more rigid distribution, while others work by
/// actively monitoring the situation across different nodes and transferring
/// entities around as needed.
#[derive(Serialize, Deserialize)]
pub enum DistributionPolicy {
    /// Set binding to a specific node
    BindToNode(u32),
    /// Set binding to a specific node based on parameters. For example
    BindToNodeWithParams(String),
    /// Naive random distribution using an RNG
    Random,
    /// Optimize for processing speed, using the most capable nodes first
    MaxSpeed,
    /// Optimize for lowest network traffic, grouping together entity pairs
    /// that tend to cause most inter-machine chatter
    LowTraffic,
    /// Balanced approach, sane default policy for most cases
    Balanced,
    /// Focus on similar memory usage across nodes, relative to capability
    SimilarMemoryUsage,
    /// Focus on similar processor usage across nodes, relative to capability
    SimilarProcessorUsage,
    /// Spatial distribution based on entity world coordinates.
    ///
    /// # Details
    ///
    /// Pulls into the model a built-in `position` component containing floats
    /// for x, y and z coordinates.
    ///
    /// Three-dimensional bounding box is defined for each node. Entities are
    /// distributed based on which box they are currently in.
    Spatial,
}
