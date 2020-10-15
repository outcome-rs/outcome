//! This example shows how to quickly put together a simple
//! interactive command line interpreter using the building
//! blocks provided by the library.

extern crate outcome_core;
extern crate simplelog;

use std::env;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use simplelog::{Config, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};

use outcome_core::machine::{cmd, exec, script, LocationInfo};
use outcome_core::Sim;

fn main() {
    // initialize the simulation instance object
    let mut sim = match init_sim() {
        Ok(s) => s,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let (ent_uid, ent) = sim.entities.iter().nth(1).unwrap();
    let comp_uid = *ent.components.map.iter().next().unwrap().0;
    let ent_uid = *ent_uid;
    let ent_index = sim
        .entities_idx
        .iter()
        .find(|(_, euid)| euid == &&ent_uid)
        .map(|(str_idx, _)| *str_idx)
        .unwrap();

    let mut input_amalg = String::new();
    'outer: loop {
        print!(">");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("failed to read from stdin");

        if input.trim().ends_with("\\") {
            input_amalg.push_str(&input);
            continue;
        } else if !input_amalg.is_empty() {
            input_amalg.push_str(&input);
            input = input_amalg.clone();
            input_amalg = String::new();
        }
        let instructions = match script::parser::parse_lines(&input, "") {
            Ok(i) => i,
            Err(e) => {
                println!("{:?}", e);
                continue;
            }
        };
        let mut cmd_protos = Vec::new();
        for instr in instructions {
            let cmd_proto = match instr.kind {
                script::InstructionKind::Command(cp) => cp,
                _ => {
                    println!("not a command");
                    continue 'outer;
                }
            };
            cmd_protos.push(cmd_proto);
        }
        let mut commands = Vec::new();
        for (n, cmd_proto) in cmd_protos.iter().enumerate() {
            let mut location = LocationInfo::empty();
            location.line = Some(n);
            // execute the input as command
            let command = match cmd::Command::from_prototype(&cmd_proto, &location, &cmd_protos) {
                Ok(c) => c,
                Err(e) => {
                    println!("{:?}", e);
                    continue 'outer;
                }
            };
            commands.push(command);
        }
        exec::execute(&commands, &ent_index, &comp_uid, &mut sim, None, None).unwrap();

        // print!("{}", input);
    }
}

fn init_sim() -> Result<Sim, String> {
    let args: Vec<String> = env::args().collect();
    // find out whether to print development logs
    // by default we only print logs from setup and from runtime
    // print commands
    let do_devlog = args.contains(&"--verbose".to_string());

    // setup the logger
    if !do_devlog {
        // add custom config to apply some filters
        let custom_config = ConfigBuilder::new()
            .add_filter_allow_str("outcome::cmd::print")
            .add_filter_allow_str("outcome::script::preprocessor")
            .build();
        TermLogger::init(LevelFilter::max(), custom_config, TerminalMode::Mixed).unwrap();
    } else {
        let default_config = Config::default();
        TermLogger::init(LevelFilter::max(), default_config, TerminalMode::Mixed).unwrap();
    }

    // handle path to scenario
    let path = match env::args().into_iter().nth(1) {
        Some(p) => p,
        None => {
            return Err("Please provide a path to an existing scenario".to_string());
        }
    };
    let current_path = env::current_dir().expect("failed getting current dir path");
    let path_buf = PathBuf::from(path).canonicalize().unwrap();
    if !path_buf.exists() || !path_buf.is_dir() {
        return Err("Please provide a path to an existing scenario".to_string());
    }
    let path_to_scenario = current_path.join(path_buf);

    // instantiate simulation
    // let (model, mut sim) = match
    // Sim::from_scenario_at(path_to_scenario) {
    match Sim::from_scenario_at_path(path_to_scenario.clone()) {
        Ok(s) => Ok(s),
        Err(e) => {
            return Err(format!(
                "failed making sim from scenario at path: {}: {}",
                path_to_scenario.to_str().unwrap(),
                e,
            ));
        }
    }
}
