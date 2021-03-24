//! This library provides basic building blocks for `outcome` networking.
//!
//! This includes client-server functionality for easy remote access and
//! control, as well as actual distributed simulation capability using workers
//! and coordinators.
//!
//! Perhaps most crucially, this library also provides standardized message
//! definitions which are used extensively by the higher level constructs in
//! their communication protocols. Building upon these message definitions it's
//! possible to introduce custom implementations of things like `Workers` and
//! `Clients` and connect them together to form larger deployments.
//!
//!
//! # Networking constructs overview
//!
//! `outcome-core` defines two basic types of objects for running distributed
//! simulations: `Central` and `Node`, focused specifically on core storage and
//! logic execution capabilities. We not only need to implement them using
//! concrete networking paradigms, but also have to provide additional
//! abstractions for creating and maintaining different types of connections
//! across our network.
//!
//! This library introduces the following four constructs: `Coordinator`
//! (shortened to `Coord`), `Worker`, `Server` and `Client`.
//!
//! `Coord` and `Worker` build on top of `Central` and `Node` respectively,
//! implementing their network communications. Together these two make up the
//! low level networking layer.
//!
//! `Server` and `Client` represent a higher level interface that enables
//! interacting with the simulation, including things like reading and writing
//! data, creating entities and more, simply by passing messages.
//!
//! Message-based communication can be considered language agnostic, with
//! many popular messaging and serialization libraries providing
//! implementations in multiple programming languages. This means client code
//! can be written in anything from C to Python to JavaScript.
//!
//! `Server` construct can exists in one of three forms: `SimServer` for
//! single-machine deployments, and `CoordServer` or `WorkerServer` for
//! distributed ones. `Server`s can understand, process and respond to messages
//! coming from `Client`s. `Client` construct is used by both *services*
//! (which are just programs doing work on data) and *human users* connecting
//! to query and/or mutate simulation state. `Client`s can use the
//! simulation-wide clock and the *blocking* mechanism for synchronization
//! purposes. Coordinator keeps track of all *blocking* `Client`s and will not
//! allow performing next simulation *step* until all of them report their
//! readiness to do so.
//!
//!
//! # I *gotta go fast*, can I make a custom worker?
//!
//! If you know some Rust you can definitely go about implementing your own
//! `Worker`. This way you could skip some of the *IPC* overhead and gain
//! direct access to the entities stored on the node attached to that worker.
//!
//!
//! # Using different transports ("drivers")
//!
//! Due to the way `cargo` handles crate features, and due to the need for
//! multiple different network transport variants, this crate doesn't include
//! any particular solution by default. To successfuly use `outcome-net` you
//! will need to specify which *driver* you want to use.
//!
//! So for example, to use `nng` as the message transport layer:
//!
//! ```toml
//! outcome-net = { version = "*", features = ["transport_nng"] }
//! ```
//!
//! # Discussion
//!
//! This crate may not be very useful if one wanted to implement fundamentally
//! different networking functionality to what is provided here with the basic
//! constructs.

#![allow(unused)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate log;

extern crate outcome_core as outcome;

pub mod msg;

mod sig;
mod transport;

mod client;
mod coord;
mod error;
mod server;
mod socket;
mod util;
mod worker;

#[cfg(feature = "transport_nng")]
pub(crate) use transport::nng::*;
#[cfg(feature = "transport_zmq")]
pub(crate) use transport::zmq::*;

pub use client::{Client, ClientConfig, CompressionPolicy};
pub use coord::Coord;
pub use server::{Server, ServerConfig, SimConnection};
pub use worker::Worker;

pub use socket::Encoding;
pub use socket::SocketEvent;
pub use socket::Transport;

pub use error::{Error, Result};
