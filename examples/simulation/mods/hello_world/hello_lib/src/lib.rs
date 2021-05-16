use outcome_core::machine::cmd::CommandResult;
use outcome_core::{
    arraystring::new_truncate, entity::Storage, entity::StorageIndex, machine::Result, CompName,
    EntityId,
};

// #[no_mangle]
// pub fn minicall() -> u8 {
//     3u8
// }

#[no_mangle]
pub fn calculate_string(hello_string: &mut String) -> Result<CommandResult> {
    // something
    hello_string.push_str("[calculated string inside lib]");
    Ok(CommandResult::Continue)
}

#[no_mangle]
pub fn calculate_entity(entity_id: &EntityId, storage: &mut Storage) {
    let key = (new_truncate("greeting"), new_truncate("hello"));
    if let Some(hello) = storage.map.get_mut(&key) {
        hello
            .as_string_mut()
            .unwrap()
            .push_str("[calculated entity inside lib]");
    }
    // Ok(CommandResult::Continue)
}
