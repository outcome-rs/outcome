//! Variable types and their transformations.

use std::fmt;

use crate::error::{Error, Result};

// default values for base var types
const DEFAULT_STR_VALUE: &str = "";
const DEFAULT_INT_VALUE: crate::Int = 0;
const DEFAULT_FLOAT_VALUE: crate::Float = 0.0;
const DEFAULT_BOOL_VALUE: bool = false;

const STR_VAR_TYPE_NAME: &str = "str";
const STR_VAR_TYPE_NAME_ALT: &str = "string";
const INT_VAR_TYPE_NAME: &str = "int";
const INT_VAR_TYPE_NAME_ALT: &str = "integer";
const FLOAT_VAR_TYPE_NAME: &str = "float";
const FLOAT_VAR_TYPE_NAME_ALT: &str = "flt";
const BOOL_VAR_TYPE_NAME: &str = "bool";
const BOOL_VAR_TYPE_NAME_ALT: &str = "boolean";
const STR_LIST_VAR_TYPE_NAME: &str = "str_list";
const STR_LIST_VAR_TYPE_NAME_ALT: &str = "string_list";
const INT_LIST_VAR_TYPE_NAME: &str = "int_list";
const INT_LIST_VAR_TYPE_NAME_ALT: &str = "integer_list";
const FLOAT_LIST_VAR_TYPE_NAME: &str = "float_list";
const FLOAT_LIST_VAR_TYPE_NAME_ALT: &str = "flt_list";
const BOOL_LIST_VAR_TYPE_NAME: &str = "bool_list";
const BOOL_LIST_VAR_TYPE_NAME_ALT: &str = "boolean_list";
const STR_GRID_VAR_TYPE_NAME: &str = "str_grid";
const STR_GRID_VAR_TYPE_NAME_ALT: &str = "string_grid";
const INT_GRID_VAR_TYPE_NAME: &str = "int_grid";
const INT_GRID_VAR_TYPE_NAME_ALT: &str = "integer_grid";
const FLOAT_GRID_VAR_TYPE_NAME: &str = "float_grid";
const FLOAT_GRID_VAR_TYPE_NAME_ALT: &str = "flt_grid";
const BOOL_GRID_VAR_TYPE_NAME: &str = "bool_grid";
const BOOL_GRID_VAR_TYPE_NAME_ALT: &str = "boolean_grid";

/// Defines all possible types of values.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum VarType {
    Str,
    Int,
    Float,
    Bool,
    StrList,
    IntList,
    FloatList,
    BoolList,
    #[cfg(feature = "grids")]
    StrGrid,
    #[cfg(feature = "grids")]
    IntGrid,
    #[cfg(feature = "grids")]
    FloatGrid,
    #[cfg(feature = "grids")]
    BoolGrid,
}

impl fmt::Display for VarType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(formatter, "{}", self.to_str())
    }
}

/// List of all possible variable types.
pub static VAR_TYPES: &[&str; 12] = &[
    STR_VAR_TYPE_NAME,
    INT_VAR_TYPE_NAME,
    FLOAT_VAR_TYPE_NAME,
    BOOL_VAR_TYPE_NAME,
    STR_LIST_VAR_TYPE_NAME,
    INT_LIST_VAR_TYPE_NAME,
    FLOAT_LIST_VAR_TYPE_NAME,
    BOOL_LIST_VAR_TYPE_NAME,
    STR_GRID_VAR_TYPE_NAME,
    INT_GRID_VAR_TYPE_NAME,
    FLOAT_GRID_VAR_TYPE_NAME,
    BOOL_GRID_VAR_TYPE_NAME,
];

impl VarType {
    /// Creates new `VarType` from str.
    pub fn from_str(s: &str) -> Result<VarType> {
        let var_type = match s {
            STR_VAR_TYPE_NAME | STR_VAR_TYPE_NAME_ALT => VarType::Str,
            INT_VAR_TYPE_NAME | INT_VAR_TYPE_NAME_ALT => VarType::Int,
            FLOAT_VAR_TYPE_NAME | FLOAT_VAR_TYPE_NAME_ALT => VarType::Float,
            BOOL_VAR_TYPE_NAME | BOOL_VAR_TYPE_NAME_ALT => VarType::Bool,
            STR_LIST_VAR_TYPE_NAME | STR_LIST_VAR_TYPE_NAME_ALT => VarType::StrList,
            INT_LIST_VAR_TYPE_NAME | INT_LIST_VAR_TYPE_NAME_ALT => VarType::IntList,
            FLOAT_LIST_VAR_TYPE_NAME | FLOAT_LIST_VAR_TYPE_NAME_ALT => VarType::FloatList,
            BOOL_LIST_VAR_TYPE_NAME | BOOL_LIST_VAR_TYPE_NAME_ALT => VarType::BoolList,
            #[cfg(feature = "grids")]
            STR_GRID_VAR_TYPE_NAME | STR_GRID_VAR_TYPE_NAME_ALT => VarType::StrGrid,
            #[cfg(feature = "grids")]
            INT_GRID_VAR_TYPE_NAME | INT_GRID_VAR_TYPE_NAME_ALT => VarType::IntGrid,
            #[cfg(feature = "grids")]
            FLOAT_GRID_VAR_TYPE_NAME | FLOAT_GRID_VAR_TYPE_NAME_ALT => VarType::FloatGrid,
            #[cfg(feature = "grids")]
            BOOL_GRID_VAR_TYPE_NAME | BOOL_GRID_VAR_TYPE_NAME_ALT => VarType::BoolGrid,
            _ => return Err(Error::InvalidVarType(s.to_string())),
        };
        Ok(var_type)
    }

    /// Creates new `VarType` from str. Panics on invalid input.
    pub fn from_str_unchecked(s: &str) -> VarType {
        let var_type = match s {
            STR_VAR_TYPE_NAME | STR_VAR_TYPE_NAME_ALT => VarType::Str,
            INT_VAR_TYPE_NAME | INT_VAR_TYPE_NAME_ALT => VarType::Int,
            FLOAT_VAR_TYPE_NAME | FLOAT_VAR_TYPE_NAME_ALT => VarType::Float,
            BOOL_VAR_TYPE_NAME | BOOL_VAR_TYPE_NAME_ALT => VarType::Bool,
            STR_LIST_VAR_TYPE_NAME | STR_LIST_VAR_TYPE_NAME_ALT => VarType::StrList,
            INT_LIST_VAR_TYPE_NAME | INT_LIST_VAR_TYPE_NAME_ALT => VarType::IntList,
            FLOAT_LIST_VAR_TYPE_NAME | FLOAT_LIST_VAR_TYPE_NAME_ALT => VarType::FloatList,
            BOOL_LIST_VAR_TYPE_NAME | BOOL_LIST_VAR_TYPE_NAME_ALT => VarType::BoolList,
            #[cfg(feature = "grids")]
            STR_GRID_VAR_TYPE_NAME | STR_GRID_VAR_TYPE_NAME_ALT => VarType::StrGrid,
            #[cfg(feature = "grids")]
            INT_GRID_VAR_TYPE_NAME | INT_GRID_VAR_TYPE_NAME_ALT => VarType::IntGrid,
            #[cfg(feature = "grids")]
            FLOAT_GRID_VAR_TYPE_NAME | FLOAT_GRID_VAR_TYPE_NAME_ALT => VarType::FloatGrid,
            #[cfg(feature = "grids")]
            BOOL_GRID_VAR_TYPE_NAME | BOOL_GRID_VAR_TYPE_NAME_ALT => VarType::BoolGrid,
            _ => panic!("invalid var type: {}", s),
        };
        var_type
    }

    /// Returns string literal name of the `VarType`.
    pub fn to_str(&self) -> &str {
        match self {
            VarType::Str => STR_VAR_TYPE_NAME,
            VarType::Int => INT_VAR_TYPE_NAME,
            VarType::Float => FLOAT_VAR_TYPE_NAME,
            VarType::Bool => BOOL_VAR_TYPE_NAME,
            VarType::StrList => STR_LIST_VAR_TYPE_NAME,
            VarType::IntList => INT_LIST_VAR_TYPE_NAME,
            VarType::FloatList => FLOAT_LIST_VAR_TYPE_NAME,
            VarType::BoolList => BOOL_LIST_VAR_TYPE_NAME,
            #[cfg(feature = "grids")]
            VarType::StrGrid => STR_GRID_VAR_TYPE_NAME,
            #[cfg(feature = "grids")]
            VarType::IntGrid => INT_GRID_VAR_TYPE_NAME,
            #[cfg(feature = "grids")]
            VarType::FloatGrid => FLOAT_GRID_VAR_TYPE_NAME,
            #[cfg(feature = "grids")]
            VarType::BoolGrid => BOOL_GRID_VAR_TYPE_NAME,
        }
    }

    /// Get default value of the `VarType`.
    pub fn default_value(&self) -> Var {
        match self {
            VarType::Str => Var::Str(DEFAULT_STR_VALUE.to_string()),
            VarType::Int => Var::Int(DEFAULT_INT_VALUE),
            VarType::Float => Var::Float(DEFAULT_FLOAT_VALUE),
            VarType::Bool => Var::Bool(DEFAULT_BOOL_VALUE),
            VarType::StrList => Var::StrList(Vec::new()),
            VarType::IntList => Var::IntList(Vec::new()),
            VarType::FloatList => Var::FloatList(Vec::new()),
            VarType::BoolList => Var::BoolList(Vec::new()),
            #[cfg(feature = "grids")]
            VarType::StrGrid => Var::StrGrid(Vec::new()),
            #[cfg(feature = "grids")]
            VarType::IntGrid => Var::IntGrid(Vec::new()),
            #[cfg(feature = "grids")]
            VarType::FloatGrid => Var::FloatGrid(Vec::new()),
            #[cfg(feature = "grids")]
            VarType::BoolGrid => Var::BoolGrid(Vec::new()),
            // TODO implement other var types
            _ => unimplemented!(),
        }
    }
}

/// Abstraction over all available variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Var {
    Str(String),
    Int(crate::Int),
    Float(crate::Float),
    Bool(bool),
    StrList(Vec<String>),
    IntList(Vec<crate::Int>),
    FloatList(Vec<crate::Float>),
    BoolList(Vec<bool>),
    #[cfg(feature = "grids")]
    StrGrid(Vec<Vec<String>>),
    #[cfg(feature = "grids")]
    IntGrid(Vec<Vec<crate::Int>>),
    #[cfg(feature = "grids")]
    FloatGrid(Vec<Vec<crate::Float>>),
    #[cfg(feature = "grids")]
    BoolGrid(Vec<Vec<bool>>),
}

impl Var {
    pub fn new(var_type: &VarType) -> Self {
        match var_type {
            VarType::Str => Var::Str(DEFAULT_STR_VALUE.to_string()),
            VarType::Int => Var::Int(DEFAULT_INT_VALUE),
            VarType::Float => Var::Float(DEFAULT_FLOAT_VALUE),
            VarType::Bool => Var::Bool(DEFAULT_BOOL_VALUE),
            _ => unimplemented!(),
        }
    }
    pub fn get_type(&self) -> VarType {
        match self {
            Var::Str(_) => VarType::Str,
            Var::Int(_) => VarType::Int,
            Var::Float(_) => VarType::Float,
            Var::Bool(_) => VarType::Bool,
            Var::StrList(_) => VarType::StrList,
            Var::IntList(_) => VarType::IntList,
            Var::FloatList(_) => VarType::FloatList,
            Var::BoolList(_) => VarType::BoolList,
            #[cfg(feature = "grids")]
            Var::StrGrid(_) => VarType::StrGrid,
            #[cfg(feature = "grids")]
            Var::IntGrid(_) => VarType::IntGrid,
            #[cfg(feature = "grids")]
            Var::FloatGrid(_) => VarType::FloatGrid,
            #[cfg(feature = "grids")]
            Var::BoolGrid(_) => VarType::BoolGrid,
        }
    }
}

/// Type-strict `is_type` checkers.
impl Var {
    pub fn is_str(&self) -> bool {
        match self {
            Var::Str(_) => true,
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

    pub fn is_str_list(&self) -> bool {
        match self {
            Var::StrList(_) => true,
            _ => false,
        }
    }

    pub fn is_int_list(&self) -> bool {
        match self {
            Var::IntList(_) => true,
            _ => false,
        }
    }

    pub fn is_float_list(&self) -> bool {
        match self {
            Var::FloatList(_) => true,
            _ => false,
        }
    }

    pub fn is_bool_list(&self) -> bool {
        match self {
            Var::BoolList(_) => true,
            _ => false,
        }
    }
}

/// Type-strict `as_type` getters.
impl Var {
    pub fn as_str(&self) -> Result<&String> {
        match self {
            Var::Str(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_str_mut(&mut self) -> Result<&mut String> {
        match self {
            Var::Str(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int(&self) -> Result<&crate::Int> {
        match self {
            Var::Int(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_mut(&mut self) -> Result<&mut crate::Int> {
        match self {
            Var::Int(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float(&self) -> Result<&crate::Float> {
        match self {
            Var::Float(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_mut(&mut self) -> Result<&mut crate::Float> {
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

    pub fn as_str_list(&self) -> Result<&Vec<String>> {
        match self {
            Var::StrList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_str_list_mut(&mut self) -> Result<&mut Vec<String>> {
        match self {
            Var::StrList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected string list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_list(&self) -> Result<&Vec<crate::Int>> {
        match self {
            Var::IntList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_list_mut(&mut self) -> Result<&mut Vec<crate::Int>> {
        match self {
            Var::IntList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_list(&self) -> Result<&Vec<crate::Float>> {
        match self {
            Var::FloatList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_list_mut(&mut self) -> Result<&mut Vec<crate::Float>> {
        match self {
            Var::FloatList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_list(&self) -> Result<&Vec<bool>> {
        match self {
            Var::BoolList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool list, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_list_mut(&mut self) -> Result<&mut Vec<bool>> {
        match self {
            Var::BoolList(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool list, got {}",
                self.get_type().to_str()
            ))),
        }
    }
}

#[cfg(feature = "grids")]
impl Var {
    pub fn as_str_grid(&self) -> Result<&Vec<Vec<String>>> {
        match self {
            Var::StrGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected str grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_str_grid_mut(&mut self) -> Result<&mut Vec<Vec<String>>> {
        match self {
            Var::StrGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected str grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_grid(&self) -> Result<&Vec<Vec<crate::Int>>> {
        match self {
            Var::IntGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_int_grid_mut(&mut self) -> Result<&mut Vec<Vec<crate::Int>>> {
        match self {
            Var::IntGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected int grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_grid(&self) -> Result<&Vec<Vec<crate::Float>>> {
        match self {
            Var::FloatGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_float_grid_mut(&mut self) -> Result<&mut Vec<Vec<crate::Float>>> {
        match self {
            Var::FloatGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected float grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_grid(&self) -> Result<&Vec<Vec<bool>>> {
        match self {
            Var::BoolGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn as_bool_grid_mut(&mut self) -> Result<&mut Vec<Vec<bool>>> {
        match self {
            Var::BoolGrid(v) => Ok(v),
            _ => Err(Error::InvalidVarType(format!(
                "expected bool grid, got {}",
                self.get_type().to_str()
            ))),
        }
    }

    pub fn is_str_grid(&self) -> bool {
        match self {
            Var::StrGrid(_) => true,
            _ => false,
        }
    }

    pub fn is_int_grid(&self) -> bool {
        match self {
            Var::IntGrid(_) => true,
            _ => false,
        }
    }

    pub fn is_float_grid(&self) -> bool {
        match self {
            Var::FloatGrid(_) => true,
            _ => false,
        }
    }

    pub fn is_bool_grid(&self) -> bool {
        match self {
            Var::BoolGrid(_) => true,
            _ => false,
        }
    }
}

impl Var {
    pub fn from_str(string: &str, target_type: Option<VarType>) -> Result<Var> {
        match target_type {
            Some(tt) => match tt {
                VarType::Str => Ok(Var::Str(string.to_string())),
                VarType::Int => Ok(Var::Int(string.parse::<crate::Int>()?)),
                VarType::Float => Ok(Var::Float(string.parse::<crate::Float>()?)),
                VarType::Bool => Ok(Var::Bool(string.parse::<bool>()?)),
                _ => unimplemented!(),
            },
            None => {
                if string.starts_with('"') {
                    if string.ends_with('"') {
                        return Ok(Var::Str(string.to_string()));
                    } else {
                        return Err(Error::Other("".to_string()));
                    }
                } else if string == "true" || string == "false" {
                    return Ok(Var::Bool(string.parse::<bool>().unwrap()));
                } else {
                    match string.parse::<crate::Int>() {
                        Ok(i) => return Ok(Var::Int(i)),
                        Err(e) => return Err(Error::Other(e.to_string())),
                    }
                }
            }
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Var::Str(v) => v.clone(),
            Var::Int(v) => format!("{}", v),
            Var::Float(v) => format!("{}", v),
            Var::Bool(v) => format!("{}", v),
            Var::StrList(v) => format!("{:?}", v),
            Var::IntList(v) => format!("{:?}", v),
            Var::FloatList(v) => format!("{:?}", v),
            Var::BoolList(v) => format!("{:?}", v),
            #[cfg(feature = "grids")]
            Var::StrGrid(v) => format!("{:?}", v),
            #[cfg(feature = "grids")]
            Var::IntGrid(v) => format!("{:?}", v),
            #[cfg(feature = "grids")]
            Var::FloatGrid(v) => format!("{:?}", v),
            #[cfg(feature = "grids")]
            Var::BoolGrid(v) => format!("{:?}", v),
        }
    }

    pub fn to_int(&self) -> crate::Int {
        match self {
            Var::Str(v) => v.len() as crate::Int,
            Var::Int(v) => *v,
            Var::Float(v) => *v as crate::Int,
            Var::Bool(v) => {
                if *v {
                    1
                } else {
                    0
                }
            }
            Var::StrList(v) => v.len() as crate::Int,
            Var::IntList(v) => v.len() as crate::Int,
            Var::FloatList(v) => v.len() as crate::Int,
            Var::BoolList(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::StrGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::IntGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::FloatGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::BoolGrid(v) => v.len() as crate::Int,
        }
    }

    pub fn to_float(&self) -> crate::Float {
        match self {
            Var::Str(v) => v.len() as crate::Float,
            Var::Int(v) => *v as crate::Float,
            Var::Float(v) => *v,
            Var::Bool(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            Var::StrList(v) => v.len() as crate::Float,
            Var::IntList(v) => v.len() as crate::Float,
            Var::FloatList(v) => v.len() as crate::Float,
            Var::BoolList(v) => v.len() as crate::Float,
            #[cfg(feature = "grids")]
            Var::StrGrid(v) => v.len() as crate::Float,
            #[cfg(feature = "grids")]
            Var::IntGrid(v) => v.len() as crate::Float,
            #[cfg(feature = "grids")]
            Var::FloatGrid(v) => v.len() as crate::Float,
            #[cfg(feature = "grids")]
            Var::BoolGrid(v) => v.len() as crate::Float,
        }
    }

    pub fn to_bool(&self) -> bool {
        let num: crate::Int = match self {
            Var::Str(v) => v.len() as crate::Int,
            Var::Int(v) => *v,
            Var::Float(v) => *v as crate::Int,
            Var::Bool(v) => return *v,
            Var::StrList(v) => v.len() as crate::Int,
            Var::IntList(v) => v.len() as crate::Int,
            Var::FloatList(v) => v.len() as crate::Int,
            Var::BoolList(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::StrGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::IntGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::FloatGrid(v) => v.len() as crate::Int,
            #[cfg(feature = "grids")]
            Var::BoolGrid(v) => v.len() as crate::Int,
        };
        match num {
            0 => false,
            _ => true,
        }
    }
}
