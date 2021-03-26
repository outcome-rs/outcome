//! This library implements core engine functionality.
//!
//! Programming interface is centered around the [`Sim`] structure, which
//! encapsulates simulation state. [`Sim`] can be created from path to
//! appropriate data such as scenario or snapshot. Once initialized it can be
//! stepped through and serialized to file. Contents of both entities and the
//! simulation model can be mutated at runtime. [`Sim`]'s equivalent for
//! distributed simulation is [`SimCentral`]. [`SimCentral`] provides a similar
//! API to [`Sim`], while orchestrating multiple nodes "behind the scenes".
//!
//!
//! # Networking
//!
//! By itself, this library does not provide any networking capability. Only
//! the most basic structures and traits for implementing distributed storage
//! and processing are provided. These can be used to implement distributed
//! simulation capability using different configurations, transports and
//! topologies. For an example networking implementation see `outcome-net`.
//!
//! # Using the library
//!
//! To use `outcome-core` in your Rust project add the following to your
//! `Cargo.toml`:
//!
//! ```toml
//! outcome-core = "0.1.0"
//! ```
//!
//! You might also want to select a set of engine features to enable.
//! For example:
//!
//! ```toml
//! outcome-core = { version = "0.1.0", features = ["machine_sandbox"] }
//! ```
//!
//! See crate's `Cargo.toml` for a full listing of available features.
//!
//! ## Example
//!
//! Here's a very simple example of how the library can be used inside your
//! program:
//!
//! ```ignore
//! extern crate outcome_core as outcome;
//! use outcome::Sim;
//! use std::env;
//!
//! pub fn main() {
//!     let path = env::current_dir().unwrap();
//!     let mut sim = Sim::from_scenario_at(path).unwrap();
//!     sim.step();
//! }
//! ```
//!
//! # More information
//!
//! For more information about the project see the
//! [project website](https://theoutcomeproject.com). The
//! [book](https://book.theoutcomeproject.com), aims to provide in-depth
//! explanations for all the topics related to the project. It also includes
//! tutorials and guides on how to use the provided software.
//!
//!
//! [`Sim`]: sim/struct.Sim.html
//! [`SimCentral`]: distr/central/struct.SimCentral.html

#![allow(unused)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate log;

#[cfg(feature = "machine")]
#[macro_use]
extern crate fasteval;

// reexports
pub use address::Address;
pub use error::Result;
pub use model::SimModel;
pub use sim::Sim;
pub use var::{Var, VarType};

pub mod address;
pub mod arraystring;
pub mod distr;
pub mod entity;
pub mod error;
pub mod model;
pub mod query;
pub mod sim;
pub mod var;

mod util;

// features
pub const FEATURE_NAME_SMALL_NUMS: &str = "small_nums";
#[cfg(not(feature = "small_nums"))]
pub const FEATURE_SMALL_NUMS: bool = false;
#[cfg(feature = "small_nums")]
pub const FEATURE_SMALL_NUMS: bool = true;

pub const FEATURE_NAME_SHORT_STRINGID: &str = "short_stringid";
#[cfg(not(feature = "short_stringid"))]
pub const FEATURE_SHORT_STRINGID: bool = false;
#[cfg(feature = "short_stringid")]
pub const FEATURE_SHORT_STRINGID: bool = true;

pub const FEATURE_NAME_MACHINE_SYSINFO: &str = "machine_sysinfo";
#[cfg(not(feature = "machine_sysinfo"))]
pub const FEATURE_MACHINE_SYSINFO: bool = false;
#[cfg(feature = "machine_sysinfo")]
pub const FEATURE_MACHINE_SYSINFO: bool = true;

pub const FEATURE_NAME_MACHINE_SCRIPT: &str = "machine_script";
#[cfg(not(feature = "machine_script"))]
pub const FEATURE_MACHINE_SCRIPT: bool = false;
#[cfg(feature = "machine_script")]
pub const FEATURE_MACHINE_SCRIPT: bool = true;

pub const FEATURE_NAME_MACHINE: &str = "machine";
#[cfg(feature = "machine")]
pub const FEATURE_MACHINE: bool = true;
#[cfg(not(feature = "machine"))]
pub const FEATURE_MACHINE: bool = false;
#[cfg(feature = "machine")]
pub mod machine;

pub const FEATURE_NAME_MACHINE_DYNLIB: &str = "machine_dynlib";
#[cfg(not(feature = "machine_dynlib"))]
pub const FEATURE_MACHINE_DYNLIB: bool = false;
#[cfg(feature = "machine_dynlib")]
pub const FEATURE_MACHINE_DYNLIB: bool = true;

pub const FEATURE_NAME_MACHINE_LUA: &str = "machine_lua";
#[cfg(not(feature = "machine_lua"))]
pub const FEATURE_MACHINE_LUA: bool = false;
#[cfg(feature = "machine_lua")]
pub const FEATURE_MACHINE_LUA: bool = true;

// TODO are these necessary?
// aggregate features
pub const FEATURE_NAME_MACHINE_SANDBOX: &str = "machine_sandbox";
#[cfg(not(feature = "machine_sandbox"))]
pub const FEATURE_MACHINE_SANDBOX: bool = false;
#[cfg(feature = "machine_sandbox")]
pub const FEATURE_MACHINE_SANDBOX: bool = true;

pub const FEATURE_NAME_MACHINE_COMPLETE: &str = "machine_complete";
#[cfg(not(feature = "machine_complete"))]
pub const FEATURE_MACHINE_COMPLETE: bool = false;
#[cfg(feature = "machine_complete")]
pub const FEATURE_MACHINE_COMPLETE: bool = true;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

const SCENARIO_MANIFEST_FILE: &str = "scenario.toml";
const MODULE_MANIFEST_FILE: &str = "mod.toml";

/// Name of the module directory within the scenario file tree.
pub const SCENARIOS_DIR_NAME: &str = "scenarios";
/// Name of the module directory within the scenario file tree.
pub const SNAPSHOTS_DIR_NAME: &str = "snapshots";

/// Name of the module directory within the scenario file tree.
pub const MODULES_DIR_NAME: &str = "mods";
/// Module entry file name, not including the file extension.
const MODULE_ENTRY_FILE_NAME: &str = "mod";

const DEFAULT_MODULE_DEP_VERSION: &str = "*";
const DEFAULT_SCENARIO_MODULE_DEP_VERSION: &str = "*";

#[cfg(feature = "machine")]
const DEFAULT_INACTIVE_STATE: &str = "idle";
#[cfg(feature = "machine")]
const DEFAULT_TRIGGER_EVENT: &str = "step";
#[cfg(feature = "machine")]
const DEFAULT_INIT_EVENT: &str = "init";

/// Floating point numer type used throughout the library.
#[cfg(feature = "small_nums")]
pub type Float = f32;
/// Floating point numer type used throughout the library.
#[cfg(not(feature = "small_nums"))]
pub type Float = f64;
/// Integer number type used throughout the library.
#[cfg(feature = "small_nums")]
pub type Int = i32;
/// Integer number type used throughout the library.
#[cfg(not(feature = "small_nums"))]
pub type Int = i64;

/// Fixed-size string used internally for indexing objects.
///
/// # Length
///
/// Default length is 23 characters, but it can be restricted to just
/// 10 characters using the `short_stringid` feature.
#[cfg(not(feature = "short_stringid"))]
pub type StringId = arrayvec::ArrayString<[u8; 23]>;
/// Fixed-size string used internally for indexing objects.
#[cfg(feature = "short_stringid")]
pub type StringId = arrayvec::ArrayString<[u8; 10]>;

/// Short fixed-size string type.
pub type ShortString = arrayvec::ArrayString<[u8; 23]>;
/// Medium-length fixed-size string type.
type MedString = arrayvec::ArrayString<[u8; 40]>;
/// Long fixed-size string type.
type LongString = arrayvec::ArrayString<[u8; 100]>;

/// Entity string identifier type.
pub type EntityName = StringId;
/// Component string identifier type.
pub type CompName = StringId;
/// Variable string identifier type.
pub type VarName = StringId;
/// Event string identifier type.
pub type EventName = StringId;

/// Entity unique integer identifier type.
pub type EntityId = u32;
