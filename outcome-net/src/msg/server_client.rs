//! # External API
//!
//! Collection of structures that represent messages (and their parts)
//! that can be sent between server and client during two-way exchange.

#![allow(unused)]

extern crate lz4;
extern crate rmp_serde;
extern crate serde;

use std::collections::HashMap;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::net::TcpStream;

//use self::byteorder::{ByteOrder, LittleEndian};

use self::lz4::block::CompressionMode;
use self::rmp_serde::{Deserializer, Serializer};
use self::serde::{Deserialize, Serialize};
use crate::msg::Payload;
use std::io;
use std::time::Duration;

pub const BUF_SIZE: usize = 1024;

pub const PING_REQUEST: &str = "PingRequest";
pub const PING_RESPONSE: &str = "PingResponse";
pub const STATUS_REQUEST: &str = "StatusRequest";
pub const STATUS_RESPONSE: &str = "StatusResponse";

pub const CHAT_MSG_PULL_REQUEST: &str = "ChatMsgPullRequest";
pub const CHAT_MSG_PULL_RESPONSE: &str = "ChatMsgPullResponse";
pub const CHAT_MSG_TRANSFER_REQUEST: &str = "ChatMsgTransferRequest";
pub const CHAT_MSG_TRANSFER_RESPONSE: &str = "ChatMsgTransferResponse";

pub const REGISTER_CLIENT_REQUEST: &str = "RegisterClientRequest";
pub const REGISTER_CLIENT_RESPONSE: &str = "RegisterClientResponse";
pub const SET_CLIENT_OPTIONS_REQUEST: &str = "SetClientOptionsRequest";
pub const SET_CLIENT_OPTIONS_RESPONSE: &str = "SetClientOptionsResponse";

pub const DATA_TRANSFER_REQUEST: &str = "DataTransferRequest";
pub const DATA_TRANSFER_RESPONSE: &str = "DataTransferResponse";
pub const DATA_PULL_REQUEST: &str = "DataPullRequest";
pub const DATA_PULL_RESPONSE: &str = "DataPullResponse";

pub const TURN_ADVANCE_REQUEST: &str = "TurnAdvanceRequest";
pub const TURN_ADVANCE_RESPONSE: &str = "TurnAdvanceResponse";

pub const LIST_LOCAL_SCENARIOS_REQUEST: &str = "ListLocalScenariosRequest";
pub const LIST_LOCAL_SCENARIOS_RESPONSE: &str = "ListLocalScenariosResponse";
//TODO
//pub const LIST_LOCAL_SNAPSHOTS_REQUEST: &str = "ListLocalSnapshotsRequest";
//pub const LIST_LOCAL_SNAPSHOTS_RESPONSE: &str = "ListLocalSnapshotsResponse";
//pub const LIST_LOCAL_PROOFS_REQUEST: &str = "ListLocalProofsRequest";
//pub const LIST_LOCAL_PROOFS_RESPONSE: &str = "ListLocalProofsResponse";

pub const LOAD_LOCAL_SCENARIO_REQUEST: &str = "LoadLocalScenarioRequest";
pub const LOAD_LOCAL_SCENARIO_RESPONSE: &str = "LoadLocalScenarioResponse";

pub const LOAD_REMOTE_SCENARIO_REQUEST: &str = "LoadRemoteScenarioRequest";
pub const LOAD_REMOTE_SCENARIO_RESPONSE: &str = "LoadRemoteScenarioResponse";

/////////////////////////////////
// Message types (payloads)
/////////////////////////////////
