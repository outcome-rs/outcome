//! Contains structs used for procedural deserialization.
//!
//! Note: many objects within user-files are not statically
//! defined, rather a _Value_ object from serde is used.
//! These _Value_ objects are later turned into proper
//! objects based on some conditional logic. This separation
//! allows some fields to accept different data collections.
//! For example scenario manifest's mod dependencies can be
//! either simple 'name:version' or a more complex
//! 'name:[version:version,git:address]', which so far isn't
//! possible with the default struct based serde.

// extern crate serde_yaml;
extern crate linked_hash_map;
extern crate toml;

use std::collections::HashMap;

use self::linked_hash_map::LinkedHashMap;

// use self::serde_yaml::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleManifest {
    #[serde(rename = "mod")]
    pub mod_: ModuleManifestMod,
    #[serde(default)]
    pub dependencies: HashMap<String, toml::Value>,
    #[serde(default)]
    pub reqs: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleManifestMod {
    // required
    pub name: String,
    pub version: String,
    pub engine: toml::Value,

    // optional
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub desc_long: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub website: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScenarioManifest {
    pub scenario: ScenarioManifestScenario,
    #[serde(default)]
    pub mods: LinkedHashMap<String, toml::Value>,
    #[serde(default)]
    pub settings: HashMap<String, toml::Value>,
    #[serde(default)]
    pub services: HashMap<String, toml::Value>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScenarioManifestScenario {
    // required
    pub name: String,
    pub version: String,
    pub engine: String,

    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub desc_long: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub website: String,
}

// TODO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProofManifest {
    // required
    name: String,
    version: String,
    outcome: String,
    scenario: String,
    // optional
    #[serde(default)]
    title: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    desc_long: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    website: String,
}
