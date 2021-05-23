use std::iter::FromIterator;

use crate::address::Address;
use crate::entity::{Entity, Storage};
use crate::model::{ComponentModel, LogicModel, SimModel, VarModel};
use crate::{
    string, CompName, EntityId, EntityName, LongString, ShortString, Sim, StringId, VarType,
};

use crate::distr::SimCentral;
use crate::machine::cmd::register::{RegisterComponent, RegisterEntityPrefab};
use crate::machine::cmd::{
    CentralRemoteCommand, Command, CommandPrototype, CommandResult, LocationInfo,
};
use crate::machine::error::{Error, ErrorKind, Result};
use crate::machine::script::parse_script_at;
use crate::machine::ComponentCallInfo;
use crate::machine::{
    CallInfo, CallStackVec, IfElseCallInfo, IfElseMetaData, ProcedureCallInfo, Registry,
};

pub const COMMAND_NAMES: [&'static str; 1] = ["component"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBlock {
    pub name: StringId,
    pub source_comp: StringId,
    pub source_file: LongString,
    pub start_line: usize,
    pub end_line: usize,
    pub output_variable: Option<Address>,
}

impl ComponentBlock {
    pub fn new(
        args: Vec<String>,
        location: &LocationInfo,
        commands: &Vec<CommandPrototype>,
    ) -> Result<Command> {
        trace!("making new comp block");

        let line = location.line.unwrap();

        // start names
        let mut start_names = Vec::new();
        start_names.extend(&COMMAND_NAMES);
        // middle names
        let mut middle_names = Vec::new();
        // end names
        let mut end_names = Vec::new();
        end_names.extend(&super::end::COMMAND_NAMES);
        // other block starting names
        let mut start_blocks = Vec::new();
        start_blocks.extend(&super::ifelse::IF_COMMAND_NAMES);
        start_blocks.extend(&super::ifelse::ELSE_COMMAND_NAMES);
        start_blocks.extend(&super::forin::COMMAND_NAMES);
        start_blocks.extend(&super::procedure::COMMAND_NAMES);
        start_blocks.extend(&super::state::COMMAND_NAMES);
        // other block ending names
        let mut end_blocks = Vec::new();
        end_blocks.extend(&super::end::COMMAND_NAMES);

        let positions_options = match crate::machine::command_search(
            location,
            &commands,
            (line + 1, None),
            (&start_names, &middle_names, &end_names),
            (&start_blocks, &end_blocks),
            true,
        ) {
            Ok(po) => po,
            Err(e) => {
                return Err(Error::new(
                    location.clone(),
                    ErrorKind::InvalidCommandBody(e.to_string()),
                ))
            }
        };

        match positions_options {
            Some(positions) => Ok(Command::Component(ComponentBlock {
                name: string::new_truncate(&args[0]),
                source_comp: location.comp_name.clone().unwrap(),
                source_file: location.source.unwrap(),
                start_line: line + 1,
                end_line: positions.0,
                output_variable: None,
            })),
            // {
            //     Ok(Command::Register(Register::Component(RegComponent {
            //         name: StringId::from_truncate(&args[0]),
            //         trigger_events: vec![],
            //     })))
            // }
            None => Err(Error::new(
                location.clone(),
                ErrorKind::InvalidCommandBody("end of component block not found".to_string()),
            )),
        }
    }
    pub fn execute_loc(
        &self,
        call_stack: &mut CallStackVec,
        ent_uid: &EntityId,
        comp_name: &CompName,
        line: usize,
    ) -> Vec<CommandResult> {
        trace!("executing component block: {:?}", self);

        let mut new_self = self.clone();

        call_stack.push(CallInfo::Component(ComponentCallInfo {
            name: new_self.name.clone(),
            start_line: new_self.start_line,
            end_line: new_self.end_line,
        }));

        let mut out_vec = Vec::new();
        out_vec.push(CommandResult::ExecCentralExt(
            CentralRemoteCommand::Component(new_self),
        ));
        // out_vec.push(CommandResult::ExecCentralExt(
        //     CentralRemoteCommand::Component(RegisterComponent {
        //         name: arraystring::new_truncate(&new_self.name),
        //         trigger_events: vec![],
        //         source_comp: self.source_comp,
        //         start_line: self.start_line,
        //         end_line: self.end_line,
        //     }),
        // ));
        out_vec.push(CommandResult::Continue);
        // out_vec.push(CommandResult::JumpToLine(self.end_line + 1));
        out_vec
    }
    pub fn execute_ext(&self, sim: &mut Sim) -> Result<()> {
        trace!("registering component");

        let comp_model = sim.model.get_component(&self.source_comp).unwrap();

        let component = ComponentModel {
            name: self.name.clone(),
            //start_state: arraystring::new_unchecked("start"),
            // triggers: vec![StringId::from_unchecked("step")],
            // logic: LogicModel {
            //     commands: comp_model.logic.commands.clone(),
            //     cmd_location_map: comp_model.logic.cmd_location_map.clone(),
            //     ..LogicModel::default()
            // },
            logic: comp_model.logic.get_subset(self.start_line, self.end_line),
            ..ComponentModel::default()
        };

        debug!("adding new component to model: {:?}", component);

        // overwrite existing components with the same name by default
        if let Some(n) = sim
            .model
            .components
            .iter()
            .enumerate()
            .find(|(_, c)| c.name == component.name)
            .map(|(n, _)| n)
        {
            sim.model.components.remove(n);
        }
        sim.model.components.push(component);
        // trace!("{:?}", self);
        Ok(())
    }

    pub fn execute_ext_distr(&self, central: &mut SimCentral) -> Result<()> {
        warn!("component block");
        let comp_model = central.model.get_component(&self.source_comp).unwrap();

        let component = ComponentModel {
            name: self.name.clone(),
            //start_state: arraystring::new_unchecked("start"),
            // triggers: vec![StringId::from_unchecked("step")],
            // logic: LogicModel {
            //     commands: comp_model.logic.commands.clone(),
            //     cmd_location_map: comp_model.logic.cmd_location_map.clone(),
            //     ..LogicModel::default()
            // },
            logic: comp_model.logic.get_subset(self.start_line, self.end_line),
            ..ComponentModel::default()
        };

        trace!(
            "execute_ext_distr: adding new component to model: {:?}",
            component
        );

        // overwrite existing components with the same name by default
        if let Some(n) = central
            .model
            .components
            .iter()
            .enumerate()
            .find(|(_, c)| c.name == component.name)
            .map(|(n, _)| n)
        {
            central.model.components.remove(n);
        }
        central.model.components.push(component);
        // trace!("{:?}", self);
        Ok(())
    }
}
