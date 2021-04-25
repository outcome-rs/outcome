//! This library provides basic building blocks for `outcome` networking.
//!
//! This includes client-server functionality for easy remote access and
//! control, as well as distributed storage and processing capability using
//! workers and coordinators.
//!
//! Standardized message definitions are provided, and are used extensively by
//! the higher level constructs in their communication protocols. Building upon
//! these message definitions it's possible to introduce [`Server`]s, [`Client`]s
//! and [`Worker`]s, and connect them together to form large distributed
//! deployments for simulation processing.
//!
//!
//! # Networking constructs overview
//!
//! [`outcome_core`] defines two basic types of objects for running distributed
//! simulations: [`SimCentral`] and [`SimNode`], focused specifically on core
//! storage and logic execution capabilities. We not only need to implement
//! them using concrete networking paradigms, but also have to provide
//! additional abstractions for creating and maintaining different types of
//! connections across our network.
//!
//! This library introduces the following four constructs: [`Coord`]
//! (short for `Coordinator`), [`Worker`], [`Server`] and [`Client`].
//!
//! [`Coord`] and [`Worker`] build on top of [`SimCentral`] and [`SimNode`]
//! respectively, implementing their network communications. Together these
//! two make up the low level networking layer.
//!
//! [`Server`] and [`Client`] represent a higher level interface that enables
//! interacting with the simulation, including things like reading and writing
//! data, creating entities and more, simply by passing messages.
//!
//! Message-based communication can be considered language agnostic, with
//! many popular messaging and serialization libraries providing
//! implementations in multiple programming languages. This means client code
//! can be written in anything from C to Python to JavaScript.
//!
//! [`Server`] construct can be created on top of three different types of
//! simulation representations, defining the way it interacts with simulation
//! data:
//! - local form encapsulating a [`Sim`] struct
//! - distributed form using a [`Coord`]
//! - distributed form using a [`Worker`]
//!
//! [`Server`]s can understand, process and respond to messages
//! coming from [`Client`]s. [`Client`] construct is used by both *services*
//! (which are defined as external programs doing work on simulation data) and
//! *human users* connecting to query and/or mutate simulation state.
//!
//! [`Client`]s can use the simulation-wide clock and the *blocking* mechanism
//! for synchronization purposes. Coordinator keeps track of all *blocking*
//! [`Client`]s and will not allow performing next simulation *step* until all
//! of them report their readiness to do so.
//!
//!
//! # I *gotta go fast*, can I make a custom worker?
//!
//! If you know some Rust you can definitely go about implementing your own
//! [`Worker`]. This way you could skip some of the *IPC* overhead and gain
//! direct access to the entities stored on the node attached to that worker.
//!
//!
//! # Using different transports and encodings
//!
//! By default, this crate includes a basic TCP transport along with Bincode
//! encoding. Additional transports and encodings are available with the use
//! of appropriate crate features.
//!
//! [`Server`]s can support multiple transports and encodings at once, allowing
//! connections from widely different [`Client`]s.
//!
//!
//! [`SimCentral`]: outcome_core::distr::SimCentral
//! [`SimNode`]: outcome_core::distr::SimNode
//! [`Sim`]: outcome_core::Sim

#![allow(unused)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate log;

extern crate outcome_core as outcome;

pub use error::{Error, Result};

pub use socket::Encoding;
pub use socket::Transport;
pub use socket::{SocketEvent, SocketEventType};

pub use client::{Client, ClientConfig, CompressionPolicy};
pub use server::{Server, ServerConfig, SimConnection};

pub use coord::Coord;
pub use relay::Relay;
pub use worker::Worker;

pub mod msg;

mod sig;

mod client;
mod coord;
mod error;
mod relay;
mod server;
mod service;
mod socket;
mod util;
mod worker;

pub(crate) type TaskId = u32;
