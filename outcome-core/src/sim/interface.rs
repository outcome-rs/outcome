use crate::error::{Error, Result};
use crate::{Address, EntityId, ShortString, SimModel, StringId, Var};
use std::collections::HashMap;
use std::path::PathBuf;

/// Defines public interface for interacting with the simulation.
///
/// This trait exists so that both local `Sim` and distributed coordinator
/// `SimCentral` can use the same interface for accessing data and executing
/// commands.
pub trait SimInterface
where
    Self: SimInterfaceStorage + std::marker::Sized,
{
    fn get_clock(&self) -> usize;

    fn from_scenario_at(path: PathBuf) -> Result<Self>;
    fn from_model(model: SimModel) -> Result<Self>;

    fn apply_model(&mut self) -> Result<()>;
    fn get_model(&self) -> &SimModel;
    fn get_model_mut(&mut self) -> &mut SimModel;

    fn get_event_queue(&self) -> &Vec<StringId>;
    fn get_event_queue_mut(&mut self) -> &mut Vec<StringId>;

    fn get_entity_handles(&self) -> Vec<EntityId>;

    fn add_entity(&mut self, model_type: &str, model_id: &str, new_id: &str) -> Result<()>;

    #[cfg(feature = "machine_lua")]
    fn setup_lua_state(&mut self);
}

pub trait SimInterfaceStorage {
    fn get_as_string(&self, addr: &Address) -> Option<String>;
    fn get_as_int(&self, addr: &Address) -> Option<i32>;

    fn get_all_as_strings(&self) -> HashMap<String, String>;

    fn get_var(&self, addr: &Address) -> Option<Var>;

    fn get_str(&self, addr: &Address) -> Option<&String>;
    fn get_str_mut(&mut self, addr: &Address) -> Option<&mut String>;
    fn get_int(&self, addr: &Address) -> Option<&i32>;
    fn get_int_mut(&mut self, addr: &Address) -> Option<&mut i32>;
    fn get_float(&self, addr: &Address) -> Option<&f32>;
    fn get_float_mut(&mut self, addr: &Address) -> Option<&mut f32>;
    fn get_bool(&self, addr: &Address) -> Option<&bool>;
    fn get_bool_mut(&mut self, addr: &Address) -> Option<&mut bool>;
    fn get_str_list(&self, addr: &Address) -> Option<&Vec<String>>;
    fn get_str_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<String>>;
    fn get_int_list(&self, addr: &Address) -> Option<&Vec<i32>>;
    fn get_int_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<i32>>;
    fn get_float_list(&self, addr: &Address) -> Option<&Vec<f32>>;
    fn get_float_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<f32>>;
    fn get_bool_list(&self, addr: &Address) -> Option<&Vec<bool>>;
    fn get_bool_list_mut(&mut self, addr: &Address) -> Option<&mut Vec<bool>>;
    fn get_str_grid(&self, addr: &Address) -> Option<&Vec<Vec<String>>>;
    fn get_str_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<String>>>;
    fn get_int_grid(&self, addr: &Address) -> Option<&Vec<Vec<i32>>>;
    fn get_int_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<i32>>>;
    fn get_float_grid(&self, addr: &Address) -> Option<&Vec<Vec<f32>>>;
    fn get_float_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<f32>>>;
    fn get_bool_grid(&self, addr: &Address) -> Option<&Vec<Vec<bool>>>;
    fn get_bool_grid_mut(&mut self, addr: &Address) -> Option<&mut Vec<Vec<bool>>>;

    fn set_from_string(&mut self, addr: &Address, val: &String) -> Result<()>;
    fn set_from_string_list(&mut self, addr: &Address, vec: &Vec<String>) -> Result<()>;
    fn set_from_string_grid(&mut self, addr: &Address, vec2d: &Vec<Vec<String>>) -> Result<()>;
}
