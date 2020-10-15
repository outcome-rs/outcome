//! This example runs a simulation using a path to scenario directory.

#![allow(unused)]

extern crate log;
extern crate outcome_core as outcome;
extern crate simplelog;

use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::{thread, time};

use outcome::Sim;
use simplelog::{
    CombinedLogger, Config, ConfigBuilder, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    // find out whether to print development logs
    // by default we only print logs from setup and from runtime
    // print commands
    let be_verbose = args.contains(&"--verbose".to_string());

    // setup the logger
    if !be_verbose {
        // add custom config to apply some filters
        let custom_config = ConfigBuilder::new()
            .add_filter_allow_str("outcome::cmd::print")
            .add_filter_allow_str("outcome::script::preprocessor")
            .add_filter_allow_str("outcome::logic")
            .build();
        TermLogger::init(LevelFilter::max(), custom_config, TerminalMode::Mixed).unwrap();
    } else {
        let mut default_config = Config::default();
        TermLogger::init(LevelFilter::max(), default_config, TerminalMode::Mixed).unwrap();
    }

    // uncomment the following `CombinedLogger` to test logging
    // to file    CombinedLogger::init(
    //        vec![
    //            TermLogger::new(LevelFilter::max(),
    // Config::default()).unwrap(),
    // WriteLogger::new(                LevelFilter::Info,
    // Config::default(),
    // File::create("log").unwrap()),        ]
    //    ).unwrap();

    println!("FEATURE_SYSTEM_INFO = {}", outcome::FEATURE_MACHINE_SYSINFO);

    // handle path to scenario
    let path = match env::args().into_iter().nth(1) {
        Some(p) => p,
        None => {
            println!("Please provide a path to an existing scenario");
            return;
        }
    };
    let current_path = env::current_dir().expect("failed getting current dir path");

    let path_buf = match PathBuf::from(path).canonicalize() {
        Ok(pb) => pb,
        Err(e) => {
            println!(
                "Please provide a valid path to an existing directory: {}",
                e
            );
            return;
        }
    };
    if !path_buf.is_dir() {
        println!(
            "Please provide a valid path to an existing directory: \
            path exists but is not a directory"
        );
        return;
    }
    let path_to_scenario = current_path.join(path_buf);

    // instantiate simulation
    // let (model, mut sim) = match
    // Sim::from_scenario_at(path_to_scenario) {
    let mut sim = match Sim::from_scenario_at_path(path_to_scenario.clone()) {
        Ok(s) => s,
        Err(e) => {
            println!(
                "failed making sim from scenario at path: {}: {}",
                path_to_scenario.to_str().unwrap(),
                e,
            );
            return;
        }
    };
    for n in 0..1000 {
        sim.step();
    }

    // let snap = outcome::to_snapshot(&sim, false).unwrap();
    // drop(sim);
    //
    // let sim = outcome::from_snapshot(snap, false).unwrap();

    // println!("{:?}", model)

    //    let addr =
    // Address::global_from_str("/region/e01001/generic/
    // test_component/string/ string_var").unwrap();

    // processing loop
    //    loop {
    //        sim.process_tick();
    //        thread::sleep(time::Duration::
    // from_millis(50));    }

    // for _ in 0..100 {
    // sim.process_tick(&model);
    //}

    // save snapshot
    //    let snap = sim.to_snapshot(false);
}
