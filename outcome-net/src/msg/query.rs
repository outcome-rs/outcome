use crate::Error;
use serde_repr::*;
use std::convert::TryInto;

/// Alternative query structure compatible with environments that don't
/// support native query's variant enum layout.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Query {
    pub trigger: Trigger,
    pub description: Description,
    pub layout: Layout,
    pub filters: Vec<Filter>,
    pub mappings: Vec<Map>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Trigger {
    pub type_: TriggerType,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum TriggerType {
    Immediate,
    Event,
    Mutation,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Filter {
    pub type_: FilterType,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FilterType {
    AllComponents,
    SomeComponents,
    Name,
    Id,
    VarRange,
    AttrRange,
    Distance,
    Node,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Map {
    pub type_: MapType,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum MapType {
    All,
    SelectAddr,
    Components,
    Var,
    VarName,
    VarType,
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Layout {
    Var,
    Typed,
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Description {
    NativeDescribed,
    Addressed,
    StringAddressed,
    Ordered,
    None,
}

impl TryInto<outcome::Query> for Query {
    type Error = Error;

    fn try_into(self) -> Result<outcome::Query, Self::Error> {
        let mut query = outcome::Query {
            trigger: outcome::query::Trigger::Immediate,
            description: outcome::query::Description::None,
            layout: outcome::query::Layout::Typed,
            filters: vec![],
            mappings: vec![],
        };

        query.trigger = match self.trigger.type_ {
            TriggerType::Immediate => outcome::query::Trigger::Immediate,
            TriggerType::Event => outcome::query::Trigger::Event(
                outcome::arraystring::new_truncate(&self.trigger.args[0]),
            ),
            TriggerType::Mutation => outcome::query::Trigger::Mutation(outcome::Address::from_str(
                &self.trigger.args[0],
            )?),
        };

        query.description = match self.description {
            Description::None => outcome::query::Description::None,
            Description::StringAddressed => outcome::query::Description::StringAddressed,
            Description::Addressed => outcome::query::Description::Addressed,
            Description::NativeDescribed => outcome::query::Description::NativeDescribed,
            Description::Ordered => outcome::query::Description::Ordered,
        };

        query.layout = match self.layout {
            Layout::Typed => outcome::query::Layout::Typed,
            Layout::Var => outcome::query::Layout::Var,
        };

        for filter in self.filters {
            let _filter = match filter.type_ {
                FilterType::AllComponents => outcome::query::Filter::AllComponents(
                    filter
                        .args
                        .into_iter()
                        .map(|s| outcome::arraystring::new_truncate(&s))
                        .collect(),
                ),
                FilterType::SomeComponents => outcome::query::Filter::AllComponents(
                    filter
                        .args
                        .into_iter()
                        .map(|s| outcome::arraystring::new_truncate(&s))
                        .collect(),
                ),
                FilterType::Name => outcome::query::Filter::Name(
                    filter
                        .args
                        .into_iter()
                        .map(|s| outcome::arraystring::new_truncate(&s))
                        .collect(),
                ),
                FilterType::Id => outcome::query::Filter::Id(
                    filter
                        .args
                        .into_iter()
                        .map(|s| s.parse().unwrap())
                        .collect(),
                ),
                FilterType::Distance => outcome::query::Filter::Distance(
                    outcome::Address::from_str(&filter.args[0])?,
                    outcome::Address::from_str(&filter.args[1])?,
                    outcome::Address::from_str(&filter.args[2])?,
                    filter.args[3].parse().unwrap(),
                    filter.args[4].parse().unwrap(),
                    filter.args[5].parse().unwrap(),
                ),
                _ => unimplemented!(),
            };
            query.filters.push(_filter);
        }

        for map in self.mappings {
            let _map = match map.type_ {
                MapType::All => outcome::query::Map::All,
                MapType::Var => outcome::query::Map::Var(
                    outcome::VarType::from_str(&map.args[0])?,
                    outcome::arraystring::new_truncate(&map.args[1]),
                ),
                MapType::VarType => {
                    outcome::query::Map::VarType(outcome::VarType::from_str(&map.args[0])?)
                }
                MapType::VarName => {
                    outcome::query::Map::VarName(outcome::arraystring::new_truncate(&map.args[1]))
                }
                MapType::Components => outcome::query::Map::Components(
                    map.args
                        .iter()
                        .map(|s| outcome::arraystring::new_truncate(s))
                        .collect(),
                ),
                _ => unimplemented!()
                // MapType::SelectAddr => outcome::query::Map::SelectAddr(
                //     map.args
                //         .iter()
                //         .map(|s| outcome::query::GlobAddress::from_str(s).unwrap())
                //         .collect(),
                // ),
            };
            query.mappings.push(_map);
        }

        Ok(query)
    }
}
