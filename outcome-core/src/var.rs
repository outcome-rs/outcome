//! Defines all the available variable types and their transformations.

use std::fmt;

// default values for base var types
const DEFAULT_STR_VALUE: &str = "";
const DEFAULT_INT_VALUE: i32 = 0;
const DEFAULT_FLOAT_VALUE: f32 = 0.0;
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
    StrGrid,
    IntGrid,
    FloatGrid,
    BoolGrid,
}
impl fmt::Display for VarType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "{}", self.to_str())
    }
}
/// List of all possible types of values.
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
    pub fn from_str(s: &str) -> Option<VarType> {
        let var_type = match s {
            STR_VAR_TYPE_NAME | STR_VAR_TYPE_NAME_ALT => VarType::Str,
            INT_VAR_TYPE_NAME | INT_VAR_TYPE_NAME_ALT => VarType::Int,
            FLOAT_VAR_TYPE_NAME | FLOAT_VAR_TYPE_NAME_ALT => VarType::Float,
            BOOL_VAR_TYPE_NAME | BOOL_VAR_TYPE_NAME_ALT => VarType::Bool,
            STR_LIST_VAR_TYPE_NAME | STR_LIST_VAR_TYPE_NAME_ALT => VarType::StrList,
            INT_LIST_VAR_TYPE_NAME | INT_LIST_VAR_TYPE_NAME_ALT => VarType::IntList,
            FLOAT_LIST_VAR_TYPE_NAME | FLOAT_LIST_VAR_TYPE_NAME_ALT => VarType::FloatList,
            BOOL_LIST_VAR_TYPE_NAME | BOOL_LIST_VAR_TYPE_NAME_ALT => VarType::BoolList,
            STR_GRID_VAR_TYPE_NAME | STR_GRID_VAR_TYPE_NAME_ALT => VarType::StrGrid,
            INT_GRID_VAR_TYPE_NAME | INT_GRID_VAR_TYPE_NAME_ALT => VarType::IntGrid,
            FLOAT_GRID_VAR_TYPE_NAME | FLOAT_GRID_VAR_TYPE_NAME_ALT => VarType::FloatGrid,
            BOOL_GRID_VAR_TYPE_NAME | BOOL_GRID_VAR_TYPE_NAME_ALT => VarType::BoolGrid,
            _ => return None,
        };
        Some(var_type)
    }
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
            STR_GRID_VAR_TYPE_NAME | STR_GRID_VAR_TYPE_NAME_ALT => VarType::StrGrid,
            INT_GRID_VAR_TYPE_NAME | INT_GRID_VAR_TYPE_NAME_ALT => VarType::IntGrid,
            FLOAT_GRID_VAR_TYPE_NAME | FLOAT_GRID_VAR_TYPE_NAME_ALT => VarType::FloatGrid,
            BOOL_GRID_VAR_TYPE_NAME | BOOL_GRID_VAR_TYPE_NAME_ALT => VarType::BoolGrid,
            _ => panic!("failed creating var_type from: {}", s),
        };
        var_type
    }
    pub fn from_string(string: String) -> Option<VarType> {
        VarType::from_str(string.as_str())
    }
    /// Get the name of the VarType.
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
            VarType::StrGrid => STR_GRID_VAR_TYPE_NAME,
            VarType::IntGrid => INT_GRID_VAR_TYPE_NAME,
            VarType::FloatGrid => FLOAT_GRID_VAR_TYPE_NAME,
            VarType::BoolGrid => BOOL_GRID_VAR_TYPE_NAME,
        }
    }

    /// Get default value as string.
    pub fn default_value_str(&self) -> String {
        match self {
            VarType::Str => DEFAULT_STR_VALUE.to_string(),
            VarType::Int => DEFAULT_INT_VALUE.to_string(),
            VarType::Float => DEFAULT_FLOAT_VALUE.to_string(),
            VarType::Bool => DEFAULT_BOOL_VALUE.to_string(),
            // TODO implement other var types
            _ => "err".to_string(),
        }
    }
}

/// Abstraction over all possible values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Var {
    Str(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    StrList(Vec<String>),
    IntList(Vec<i32>),
    FloatList(Vec<f32>),
    BoolList(Vec<bool>),
    StrGrid(Vec<Vec<String>>),
    IntGrid(Vec<Vec<i32>>),
    FloatGrid(Vec<Vec<f32>>),
    BoolGrid(Vec<Vec<bool>>),
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
/// Type-strict `as_type` getters.
impl Var {
    pub fn as_str(&self) -> Option<&String> {
        match self {
            Var::Str(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str_mut(&mut self) -> Option<&mut String> {
        match self {
            Var::Str(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<&i32> {
        match self {
            Var::Int(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_mut(&mut self) -> Option<&mut i32> {
        match self {
            Var::Int(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float(&self) -> Option<&f32> {
        match self {
            Var::Float(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float_mut(&mut self) -> Option<&mut f32> {
        match self {
            Var::Float(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<&bool> {
        match self {
            Var::Bool(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool_mut(&mut self) -> Option<&mut bool> {
        match self {
            Var::Bool(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str_list(&self) -> Option<&Vec<String>> {
        match self {
            Var::StrList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str_list_mut(&mut self) -> Option<&mut Vec<String>> {
        match self {
            Var::StrList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_list(&self) -> Option<&Vec<i32>> {
        match self {
            Var::IntList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_list_mut(&mut self) -> Option<&mut Vec<i32>> {
        match self {
            Var::IntList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float_list(&self) -> Option<&Vec<f32>> {
        match self {
            Var::FloatList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float_list_mut(&mut self) -> Option<&mut Vec<f32>> {
        match self {
            Var::FloatList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool_list(&self) -> Option<&Vec<bool>> {
        match self {
            Var::BoolList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool_list_mut(&mut self) -> Option<&mut Vec<bool>> {
        match self {
            Var::BoolList(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str_grid(&self) -> Option<&Vec<Vec<String>>> {
        match self {
            Var::StrGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str_grid_mut(&mut self) -> Option<&mut Vec<Vec<String>>> {
        match self {
            Var::StrGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_grid(&self) -> Option<&Vec<Vec<i32>>> {
        match self {
            Var::IntGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_grid_mut(&mut self) -> Option<&mut Vec<Vec<i32>>> {
        match self {
            Var::IntGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float_grid(&self) -> Option<&Vec<Vec<f32>>> {
        match self {
            Var::FloatGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_float_grid_mut(&mut self) -> Option<&mut Vec<Vec<f32>>> {
        match self {
            Var::FloatGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool_grid(&self) -> Option<&Vec<Vec<bool>>> {
        match self {
            Var::BoolGrid(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool_grid_mut(&mut self) -> Option<&mut Vec<Vec<bool>>> {
        match self {
            Var::BoolGrid(v) => Some(v),
            _ => None,
        }
    }
}

impl Var {
    pub fn from_str(string: &str, target_type: Option<VarType>) -> Option<Var> {
        match target_type {
            Some(tt) => match tt {
                VarType::Str => Some(Var::Str(string.to_string())),
                VarType::Int => match string.parse::<i32>() {
                    Ok(p) => Some(Var::Int(p)),
                    Err(_) => None,
                },
                VarType::Float => match string.parse::<f32>() {
                    Ok(p) => Some(Var::Float(p)),
                    Err(_) => None,
                },
                VarType::Bool => match string.parse::<bool>() {
                    Ok(p) => Some(Var::Bool(p)),
                    Err(_) => None,
                },
                _ => unimplemented!(),
            },
            None => {
                if string.starts_with('"') {
                    if string.ends_with('"') {
                        return Some(Var::Str(string.to_string()));
                    } else {
                        return None;
                    }
                } else if string == "true" || string == "false" {
                    return Some(Var::Bool(string.parse::<bool>().unwrap()));
                } else {
                    match string.parse::<i32>() {
                        Ok(i) => return Some(Var::Int(i)),
                        Err(e) => return None,
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
            Var::StrGrid(v) => format!("{:?}", v),
            Var::IntGrid(v) => format!("{:?}", v),
            Var::FloatGrid(v) => format!("{:?}", v),
            Var::BoolGrid(v) => format!("{:?}", v),
        }
    }
    pub fn to_int(&self) -> i32 {
        match self {
            Var::Str(v) => v.len() as i32,
            Var::Int(v) => *v,
            Var::Float(v) => *v as i32,
            Var::Bool(v) => {
                if *v {
                    1
                } else {
                    0
                }
            }
            Var::StrList(v) => v.len() as i32,
            Var::IntList(v) => v.len() as i32,
            Var::FloatList(v) => v.len() as i32,
            Var::BoolList(v) => v.len() as i32,
            Var::StrGrid(v) => v.len() as i32,
            Var::IntGrid(v) => v.len() as i32,
            Var::FloatGrid(v) => v.len() as i32,
            Var::BoolGrid(v) => v.len() as i32,
        }
    }
    pub fn to_float(&self) -> f32 {
        match self {
            Var::Str(v) => v.len() as f32,
            Var::Int(v) => *v as f32,
            Var::Float(v) => *v,
            Var::Bool(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            Var::StrList(v) => v.len() as f32,
            Var::IntList(v) => v.len() as f32,
            Var::FloatList(v) => v.len() as f32,
            Var::BoolList(v) => v.len() as f32,
            Var::StrGrid(v) => v.len() as f32,
            Var::IntGrid(v) => v.len() as f32,
            Var::FloatGrid(v) => v.len() as f32,
            Var::BoolGrid(v) => v.len() as f32,
        }
    }
    pub fn to_bool(&self) -> bool {
        let num: i32 = match self {
            Var::Str(v) => v.len() as i32,
            Var::Int(v) => *v,
            Var::Float(v) => *v as i32,
            Var::Bool(v) => return *v,
            Var::StrList(v) => v.len() as i32,
            Var::IntList(v) => v.len() as i32,
            Var::FloatList(v) => v.len() as i32,
            Var::BoolList(v) => v.len() as i32,
            Var::StrGrid(v) => v.len() as i32,
            Var::IntGrid(v) => v.len() as i32,
            Var::FloatGrid(v) => v.len() as i32,
            Var::BoolGrid(v) => v.len() as i32,
        };
        match num {
            0 => false,
            _ => true,
        }
    }
}
