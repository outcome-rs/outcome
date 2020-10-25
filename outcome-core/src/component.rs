//! Provides component related definitions.

use crate::{error::Result, model, ShortString, StringId};

/// Component struct.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Component {
    pub model_id: StringId,
    #[cfg(feature = "machine")]
    pub current_state: ShortString,
}
impl Component {
    /// Create a new Component from model.
    pub fn from_model(comp_model: &model::ComponentModel) -> Result<Component> {
        Ok(Component {
            model_id: StringId::from(&comp_model.name).unwrap(),
            #[cfg(feature = "machine")]
            current_state: ShortString::from("init").unwrap(),
        })
    }
}
