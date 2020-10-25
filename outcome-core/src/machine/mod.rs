//! Implements runtime's logic execution capability.

pub mod cmd;
pub mod error;
pub mod exec;
pub mod script;

pub use error::{Error, ErrorKind, Result};

use arrayvec::ArrayVec;
use smallvec::SmallVec;

use crate::entity::StorageIndex;
use crate::{CompId, EntityId, LongString, ShortString, StringId, VarType};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LocalAddress {
    comp: Option<StringId>,
    var_type: VarType,
    var_id: StringId,
}
impl LocalAddress {
    pub fn from_str(input: &str, location: &LocationInfo) -> Result<Self> {
        let split = input
            .split(crate::address::SEPARATOR_SYMBOL)
            .collect::<Vec<&str>>();
        if split.len() == 3 {
            unimplemented!()
        } else if split.len() == 2 {
            Ok(LocalAddress {
                comp: None,
                var_type: VarType::from_str(split[0]).unwrap(),
                var_id: StringId::from(split[1]).unwrap(),
            })
        } else {
            Err(Error::new(
                *location,
                ErrorKind::InvalidAddress(input.to_string()),
            ))
        }
    }
    pub fn storage_index(&self) -> Option<StorageIndex> {
        match self.comp {
            Some(c) => Some((c, self.var_id)),
            None => None,
        }
    }
    pub fn storage_index_using(&self, comp_id: CompId) -> StorageIndex {
        (comp_id, self.var_id)
    }
    pub fn to_string(&self) -> String {
        unimplemented!()
    }
}

/// Holds instruction location information.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct LocationInfo {
    /// Path to the source file, relative to scenario root
    pub source: Option<LongString>,
    /// Line number as seen in source file
    pub source_line: Option<usize>,
    /// Line number after trimming empty lines, more like an command index
    pub line: Option<usize>,
    /// Unique tag for this location
    pub tag: Option<ShortString>,
}
impl LocationInfo {
    pub fn to_string(&self) -> String {
        format!(
            "Source: {}, Line: {}",
            self.source
                .as_ref()
                .unwrap_or(&LongString::from("unknown").unwrap()),
            self.source_line.as_ref().unwrap_or(&0)
        )
    }
    pub fn empty() -> LocationInfo {
        LocationInfo {
            source: None,
            source_line: None,
            line: None,
            tag: None,
        }
    }
}

/// Command in it's simplest form, ready to be turned into a more concrete
/// representation.
#[derive(Debug, Clone)]
pub struct CommandPrototype {
    /// Command name
    pub name: Option<String>,
    /// Command arguments
    pub arguments: Option<Vec<String>>,
    /// Command output
    pub output: Option<String>,
}

/// Custom collection type used as the main call stack during logic execution.
//TODO determine optimal size, determine whether it should be fixed size or not
pub(crate) type CallStackVec = ArrayVec<[CallInfo; 32]>;

/// Collection type used to hold command results.
pub(crate) type CommandResultVec = SmallVec<[cmd::CommandResult; 2]>;

/// Struct containing basic information about where the execution is
/// taking place.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub ent: EntityId,
    pub comp: CompId,
    pub location: LocationInfo,
}

/// List of "stack" variables available only to the component machine
/// and not visible from the outside.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Registry {
    pub str0: LongString,
    pub int0: i32,
    pub float0: f32,
    pub bool0: bool,
}
impl Registry {
    pub fn new() -> Registry {
        Registry {
            str0: LongString::new(),
            int0: 0,
            float0: 0.0,
            bool0: false,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RegistryTarget {
    Str0,
    Int0,
    Float0,
    Bool0,
}

/// Information about a single call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CallInfo {
    Procedure(ProcedureCallInfo),
    ForIn(ForInCallInfo),
    IfElse(IfElseCallInfo),
}

/// Information about a single procedure call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProcedureCallInfo {
    pub call_line: usize,
    pub start_line: usize,
    pub end_line: usize,
    // pub output_variable: Option<String>,
}

/// Information about a single forin call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ForInCallInfo {
    /// Target that will be iterated over
    pub target: StorageIndex,
    pub target_len: usize,
    /// Variable to update while iterating
    pub variable: LocalAddress,
    // pub variable_type: Option<VarType>,
    /// Current iteration
    pub iteration: usize,
    // meta
    pub start: usize,
    pub end: usize,
}

/// Contains information about a single ifelse call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IfElseCallInfo {
    pub current: usize,
    pub passed: bool,
    pub else_line_index: usize,
    pub meta: IfElseMetaData,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IfElseMetaData {
    pub start: usize,
    pub end: usize,
    pub else_lines: [usize; 10],
}

/// Performs a command search on the provided command prototype list.
///
/// Goal is to find the end, and potentially intermediate parts, of a block.
/// To accomplish this, the function takes lists of defs describing beginning,
/// middle and ending marks of any blocks that it may stumble upon during
/// the search.
///
/// On success returns a tuple of single end part line numer and list of middle
/// part line numbers. If no matching parts are found, and no error .
pub(crate) fn command_search(
    location: &LocationInfo,
    commands: &Vec<CommandPrototype>,
    constraints: (usize, Option<usize>),
    defs: (&Vec<&str>, &Vec<&str>, &Vec<&str>),
    blocks: (&Vec<&str>, &Vec<&str>),
    recurse: bool,
) -> Result<Option<(usize, Vec<usize>)>> {
    if defs.0.is_empty() {
        return Err(Error::new(
            *location,
            ErrorKind::CommandSearchFailed(
                "command search requires begin definitions to be non-empty".to_string(),
            ),
        ));
    }
    if defs.2.is_empty() {
        return Err(Error::new(
            *location,
            ErrorKind::CommandSearchFailed(
                "command search requires ending definitions to be non-empty".to_string(),
            ),
        ));
    }
    let mut locs = (0, Vec::new());
    let mut skip_to = constraints.0;
    let mut block_diff = 0;
    let finish_idx = commands.len();
    for line in constraints.0..finish_idx {
        if line >= skip_to {
            let command = &commands[line];
            match &command.name {
                Some(command) => {
                    if blocks.0.contains(&command.as_str()) {
                        block_diff = block_diff + 1;
                    } else if defs.1.contains(&command.as_str()) {
                        locs.1.push(line);
                    } else if blocks.1.contains(&command.as_str()) && block_diff > 0 {
                        block_diff = block_diff - 1;
                    } else if defs.2.contains(&command.as_str()) {
                        locs.0 = line;
                        return Ok(Some(locs));
                    } else if defs.0.contains(&command.as_str()) {
                        if recurse {
                            match command_search(
                                location,
                                commands,
                                (line + 1, Some(finish_idx)),
                                defs,
                                blocks,
                                recurse,
                            ) {
                                Ok(locs_opt) => match locs_opt {
                                    Some(_locs) => {
                                        skip_to = _locs.0 + 1;
                                        ()
                                    }
                                    None => {
                                        return Err(Error::new(
                                            *location,
                                            ErrorKind::CommandSearchFailed(format!(
                                                "bad nesting: got {} but end not found",
                                                command
                                            )),
                                        ))
                                    }
                                },
                                Err(error) => return Err(error),
                            };
                        } else {
                            return Err(Error::new(
                                *location,
                                ErrorKind::CommandSearchFailed(format!(
                                    "bad nesting: got {}",
                                    command,
                                )),
                            ));
                        }
                    }
                    ()
                }
                None => (),
            }
        }
    }

    Err(Error::new(
        *location,
        ErrorKind::CommandSearchFailed(format!(
            "no end of structure for begin defs: {:?}",
            &defs.0
        )),
    ))
}
