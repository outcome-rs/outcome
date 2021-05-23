//! Data query system.

use crate::entity::Entity;
use crate::{
    Address, CompName, EntityId, EntityName, EventName, Float, Int, Result, StringId, Var, VarName,
    VarType,
};
use fnv::FnvHashMap;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Query {
    pub trigger: Trigger,
    pub description: Description,
    pub layout: Layout,
    pub filters: Vec<Filter>,
    pub mappings: Vec<Map>,
}

/// Uniform query product type.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum QueryProduct {
    NativeAddressedVar(FnvHashMap<(EntityId, CompName, VarName), Var>),
    AddressedVar(FnvHashMap<Address, Var>),
    AddressedTyped(AddressedTypedMap),
    OrderedVar(u32, Vec<Var>),
    Var(Vec<Var>),
    Empty,
}

impl QueryProduct {
    // TODO expand beyond only products of the same type
    /// Combines multiple products.
    pub fn combine(mut products: Vec<QueryProduct>) -> QueryProduct {
        let mut final_product = match products.pop() {
            Some(p) => p,
            None => return QueryProduct::Empty,
        };

        for product in products {
            match &mut final_product {
                QueryProduct::AddressedVar(map) => match product {
                    QueryProduct::AddressedVar(_map) => {
                        for (k, v) in _map {
                            if !map.contains_key(&k) {
                                map.insert(k, v);
                            }
                        }
                    }
                    _ => (),
                },
                _ => (),
            }
        }

        final_product
    }
}

#[derive(Default, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AddressedTypedMap {
    pub strings: FnvHashMap<Address, String>,
    pub ints: FnvHashMap<Address, Int>,
    pub floats: FnvHashMap<Address, Float>,
    pub bools: FnvHashMap<Address, bool>,
}

impl Query {
    pub fn process(
        &self,
        entities: &FnvHashMap<u32, Entity>,
        entity_names: &FnvHashMap<EntityName, EntityId>,
    ) -> Result<QueryProduct> {
        let mut selected_entities = entities.keys().map(|v| *v).collect::<Vec<u32>>();
        // println!(
        //     "copying all entity keys took: {} ms",
        //     Instant::now().duration_since(insta).as_millis()
        // );

        // first apply filters and get a list of selected entities
        for filter in &self.filters {
            // let mut to_remove = Vec::new();
            let mut to_retain = Vec::new();
            let insta = std::time::Instant::now();
            match filter {
                Filter::Id(desired_ids) => {
                    for selected_entity_id in &selected_entities {
                        if !desired_ids.contains(&selected_entity_id) {
                            continue;
                        }
                        to_retain.push(*selected_entity_id);
                    }
                }
                Filter::Name(desired_names) => {
                    for selected_entity_id in &selected_entities {
                        // check if the currently iterated selected entity has
                        // a matching entity string idx
                        for (ent_name, ent_id) in entity_names {
                            if !desired_names.contains(ent_name) {
                                continue;
                            }
                            to_retain.push(*ent_id);
                        }
                    }
                }
                Filter::AllComponents(desired_components) => {
                    'ent: for entity_id in &selected_entities {
                        // 'ent: for (entity_id, entity) in entities {
                        if let Some(entity) = entities.get(entity_id) {
                            for desired_component in desired_components {
                                if !entity.components.contains(desired_component) {
                                    continue 'ent;
                                }
                            }
                            to_retain.push(*entity_id);
                        }
                    }
                }
                Filter::Distance(x_addr, y_addr, z_addr, dx, dy, dz) => {
                    // first get the target point position
                    let entity_id = match entity_names.get(&x_addr.entity) {
                        Some(entity_id) => *entity_id,
                        None => match x_addr.entity.parse() {
                            Ok(p) => p,
                            Err(e) => continue,
                        },
                    };

                    // let insta = std::time::Instant::now();
                    let (x, y, z) = if let Some(entity) = entities.get(&entity_id) {
                        (
                            entity
                                .storage
                                .get_var(&x_addr.storage_index())
                                .unwrap()
                                .clone()
                                .to_float(),
                            entity
                                .storage
                                .get_var(&y_addr.storage_index())
                                .unwrap()
                                .clone()
                                .to_float(),
                            entity
                                .storage
                                .get_var(&z_addr.storage_index())
                                .unwrap()
                                .clone()
                                .to_float(),
                        )
                    } else {
                        unimplemented!();
                    };
                    // println!(
                    //     "getting xyz took: {} ms",
                    //     Instant::now().duration_since(insta).as_millis()
                    // );

                    // let insta = std::time::Instant::now();
                    for entity_id in &selected_entities {
                        if let Some(entity) = entities.get(entity_id) {
                            if let Ok(pos_x) = entity
                                .storage
                                .get_var(&("transform".parse().unwrap(), "pos_x".parse().unwrap()))
                            {
                                if (pos_x.to_float() - x).abs() > *dx {
                                    continue;
                                }
                            }
                            if let Ok(pos_y) = entity
                                .storage
                                .get_var(&("transform".parse().unwrap(), "pos_y".parse().unwrap()))
                            {
                                if (pos_y.to_float() - y).abs() > *dy {
                                    continue;
                                }
                            }
                            if let Ok(pos_z) = entity
                                .storage
                                .get_var(&("transform".parse().unwrap(), "pos_z".parse().unwrap()))
                            {
                                if (pos_z.to_float() - z).abs() > *dz {
                                    continue;
                                }
                            }
                            to_retain.push(*entity_id);
                        }
                    }
                    // println!(
                    //     "iterating entities took: {} ms",
                    //     Instant::now().duration_since(insta).as_millis()
                    // );
                }
                Filter::DistanceMultiPoint(multi) => {
                    for (x_addr, y_addr, z_addr, dx, dy, dz) in multi {
                        // first get the target point position
                        let entity_id = match entity_names.get(&x_addr.entity) {
                            Some(entity_id) => *entity_id,
                            None => x_addr.entity.parse().unwrap(),
                        };
                        let (x, y, z) = if let Some(entity) = entities.get(&entity_id) {
                            (
                                entity
                                    .storage
                                    .get_var(&x_addr.storage_index())
                                    .unwrap()
                                    .clone()
                                    .to_float(),
                                entity
                                    .storage
                                    .get_var(&y_addr.storage_index())
                                    .unwrap()
                                    .clone()
                                    .to_float(),
                                entity
                                    .storage
                                    .get_var(&z_addr.storage_index())
                                    .unwrap()
                                    .clone()
                                    .to_float(),
                            )
                        } else {
                            unimplemented!();
                        };

                        for entity_id in &selected_entities {
                            if let Some(entity) = entities.get(entity_id) {
                                if let Ok(pos_x) = entity.storage.get_var(&(
                                    "transform".parse().unwrap(),
                                    "pos_x".parse().unwrap(),
                                )) {
                                    if (pos_x.to_float() - x).abs() > *dx {
                                        continue;
                                    }
                                }
                                if let Ok(pos_y) = entity.storage.get_var(&(
                                    "transform".parse().unwrap(),
                                    "pos_y".parse().unwrap(),
                                )) {
                                    if (pos_y.to_float() - y).abs() > *dy {
                                        continue;
                                    }
                                }
                                if let Ok(pos_z) = entity.storage.get_var(&(
                                    "transform".parse().unwrap(),
                                    "pos_z".parse().unwrap(),
                                )) {
                                    if (pos_z.to_float() - z).abs() > *dz {
                                        continue;
                                    }
                                }
                                to_retain.push(*entity_id);
                            }
                        }
                    }
                }
                _ => unimplemented!(),
            }

            selected_entities = to_retain;
        }

        // let insta = std::time::Instant::now();
        let mut mapped_data = FnvHashMap::default();
        for entity_id in &selected_entities {
            for mapping in &self.mappings {
                match mapping {
                    Map::All => {
                        if let Some(entity) = entities.get(entity_id) {
                            for ((comp_name, var_name), var) in &entity.storage.map {
                                mapped_data.insert((entity_id, comp_name, var_name), var);
                            }
                        }
                        // we've selected everything, disregard other mappings
                        break;
                    }
                    Map::Var(map_var_type, map_var_name) => {
                        if let Some(entity) = entities.get(entity_id) {
                            for ((comp_name, var_name), var) in &entity.storage.map {
                                if &var.get_type() == map_var_type && var_name == map_var_name {
                                    mapped_data.insert((entity_id, comp_name, var_name), var);
                                }
                            }
                        }
                    }
                    Map::VarName(map_var_name) => {
                        if let Some(entity) = entities.get(entity_id) {
                            for ((comp_name, var_name), var) in &entity.storage.map {
                                if var_name == map_var_name {
                                    mapped_data.insert((entity_id, comp_name, var_name), var);
                                }
                            }
                        }
                    }
                    Map::Components(map_components) => {
                        for map_component in map_components {
                            if let Some(entity) = entities.get(entity_id) {
                                for ((comp_name, var_name), var) in &entity.storage.map {
                                    if comp_name == map_component {
                                        mapped_data.insert((entity_id, comp_name, var_name), var);
                                    }
                                }
                            }
                        }
                    }
                    _ => unimplemented!(),
                }
            }
        }
        // println!(
        //     "mapping took: {} ms",
        //     Instant::now().duration_since(insta).as_millis()
        // );

        // let insta = std::time::Instant::now();
        let mut query_product = QueryProduct::Empty;
        match self.description {
            Description::None => match self.layout {
                Layout::Var => {
                    query_product = QueryProduct::Var(
                        mapped_data
                            .into_iter()
                            .map(|(_, var)| var.clone())
                            .collect(),
                    );
                }
                _ => unimplemented!(),
            },
            Description::NativeDescribed => match self.layout {
                Layout::Var => {
                    query_product = QueryProduct::NativeAddressedVar(
                        mapped_data
                            .into_iter()
                            .map(|((ent_id, comp_name, var_name), var)| {
                                ((*ent_id, comp_name.clone(), var_name.clone()), var.clone())
                            })
                            .collect(),
                    );
                }
                _ => unimplemented!(),
            },
            Description::Addressed => match self.layout {
                Layout::Var => {
                    let mut data = FnvHashMap::default();
                    for ((ent_id, comp_name, var_name), var) in mapped_data {
                        let addr = Address {
                            // TODO make it optional to search for entity string name
                            // entity: entity_names
                            //     .iter()
                            //     .find(|(name, id)| id == &ent_id)
                            //     .map(|(name, _)| *name)
                            //     .unwrap_or(ent_id.to_string().parse().unwrap()),
                            entity: ent_id.to_string().parse().unwrap(),
                            component: comp_name.clone(),
                            var_type: var.get_type(),
                            var_name: var_name.clone(),
                        };
                        data.insert(addr, var.clone());
                    }
                    query_product = QueryProduct::AddressedVar(data);
                }
                Layout::Typed => {
                    let mut data = AddressedTypedMap::default();
                    for ((ent_id, comp_name, var_name), var) in mapped_data {
                        let addr = Address {
                            // TODO make it optional to search for entity string name
                            // entity: entity_names
                            // .iter()
                            // .find(|(name, id)| id == &ent_id)
                            // .map(|(name, _)| *name)
                            // .unwrap_or(ent_id.to_string().parse().unwrap()),
                            entity: ent_id.to_string().parse().unwrap(),
                            component: comp_name.clone(),
                            var_type: var.get_type(),
                            var_name: var_name.clone(),
                        };
                        if var.is_float() {
                            data.floats.insert(addr, var.to_float());
                        } else if var.is_bool() {
                            data.bools.insert(addr, var.to_bool());
                        } else if var.is_int() {
                            data.ints.insert(addr, var.to_int());
                        }
                    }
                    query_product = QueryProduct::AddressedTyped(data);
                }
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        }

        // println!(
        //     "packing took: {} ms",
        //     Instant::now().duration_since(insta).as_millis()
        // );

        Ok(query_product)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GlobAddress {
    pub entity: String,
    pub component: String,
    pub var_type: String,
    pub var_id: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Trigger {
    /// Immediate, one-time data transfer
    Immediate,
    /// Trigger each time specific event(s) is fired
    Event(EventName),
    /// Trigger each time certain data is mutated
    Mutation(Address),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Filter {
    /// Select entities that have all the specified components
    AllComponents(Vec<CompName>),
    /// Select entities that have one or more of specified components
    SomeComponents(Vec<CompName>),
    /// Select entities that match any of the provided names
    Name(Vec<EntityName>),
    /// Filter by entity integer id
    Id(Vec<EntityId>),
    /// Filter by some variable being in specified range
    VarRange(Address, Var, Var),
    /// Filter by some variable being in specified range
    AttrRange(StringId, Var, Var),
    /// Filter by entity distance to some point, matching on the position
    /// component (x, y and z coordinates, then x,y and z max distance)
    // TODO use single address to vector3 value
    Distance(Address, Address, Address, Float, Float, Float),
    /// Filter by entity distance to any of multiple points.
    DistanceMultiPoint(Vec<(Address, Address, Address, Float, Float, Float)>),
    /// Select entities currently stored on selected worker nodes
    /// (0 is local worker)
    Node(u32),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Map {
    /// Map all the data stored on selected entities
    All,
    /// Select data based on address string matching
    SelectAddr(Vec<GlobAddress>),
    /// Select data bound to selected components
    Components(Vec<CompName>),
    Var(VarType, VarName),
    VarName(VarName),
    VarType(VarType),
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Description {
    NativeDescribed,
    /// Self-described values, each with attached address
    Addressed,
    StringAddressed,
    // CustomAddressed,
    /// Values ordered based on an order table
    Ordered,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Layout {
    /// Use the internal value representation type built on Rust's enum
    Var,
    /// Use a separate map/list for each variable type
    Typed,
    // TypedSubset(Vec<VarType>),
}
