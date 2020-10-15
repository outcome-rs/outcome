//! Provides component related definitions.

use crate::{model, ShortString, StringId};

/// Component struct.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Component {
    pub model_id: StringId,
    pub current_state: ShortString,
}
impl Component {
    /// Create a new Component from model.
    pub fn from_model(comp_model: &model::ComponentModel) -> Component {
        Component {
            model_id: StringId::from(&comp_model.name).unwrap(),
            current_state: ShortString::from("init").unwrap(),
        }
    }
}
