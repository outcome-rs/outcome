use outcome_core::machine::{cmd::CommandResult, Error, ErrorKind, LocationInfo};
use outcome_core::{
    entity::Storage, entity::StorageIndex, machine::Result, string::new_truncate, CompName,
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
pub fn calculate_entity(entity_id: &EntityId, storage: &mut Storage) -> CommandResult {
    // println!("inside calculate_entity");
    let key = (new_truncate("greeting"), new_truncate("hello"));
    if let Some(hello) = storage.map.get_mut(&key) {
        if let Ok(hello_string) = hello.as_string_mut() {
            hello_string.push_str("[calculated entity inside lib]")
        } else {
            return CommandResult::Err(Error::new(
                LocationInfo::empty(),
                ErrorKind::Other(format!(
                    "unable to get var as string: {:?}, storage: {:?}",
                    key, storage
                )),
            ));
        }
    } else {
        return CommandResult::Err(Error::new(
            LocationInfo::empty(),
            ErrorKind::Other(format!(
                "unable to get var at key: {:?}, storage: {:?}",
                key, storage
            )),
        ));
    }
    CommandResult::Err(Error::new(
        LocationInfo::empty(),
        ErrorKind::Other(format!(
            "dummy error, key: {:?}, storage: {:?}",
            key, storage
        )),
    ))
}
