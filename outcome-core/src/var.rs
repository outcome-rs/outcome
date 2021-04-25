//! Variable types and their transformations.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use fnv::FnvHashMap;
use serde_repr::*;

use crate::error::{Error, Result};
use crate::{Float, Int};

// default values for base var types
const DEFAULT_STR_VALUE: &str = "";
const DEFAULT_INT_VALUE: Int = 0;
const DEFAULT_FLOAT_VALUE: Float = 0.;
const DEFAULT_BOOL_VALUE: bool = false;
const DEFAULT_BYTE_VALUE: u8 = 0;

const STRING_VAR_TYPE_NAME: &str = "str";
const INT_VAR_TYPE_NAME: &str = "int";
const FLOAT_VAR_TYPE_NAME: &str = "float";
const BOOL_VAR_TYPE_NAME: &str = "bool";
const BYTE_VAR_TYPE_NAME: &str = "byte";
const VEC2_VAR_TYPE_NAME: &str = "vec2";
const VEC3_VAR_TYPE_NAME: &str = "vec3";

const LIST_VAR_TYPE_NAME: &str = "list";
const GRID_VAR_TYPE_NAME: &str = "grid";
const MAP_VAR_TYPE_NAME: &str = "map";

const VAR_TYPE_NAME_SEPARATOR: &str = "_";

const VALUE_SEPARATOR: char = ',';

/// Defines all possible types of values.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum VarType {
    String,
    Int,
    Float,
    Bool,
    Byte,
    Vec2,
    Vec3,

    StringList,
    IntList,
    FloatList,
    BoolList,
    ByteList,
    Vec2List,
    Vec3List,
    VarList,

    StringGrid,
    IntGrid,
    FloatGrid,
    BoolGrid,
    ByteGrid,
    Vec2Grid,
    Vec3Grid,
    VarGrid,

    Map,
}

impl fmt::Display for VarType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(formatter, "{}", self.to_str())
    }
}

impl VarType {
    /// Creates new `VarType` from str.
    pub fn from_str(s: &str) -> Result<VarType> {
        let var_type = match s {
            STRING_VAR_TYPE_NAME => VarType::String,
            INT_VAR_TYPE_NAME => VarType::Int,
            FLOAT_VAR_TYPE_NAME => VarType::Float,
            BOOL_VAR_TYPE_NAME => VarType::Bool,
            BYTE_VAR_TYPE_NAME => VarType::Byte,
            VEC2_VAR_TYPE_NAME => VarType::Vec2,
            VEC3_VAR_TYPE_NAME => VarType::Vec3,
            _ => {
                let split = s.split(VAR_TYPE_NAME_SEPARATOR).collect::<Vec<&str>>();
                if split.len() != 2 {
                    unimplemented!()
                }
                match split[0] {
                    LIST_VAR_TYPE_NAME => match split[1] {
                        STRING_VAR_TYPE_NAME => VarType::StringList,
                        INT_VAR_TYPE_NAME => VarType::IntList,
                        FLOAT_VAR_TYPE_NAME => VarType::FloatList,
                        BOOL_VAR_TYPE_NAME => VarType::BoolList,
                        BYTE_VAR_TYPE_NAME => VarType::ByteList,
                        VEC2_VAR_TYPE_NAME => VarType::Vec2List,
                        VEC3_VAR_TYPE_NAME => VarType::Vec3List,
                        _ => unimplemented!(),
                    },
                    GRID_VAR_TYPE_NAME => match split[1] {
                        STRING_VAR_TYPE_NAME => VarType::StringGrid,
                        INT_VAR_TYPE_NAME => VarType::IntGrid,
                        FLOAT_VAR_TYPE_NAME => VarType::FloatGrid,
                        BOOL_VAR_TYPE_NAME => VarType::BoolGrid,
                        BYTE_VAR_TYPE_NAME => VarType::ByteGrid,
                        VEC2_VAR_TYPE_NAME => VarType::Vec2Grid,
                        VEC3_VAR_TYPE_NAME => VarType::Vec3Grid,
                        _ => unimplemented!(),
                    },
                    MAP_VAR_TYPE_NAME => VarType::Map,
                    _ => unimplemented!(),
                }
            }
            _ => return Err(Error::InvalidVarType(s.to_string())),
        };
        Ok(var_type)
    }

    /// Creates new `VarType` from str. Panics on invalid input.
    pub fn from_str_unchecked(s: &str) -> VarType {
        let var_type = match s {
            STRING_VAR_TYPE_NAME => VarType::String,
            INT_VAR_TYPE_NAME => VarType::Int,
            FLOAT_VAR_TYPE_NAME => VarType::Float,
            BOOL_VAR_TYPE_NAME => VarType::Bool,
            BYTE_VAR_TYPE_NAME => VarType::Byte,
            VEC2_VAR_TYPE_NAME => VarType::Vec2,
            VEC3_VAR_TYPE_NAME => VarType::Vec3,
            LIST_VAR_TYPE_NAME => VarType::VarList,
            GRID_VAR_TYPE_NAME => VarType::VarGrid,
            MAP_VAR_TYPE_NAME => VarType::Map,
            _ => panic!("invalid var type: {}", s),
        };
        var_type
    }

    /// Returns string literal name of the `VarType`.
    pub fn to_str(&self) -> &str {
        match self {
            VarType::String => STRING_VAR_TYPE_NAME,
            VarType::Int => INT_VAR_TYPE_NAME,
            VarType::Float => FLOAT_VAR_TYPE_NAME,
            VarType::Bool => BOOL_VAR_TYPE_NAME,
            VarType::Byte => BYTE_VAR_TYPE_NAME,
            VarType::Vec2 => VEC2_VAR_TYPE_NAME,
            VarType::Vec3 => VEC3_VAR_TYPE_NAME,
            VarType::VarList => LIST_VAR_TYPE_NAME,
            VarType::VarGrid => GRID_VAR_TYPE_NAME,
            VarType::Map => MAP_VAR_TYPE_NAME,
            VarType::StringList => "list_str",
            VarType::IntList => "list_int",
            VarType::FloatList => "list_float",
            VarType::BoolList => "list_bool",
            VarType::ByteList => "list_byte",
            VarType::Vec2List => "list_vec2",
            VarType::Vec3List => "list_vec3",
            VarType::StringGrid => "grid_str",
            VarType::IntGrid => "grid_int",
            VarType::FloatGrid => "grid_float",
            VarType::BoolGrid => "grid_bool",
            VarType::ByteGrid => "grid_byte",
            VarType::Vec2Grid => "grid_vec2",
            VarType::Vec3Grid => "grid_vec3",
        }
    }

    /// Get default value of the `VarType`.
    pub fn default_value(&self) -> Var {
        match self {
            VarType::String => Var::String(DEFAULT_STR_VALUE.to_string()),
            VarType::Int => Var::Int(DEFAULT_INT_VALUE),
            VarType::Float => Var::Float(DEFAULT_FLOAT_VALUE),
            VarType::Bool => Var::Bool(DEFAULT_BOOL_VALUE),
            VarType::Byte => Var::Byte(DEFAULT_BYTE_VALUE),
            VarType::Vec2 => Var::Vec2(DEFAULT_FLOAT_VALUE, DEFAULT_FLOAT_VALUE),
            VarType::Vec3 => Var::Vec3(
                DEFAULT_FLOAT_VALUE,
                DEFAULT_FLOAT_VALUE,
                DEFAULT_FLOAT_VALUE,
            ),
            VarType::StringList
            | VarType::IntList
            | VarType::FloatList
            | VarType::BoolList
            | VarType::ByteList
            | VarType::Vec2List
            | VarType::Vec3List
            | VarType::VarList => Var::List(Vec::new()),
            VarType::StringGrid
            | VarType::IntGrid
            | VarType::FloatGrid
            | VarType::BoolGrid
            | VarType::ByteGrid
            | VarType::Vec2Grid
            | VarType::Vec3Grid
            | VarType::VarGrid => Var::List(Vec::new()),
            VarType::Map => Var::Map(BTreeMap::new()),
            _ => unimplemented!(),
        }
    }
}

/// Abstraction over all available variables.
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Var {
    String(String),
    Int(Int),
    Float(Float),
    Bool(bool),
    Byte(u8),
    Vec2(Float, Float),
    Vec3(Float, Float, Float),
    List(Vec<Var>),
    Grid(Vec<Vec<Var>>),
    Map(BTreeMap<Var, Var>),
}

impl Eq for Var {}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl Var {
    pub fn new(var_type: &VarType) -> Self {
        match var_type {
            VarType::String => Var::String(DEFAULT_STR_VALUE.to_string()),
            VarType::Int => Var::Int(DEFAULT_INT_VALUE),
            VarType::Float => Var::Float(DEFAULT_FLOAT_VALUE),
            VarType::Bool => Var::Bool(DEFAULT_BOOL_VALUE),
            VarType::Byte => Var::Byte(DEFAULT_BYTE_VALUE),
            VarType::Vec2 => Var::Vec2(DEFAULT_FLOAT_VALUE, DEFAULT_FLOAT_VALUE),
            VarType::Vec3 => Var::Vec3(
                DEFAULT_FLOAT_VALUE,
                DEFAULT_FLOAT_VALUE,
                DEFAULT_FLOAT_VALUE,
            ),
            VarType::StringList
            | VarType::IntList
            | VarType::FloatList
            | VarType::BoolList
            | VarType::ByteList
            | VarType::Vec2List
            | VarType::Vec3List
            | VarType::VarList => Var::List(Vec::new()),
            VarType::StringGrid
            | VarType::IntGrid
            | VarType::FloatGrid
            | VarType::BoolGrid
            | VarType::ByteGrid
            | VarType::Vec2Grid
            | VarType::Vec3Grid
            | VarType::VarGrid => Var::Grid(Vec::new()),
            VarType::Map => Var::Map(Default::default()),
        }
    }

    pub fn get_type(&self) -> VarType {
        match self {
            Var::String(_) => VarType::String,
            Var::Int(_) => VarType::Int,
            Var::Float(_) => VarType::Float,
            Var::Bool(_) => VarType::Bool,
            Var::Byte(_) => VarType::Byte,
            Var::Vec2(_, _) => VarType::Vec2,
            Var::Vec3(_, _, _) => VarType::Vec3,
            Var::List(list) => {
                if let Some(first) = list.first() {
                    match first.get_type() {
                        VarType::String => VarType::StringList,
                        VarType::Int => VarType::IntList,
                        VarType::Float => VarType::FloatList,
                        VarType::Bool => VarType::BoolList,
                        VarType::Byte => VarType::ByteList,
                        VarType::Vec2 => VarType::Vec2List,
                        VarType::Vec3 => VarType::Vec3List,
                        _ => VarType::VarList,
                    }
                } else {
                    VarType::VarList
                }
            }
            Var::Grid(grid) => {
                if let Some(first) = grid.first() {
                    if let Some(_first) = first.first() {
                        match _first.get_type() {
                            VarType::String => VarType::StringGrid,
                            VarType::Int => VarType::IntGrid,
                            VarType::Float => VarType::FloatGrid,
                            VarType::Bool => VarType::BoolGrid,
                            VarType::Byte => VarType::ByteGrid,
                            VarType::Vec2 => VarType::Vec2Grid,
                            VarType::Vec3 => VarType::Vec3Grid,
                            _ => VarType::VarGrid,
                        }
                    } else {
                        VarType::VarGrid
                    }
                } else {
                    VarType::VarGrid
                }
            }
            Var::Map(_) => VarType::Map,
            _ => unimplemented!(),
        }
    }

    pub fn set_coerce(&mut self, other: &Var) -> Result<()> {
        match self {
            Var::String(v) => *v = other.to_string(),
            Var::Int(v) => *v = other.to_int(),
            Var::Float(v) => *v = other.to_float(),
            Var::Bool(v) => *v = other.to_bool(),
            // Var::Byte(v) => *v = other.to_byte()?,
            _ => unimplemented!(),
        }
        Ok(())
    }

    pub fn coerce(&self, target_type: VarType) -> Result<Var> {
        let out = match target_type {
            VarType::String => Var::String(self.to_string()),
            VarType::Int => Var::Int(self.to_int()),
            VarType::Float => Var::Float(self.to_float()),
            VarType::Bool => Var::Bool(self.to_bool()),
            // Var::Byte(v) => *v = other.to_byte()?,
            _ => unimplemented!(),
        };
        Ok(out)
    }
}

impl Var {
    pub fn is_string(&self) -> bool {
        match self {
            Var::String(_) => true,
            _ => false,
        }
    }

    pub fn is_int(&self) -> bool {
        match self {
            Var::Int(_) => true,
            _ => false,
        }
    }

    pub fn is_float(&self) -> bool {
        match self {
            Var::Float(_) => true,
            _ => false,
        }
    }

    pub fn is_bool(&self) -> bool {
        match self {
            Var::Bool(_) => true,
            _ => false,
        }
    }
}

impl Var {
    pub fn as_string(&self) -> Result<&String> {
        match self {
            Var::String(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_string_mut(&mut self) -> Result<&mut String> {
        match self {
            Var::String(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int(&self) -> Result<&Int> {
        match self {
            Var::Int(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_mut(&mut self) -> Result<&mut Int> {
        match self {
            Var::Int(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float(&self) -> Result<&Float> {
        match self {
            Var::Float(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_mut(&mut self) -> Result<&mut Float> {
        match self {
            Var::Float(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool(&self) -> Result<&bool> {
        match self {
            Var::Bool(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_mut(&mut self) -> Result<&mut bool> {
        match self {
            Var::Bool(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_list(&self) -> Result<&Vec<Var>> {
        match self {
            Var::List(v) => Ok(v),
            _ => unimplemented!(),
        }
    }

    pub fn as_list_mut(&mut self) -> Result<&mut Vec<Var>> {
        match self {
            Var::List(v) => Ok(v),
            _ => unimplemented!(),
        }
    }

    pub fn as_grid(&self) -> Result<&Vec<Vec<Var>>> {
        match self {
            Var::Grid(v) => Ok(v),
            _ => unimplemented!(),
        }
    }

    pub fn as_grid_mut(&mut self) -> Result<&mut Vec<Vec<Var>>> {
        match self {
            Var::Grid(v) => Ok(v),
            _ => unimplemented!(),
        }
    }

    pub fn as_str_list(&self) -> Result<Vec<&String>> {
        match self {
            Var::List(v) => Ok(v.iter().map(|s| s.as_string().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected string list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_str_list_mut(&mut self) -> Result<Vec<&mut String>> {
        match self {
            Var::List(v) => Ok(v.iter_mut().map(|s| s.as_string_mut().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected string list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_list(&self) -> Result<Vec<&Int>> {
        match self {
            Var::List(v) => Ok(v.iter().map(|_v| _v.as_int().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected int list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_list_mut(&mut self) -> Result<Vec<&mut Int>> {
        match self {
            Var::List(v) => Ok(v.iter_mut().map(|_v| _v.as_int_mut().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected int list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_list(&self) -> Result<Vec<&Float>> {
        match self {
            Var::List(v) => Ok(v.iter().map(|_v| _v.as_float().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected float list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_list_mut(&mut self) -> Result<Vec<&mut Float>> {
        match self {
            Var::List(v) => Ok(v.iter_mut().map(|_v| _v.as_float_mut().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected float list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_list(&self) -> Result<Vec<&bool>> {
        match self {
            Var::List(v) => Ok(v.iter().map(|_v| _v.as_bool().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_list_mut(&mut self) -> Result<Vec<&mut bool>> {
        match self {
            Var::List(v) => Ok(v.iter_mut().map(|_v| _v.as_bool_mut().unwrap()).collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool list, got {}",
                self.get_type().to_str()
            ))),
        }
    }
}

impl Var {
    pub fn as_string_grid(&self) -> Result<Vec<Vec<&String>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter()
                .map(|_v| _v.iter().map(|__v| __v.as_string().unwrap()).collect())
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected string grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_string_grid_mut(&mut self) -> Result<Vec<Vec<&mut String>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter_mut()
                .map(|_v| {
                    _v.iter_mut()
                        .map(|__v| __v.as_string_mut().unwrap())
                        .collect()
                })
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected string grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_grid(&self) -> Result<Vec<Vec<&Int>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter()
                .map(|_v| _v.iter().map(|__v| __v.as_int().unwrap()).collect())
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected int grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_grid_mut(&mut self) -> Result<Vec<Vec<&mut Int>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter_mut()
                .map(|_v| _v.iter_mut().map(|__v| __v.as_int_mut().unwrap()).collect())
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected int grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_grid(&self) -> Result<Vec<Vec<&Float>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter()
                .map(|_v| _v.iter().map(|__v| __v.as_float().unwrap()).collect())
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected float grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_grid_mut(&mut self) -> Result<Vec<Vec<&mut Float>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter_mut()
                .map(|_v| {
                    _v.iter_mut()
                        .map(|__v| __v.as_float_mut().unwrap())
                        .collect()
                })
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected float grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_grid(&self) -> Result<Vec<Vec<&bool>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter()
                .map(|_v| _v.iter().map(|__v| __v.as_bool().unwrap()).collect())
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_grid_mut(&mut self) -> Result<Vec<Vec<&mut bool>>> {
        match self {
            Var::Grid(v) => Ok(v
                .iter_mut()
                .map(|_v| {
                    _v.iter_mut()
                        .map(|__v| __v.as_bool_mut().unwrap())
                        .collect()
                })
                .collect()),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn is_string_grid(&self) -> bool {
        self.get_type() == VarType::StringGrid
    }

    pub fn is_int_grid(&self) -> bool {
        self.get_type() == VarType::IntGrid
    }

    pub fn is_float_grid(&self) -> bool {
        self.get_type() == VarType::FloatGrid
    }

    pub fn is_bool_grid(&self) -> bool {
        self.get_type() == VarType::BoolGrid
    }
}

impl Var {
    pub fn from_str(s: &str, target_type: Option<VarType>) -> Result<Var> {
        let var = match target_type {
            Some(tt) => match tt {
                VarType::String => Var::String(s.to_string()),
                VarType::Int => Var::Int(s.parse::<Int>()?),
                VarType::Float => Var::Float(s.parse::<Float>()?),
                VarType::Bool => Var::Bool(s.parse::<bool>()?),
                VarType::Byte => Var::Byte(s.parse::<u8>()?),
                VarType::Vec2 => {
                    let split = s.split(VALUE_SEPARATOR).collect::<Vec<&str>>();
                    if split.len() != 2 {
                        return Err(Error::FailedCreatingVar(s.to_string()));
                    }
                    Var::Vec2(split[0].parse::<Float>()?, split[1].parse::<Float>()?)
                }
                VarType::Vec3 => {
                    let split = s.split(VALUE_SEPARATOR).collect::<Vec<&str>>();
                    if split.len() != 3 {
                        return Err(Error::FailedCreatingVar(s.to_string()));
                    }
                    Var::Vec3(
                        split[0].parse::<Float>()?,
                        split[1].parse::<Float>()?,
                        split[2].parse::<Float>()?,
                    )
                }
                VarType::StringList
                | VarType::IntList
                | VarType::FloatList
                | VarType::BoolList
                | VarType::ByteList
                | VarType::Vec2List
                | VarType::Vec3List
                | VarType::VarList => list_from_str(s, tt)?,
                VarType::StringGrid
                | VarType::IntGrid
                | VarType::FloatGrid
                | VarType::BoolGrid
                | VarType::ByteGrid
                | VarType::Vec2Grid
                | VarType::Vec3Grid
                | VarType::VarGrid => unimplemented!(),
                VarType::Map => unimplemented!(),
            },
            None => {
                if s.starts_with('"') {
                    if s.ends_with('"') {
                        return Ok(Var::String(s.to_string()));
                    } else {
                        return Err(Error::Other("".to_string()));
                    }
                } else if s == "true" || s == "false" {
                    return Ok(Var::Bool(s.parse::<bool>().unwrap()));
                } else {
                    match s.parse::<Int>() {
                        Ok(i) => return Ok(Var::Int(i)),
                        Err(e) => return Err(Error::Other(e.to_string())),
                    }
                }
            }
        };
        Ok(var)
    }

    pub fn to_string(&self) -> String {
        match self {
            Var::String(v) => v.clone(),
            Var::Int(v) => format!("{}", v),
            Var::Float(v) => format!("{}", v),
            Var::Bool(v) => format!("{}", v),
            Var::Byte(v) => format!("{}", v),
            Var::Vec2(v1, v2) => format!("x: {}, y: {}", v1, v2),
            Var::Vec3(v1, v2, v3) => format!("x: {}, y: {}, z: {}", v1, v2, v3),
            Var::List(v) => format!("{:?}", v),
            Var::Grid(v) => format!("{:?}", v),
            Var::Map(v) => format!("{:?}", v),
        }
    }

    pub fn to_int(&self) -> Int {
        match self {
            Var::String(v) => v.len() as Int,
            Var::Int(v) => *v,
            Var::Float(v) => *v as Int,
            Var::Bool(v) => {
                if *v {
                    1
                } else {
                    0
                }
            }
            Var::Byte(v) => *v as Int,
            Var::Vec2(v1, v2) => *v1 as Int + *v2 as Int,
            Var::Vec3(v1, v2, v3) => *v1 as Int + *v2 as Int + *v3 as Int,
            Var::List(v) => v.len() as Int,
            Var::Grid(v) => v.len() as Int,
            Var::Map(v) => v.len() as Int,
        }
    }

    pub fn to_float(&self) -> Float {
        match self {
            Var::String(v) => v.len() as Float,
            Var::Int(v) => *v as Float,
            Var::Float(v) => *v,
            Var::Bool(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            Var::Byte(v) => *v as Float,
            Var::Vec2(v1, v2) => v1 + v2,
            Var::Vec3(v1, v2, v3) => v1 + v2 + v3,
            Var::List(v) => v.len() as Float,
            Var::Grid(v) => v.len() as Float,
            Var::Map(v) => v.len() as Float,
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Var::String(v) => v.len() > 0,
            Var::Int(v) => *v > 0,
            Var::Float(v) => *v > 0.,
            Var::Bool(v) => return *v,
            Var::Byte(v) => return *v > 0,
            Var::Vec2(v1, v2) => *v1 > 0. && *v2 > 0.,
            Var::Vec3(v1, v2, v3) => *v1 > 0. && *v2 > 0. && *v3 > 0.,
            Var::List(v) => v.len() > 0,
            Var::Grid(v) => v.len() > 0,
            Var::Map(v) => v.len() > 0,
        }
    }
}

// TODO support nested lists
fn list_from_str(s: &str, var_type: VarType) -> Result<Var> {
    let split = s.split(VALUE_SEPARATOR).collect::<Vec<&str>>();
    let mut vec = Vec::new();
    for v in split {
        let var = match var_type {
            VarType::StringList => Var::String(v.to_string()),
            VarType::IntList => Var::Int(v.parse()?),
            VarType::FloatList => Var::Float(v.parse()?),
            VarType::BoolList => Var::Bool(v.parse()?),
            VarType::ByteList => Var::Byte(v.parse()?),
            // VarType::Vec2List => Var::Vec2(v.parse()?),
            // VarType::Vec3List => Var::Vec3(v.parse()?),
            _ => unimplemented!(),
        };
        vec.push(var);
    }
    Ok(Var::List(vec))
}
