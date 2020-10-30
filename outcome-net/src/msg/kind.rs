use crate::msg::Payload;
use std::collections::HashMap;

/// One-way heartbeat message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Heartbeat {}
pub(crate) const HEARTBEAT: &str = "Heartbeat";
impl Payload for Heartbeat {
    fn kind_str(&self) -> &str {
        HEARTBEAT
    }
}

/// Requests a simple `PingResponse` message. Can be used to check
/// the connection to the server.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PingRequest {
    pub bytes: Vec<u8>,
}
pub(crate) const PING_REQUEST: &str = "PingRequest";
impl Payload for PingRequest {
    fn kind_str(&self) -> &str {
        PING_REQUEST
    }
}

/// Response to `PingRequest` message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PingResponse {
    pub bytes: Vec<u8>,
}
pub(crate) const PING_RESPONSE: &str = "PingResponse";
impl Payload for PingResponse {
    fn kind_str(&self) -> &str {
        PING_RESPONSE
    }
}

/// Requests a few variables related to the current status of
/// the server.
///
/// NOT IMPLEMENTED `format` can specify what information is needed
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct StatusRequest {
    pub format: String,
}
pub(crate) const STATUS_REQUEST: &str = "StatusRequest";
impl Payload for StatusRequest {
    fn kind_str(&self) -> &str {
        STATUS_REQUEST
    }
}

/// Response containing a few variables related to the current status of
/// the server.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct StatusResponse {
    pub name: String,
    pub description: String,
    pub address: String,
    pub connected_clients: Vec<String>,
    pub endgame_version: String,
    pub uptime: usize,
    pub current_tick: usize,

    pub scenario_name: String,
    pub scenario_title: String,
    pub scenario_desc: String,
    pub scenario_desc_long: String,
    pub scenario_author: String,
    pub scenario_website: String,
    pub scenario_version: String,
    pub scenario_engine: String,
    pub scenario_mods: Vec<String>,
    pub scenario_settings: Vec<String>,
}
pub(crate) const STATUS_RESPONSE: &str = "StatusResponse";
impl Payload for StatusResponse {
    fn kind_str(&self) -> &str {
        STATUS_RESPONSE
    }
}

/// Requests registration of the client who's sending the message.
/// This is the default first message any connecting client has to send
/// before sending anything else.
///
/// If successful the client is added to the server's list of registered
/// clients. Server will try to keep all connections with registered
/// clients alive.
///
/// `name` self assigned name of the client.
///
/// `is_blocking` specifies whether the client is a blocking client.
/// A _blocking client_ is one that has to explicitly agree for the server to start
/// processing the next tick/turn).
///
/// `is_player` specifies whether the client is a player.
/// A _player client_ is one that's limited to only changing decision related
/// data of one entity.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct RegisterClientRequest {
    pub name: String,
    pub addr: Option<String>,
    pub is_blocking: bool,
    pub passwd: Option<String>,
}
pub(crate) const REGISTER_CLIENT_REQUEST: &str = "RegisterClientRequest";
impl Payload for RegisterClientRequest {
    fn kind_str(&self) -> &str {
        REGISTER_CLIENT_REQUEST
    }
}

/// Response to a `RegisterClientRequest` message.
///
/// `error` contains the report of any errors that might have occurred:
/// - `WrongPassword` if the connecting client provided a wrong password
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct RegisterClientResponse {
    pub redirect: String,
    pub error: String,
}
pub(crate) const REGISTER_CLIENT_RESPONSE: &str = "RegisterClientResponse";
impl Payload for RegisterClientResponse {
    fn kind_str(&self) -> &str {
        REGISTER_CLIENT_RESPONSE
    }
}

/// Requests transfer of data from server to client.
///
/// `transfer_type` defines the process of data selection:
///     - `Full` get all the data from the sim database (ignores `selection`)
///     - `Selected` get some selected data, based on the `selection` list
///
/// `selection` is a list of addresses that can be used to select data
/// for transfer.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DataTransferRequest {
    pub transfer_type: String,
    pub selection: Vec<String>,
}
pub(crate) const DATA_TRANSFER_REQUEST: &str = "DataTransferRequest";
impl Payload for DataTransferRequest {
    fn kind_str(&self) -> &str {
        DATA_TRANSFER_REQUEST
    }
}

/// Response to `DataTransferRequest`.
///
/// `data` structure containing a set of lists containing different types of data.
///
/// `error` contains the report of any errors that might have occurred.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DataTransferResponse {
    pub data: Option<SimDataPack>,
    pub error: String,
}
pub(crate) const DATA_TRANSFER_RESPONSE: &str = "DataTransferResponse";
impl Payload for DataTransferResponse {
    fn kind_str(&self) -> &str {
        DATA_TRANSFER_RESPONSE
    }
}

/// Structure holding all data organized based on data types.
///
/// Each data type is represented by a set of key-value pairs, where
/// keys are addresses represented with strings.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SimDataPack {
    pub strings: HashMap<String, String>,
    pub ints: HashMap<String, outcome_core::Int>,
    pub floats: HashMap<String, outcome_core::Float>,
    pub bools: HashMap<String, bool>,
    pub string_lists: HashMap<String, Vec<String>>,
    pub int_lists: HashMap<String, Vec<outcome_core::Int>>,
    pub float_lists: HashMap<String, Vec<outcome_core::Float>>,
    pub bool_lists: HashMap<String, Vec<bool>>,
    pub string_grids: HashMap<String, Vec<Vec<String>>>,
    pub int_grids: HashMap<String, Vec<Vec<outcome_core::Int>>>,
    pub float_grids: HashMap<String, Vec<Vec<outcome_core::Float>>>,
    pub bool_grids: HashMap<String, Vec<Vec<bool>>>,
}
impl SimDataPack {
    pub fn empty() -> SimDataPack {
        SimDataPack {
            strings: HashMap::new(),
            ints: HashMap::new(),
            floats: HashMap::new(),
            bools: HashMap::new(),
            string_lists: HashMap::new(),
            int_lists: HashMap::new(),
            float_lists: HashMap::new(),
            bool_lists: HashMap::new(),
            string_grids: HashMap::new(),
            int_grids: HashMap::new(),
            float_grids: HashMap::new(),
            bool_grids: HashMap::new(),
        }
    }
    pub fn add(&mut self, addr: &outcome::Address, value_str: &str) {
        match addr.var_type {
            outcome::VarType::Str => {
                self.strings.insert(addr.to_string(), value_str.to_owned());
            }
            outcome::VarType::Int => {
                self.ints.insert(
                    addr.to_string(),
                    value_str.parse::<outcome_core::Int>().unwrap(),
                );
            }
            outcome::VarType::Float => {
                self.floats.insert(
                    addr.to_string(),
                    value_str.parse::<outcome_core::Float>().unwrap(),
                );
            }
            outcome::VarType::Bool => {
                self.bools
                    .insert(addr.to_string(), value_str.parse::<bool>().unwrap());
            }
            _ => (),
        };
        ()
    }
}

/// Request the server to pull provided data into the main simulation
/// database.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DataPullRequest {
    pub data: SimDataPack,
}
pub(crate) const DATA_PULL_REQUEST: &str = "DataPullRequest";
impl Payload for DataPullRequest {
    fn kind_str(&self) -> &str {
        DATA_PULL_REQUEST
    }
}

/// Response to `DataPullRequest`.
///
/// `error` contains the report of any errors that might have occurred.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DataPullResponse {
    pub error: String,
}
pub(crate) const DATA_PULL_RESPONSE: &str = "DataPullResponse";
impl Payload for DataPullResponse {
    fn kind_str(&self) -> &str {
        DATA_PULL_RESPONSE
    }
}

/// Requests an advancement of the simulation by a turn, which the client
/// understands as a set number of simulation ticks. This number is
/// sent within the request.
///
/// In a situation with multiple blocking clients, this request acts
/// as a "thumbs up" signal from the client sending it. Until all
/// blocking clients have sent the signal that they are _ready_,
/// processing cannot continue.
///
/// `TurnAdvanceRequest` is only valid for clients that are _blocking_.
/// If the client has `is_blocking` option set to true then
/// the server will block processing every time it sends
/// a `TurnAdvanceResponse` to that client. If the client is not
/// blocking the server will ignore the request and the response to
/// this request will contain an error.
///
/// `tick_count` is the number of ticks the client considers _one turn_.
/// Server takes this value and sends a `TurnAdvanceResponse`
/// only after a number of ticks equal to the value of `tick_count`
/// is processed.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TurnAdvanceRequest {
    pub tick_count: u32,
}
pub(crate) const TURN_ADVANCE_REQUEST: &str = "TurnAdvanceRequest";
impl Payload for TurnAdvanceRequest {
    fn kind_str(&self) -> &str {
        TURN_ADVANCE_REQUEST
    }
}

/// Response to `TurnAdvanceRequest`.
///
/// `error` contains report of errors if any were encountered.
/// Possible errors include:
/// - `ClientIsNotBlocking`
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TurnAdvanceResponse {
    pub error: String,
}
pub(crate) const TURN_ADVANCE_RESPONSE: &str = "TurnAdvanceResponse";
impl Payload for TurnAdvanceResponse {
    fn kind_str(&self) -> &str {
        TURN_ADVANCE_RESPONSE
    }
}

/// Requests the server to list all local (available on the
/// server) scenarios.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListLocalScenariosRequest {}

/// Response to `ListLocalScenariosRequest`.
///
/// `scenarios` contains a list of scenarios available locally
/// on the server that can be loaded.
///
/// `error` can contain:
/// - `NoScenariosFound`
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListLocalScenariosResponse {
    pub scenarios: Vec<String>,
    pub error: String,
}

/// Requests the server to load a local (available on the
/// server) scenario using the provided scenario name.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct LoadLocalScenarioRequest {
    pub scenario: String,
}

/// Response to `LoadLocalScenarioRequest`.
///
/// `error` can contain:
/// - `ScenarioNotFound`
/// - `FailedCreatingSimInstance`
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct LoadLocalScenarioResponse {
    pub error: String,
}

/// Requests the server to load a scenario included in the message.
/// Scenario data here is user files as collections of bytes.
///
/// `scenario_manifest` is the manifest file of the scenario.
///
/// `modules` contains a list of modules, each _module_ being
/// itself a collection of files. Files for each module are
/// laid out "flat", regardless of how they may have originally
/// been organized into multiple directories, etc.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct LoadRemoteScenarioRequest {
    pub scenario_manifest: Vec<u8>,
    pub modules: Vec<Vec<u8>>,
}

/// Response to `LoadRemoteScenarioRequest`.
///
/// `error` can contain:
/// - `FailedCreatingSimInstance`
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct LoadRemoteScenarioResponse {
    pub error: String,
}

// #[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
// pub struct InitializeNodeRequest {
//     pub model: String,
// }

impl Payload for outcome::distr::Signal {
    fn kind_str(&self) -> &str {
        "Signal"
    }
}

impl Payload for ListLocalScenariosRequest {
    fn kind_str(&self) -> &str {
        "ListLocalScenariosRequest"
    }
}
impl Payload for ListLocalScenariosResponse {
    fn kind_str(&self) -> &str {
        "ListLocalScenariosResponse"
    }
}
impl Payload for LoadLocalScenarioRequest {
    fn kind_str(&self) -> &str {
        "LoadLocalScenarioRequest"
    }
}
impl Payload for LoadLocalScenarioResponse {
    fn kind_str(&self) -> &str {
        "LoadLocalScenarioResponse"
    }
}
impl Payload for LoadRemoteScenarioRequest {
    fn kind_str(&self) -> &str {
        "LoadRemoteScenarioRequest"
    }
}
impl Payload for LoadRemoteScenarioResponse {
    fn kind_str(&self) -> &str {
        "LoadRemoteScenarioResponse"
    }
}
