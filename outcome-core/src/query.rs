/// Implements data query system.
///
/// # Design
///
/// - single query is represented by a `Query` struct
/// - each `Query` defines a set of restrictions that are
/// used to narrow down what data should be returned
/// - processing of a `Query` is not certain to return
/// data
///
use crate::{Address, Sim, StringId, Var};

pub struct Query {
    pub parts: Vec<Part>,
}

impl Query {
    pub fn new() -> Self {
        Query { parts: Vec::new() }
    }
    pub fn process(&self, sim: &Sim) -> Option<Vec<(Address, &Var)>> {
        // if self.parts.is_empty() {
        //     return None;
        // };
        // let mut included_entities = Vec::new();
        // let mut included_components = Vec::new();
        // // let mut included_vars = Vec::new();
        // // let mut output = Vec::new();
        // for part in &self.parts {
        //     match part {
        //         Part::IncludeEntityByName(name) => {
        //             if sim.entities_idx.keys().find(|i| i == &name).is_some() {
        //                 included_entities.push(*name);
        //             }
        //         }
        //         Part::ExcludeEntitiesByType(types) => (),
        //         _ => (),
        //     }
        // }
        // for included_entity in &included_entities {
        //     for part in &self.parts {
        //         match part {
        //             Part::IncludeComponentByName(name) => {
        //                 // if let Some(entity) = sim.get_entity()
        //                 if sim.entities_idx.get(included_entity).is_none() {
        //                     continue;
        //                 };
        //                 let mut addr = Address::from_uids(included_entity, name);
        //                 included_components.push(addr);
        //             }
        //             Part::IncludeAllComponents => {
        //                 if let Some(entity) = sim.get_entity_str(included_entity) {
        //                     for name in entity.components.map.keys() {
        //                         let addr = Address::from_uids(included_entity, name);
        //                         included_components.push(addr);
        //                     }
        //                 }
        //             }
        //             _ => (),
        //         }
        //     }
        // }

        None
    }
}

pub enum Part {
    IncludeEntityByName(StringId),
    ExcludeEntityByName(StringId),
    // *by type* means *by attached components*
    IncludeEntitiesByType(Vec<StringId>),
    ExcludeEntitiesByType(Vec<StringId>),
    IncludeAllComponents,
    IncludeComponentByName(StringId),
}
impl Part {
    pub fn met_for(&self, sim: &Sim) -> bool {
        unimplemented!()
    }
}
