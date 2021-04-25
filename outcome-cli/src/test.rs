//! Test the system at runtime.

#![allow(unused)]

#[cfg(feature = "psutil")]
extern crate psutil;
#[cfg(feature = "psutil")]
use self::psutil::*;

use std::path::PathBuf;
use std::time;

use clap::ArgMatches;
use outcome::{Sim, SimModel};

//TODO rewrite this func
pub fn scenario(path: PathBuf, mem: bool, proc: bool) {
    info!(
        "starting test using scenario at path: {}",
        path.to_string_lossy()
    );
    let sim = Sim::from_scenario_at_path(path).unwrap();
    test_sim_struct(&sim);
    test(sim, mem, proc);
}

fn test(mut sim: Sim, mem: bool, proc: bool) {
    if mem {
        test_mem();
    }
    if proc {
        test_proc(&mut sim, 5);
    }
}

pub fn test_sim_struct(sim: &Sim) {
    let model_entity_count = sim.model.entities.len();
    let model_component_count = sim.model.components.len();
    let total_entity_count = sim.get_entities().len();
    let mut total_component_count = 0;
    // for ent in sim.get_entities() {
    //     total_component_count += ent.components.map.len();
    // }
    let mut total_str_variables_count = 0;
    let mut total_int_variables_count = 0;
    let mut total_float_variables_count = 0;
    let mut total_bool_variables_count = 0;
    let mut total_str_list_variables_count = 0;
    let mut total_int_list_variables_count = 0;
    let mut total_float_list_variables_count = 0;
    let mut total_bool_list_variables_count = 0;
    let mut total_str_grid_variables_count = 0;
    let mut total_int_grid_variables_count = 0;
    let mut total_float_grid_variables_count = 0;
    let mut total_bool_grid_variables_count = 0;
    for ent in sim.get_entities() {
        for (si, var) in ent.storage.map.iter() {
            match var.get_type() {
                outcome::VarType::String => total_str_variables_count += 1,
                outcome::VarType::Int => total_int_variables_count += 1,
                outcome::VarType::Float => total_float_variables_count += 1,
                outcome::VarType::Bool => total_bool_variables_count += 1,
                _ => (),
            }
        }
        // total_str_list_variables_count += ent.storage.get_all_str_list().len();
        // total_int_list_variables_count += ent.storage.get_all_int_list().len();
        // total_float_list_variables_count += ent.storage.get_all_float_list().len();
        // total_bool_list_variables_count += ent.storage.get_all_bool_list().len();
        // total_str_grid_variables_count += ent.storage.get_all_str_grid().len();
        // total_int_grid_variables_count += ent.storage.get_all_int_grid().len();
        // total_float_grid_variables_count += ent.storage.get_all_float_grid().len();
        // total_bool_grid_variables_count += ent.storage.get_all_bool_grid().len();
    }
    let mut total_simple_variables_count = total_str_variables_count
        + total_int_variables_count
        + total_float_variables_count
        + total_bool_variables_count;
    let mut total_list_variables_count = total_str_list_variables_count
        + total_int_list_variables_count
        + total_float_list_variables_count
        + total_bool_list_variables_count;
    let mut total_grid_variables_count = total_str_grid_variables_count
        + total_int_grid_variables_count
        + total_float_grid_variables_count
        + total_bool_grid_variables_count;

    let total_variables_count =
        total_simple_variables_count + total_list_variables_count + total_grid_variables_count;
    println!(
        "\n\
         Current sim state (step: {})\n\
         -----------------------------------------\n\
         Model entity count: {}\n\
         Model component count: {}\n\
         \
         Spawned entity count: {}\n\
         Spawned component count: {}\n\
         \
         Stored variables count (all): {}\n\
         str={}, int={}, float={}, bool={}\n\
         str_list={}, int_list={}, float_list={}, bool_list={}\n\
         str_grid={}, int_grid={}, float_grid={}, bool_grid={}\n",
        sim.get_clock(),
        model_entity_count,
        model_component_count,
        total_entity_count,
        total_component_count,
        total_variables_count,
        total_str_variables_count,
        total_int_variables_count,
        total_float_variables_count,
        total_bool_variables_count,
        total_str_list_variables_count,
        total_int_list_variables_count,
        total_float_list_variables_count,
        total_bool_list_variables_count,
        total_str_grid_variables_count,
        total_int_grid_variables_count,
        total_float_grid_variables_count,
        total_bool_grid_variables_count,
    );
}

/// Shows the memory information of the process with a fully loaded sim instance.
#[cfg(feature = "psutil")]
pub fn test_mem() {
    let mem = psutil::process::Process::current()
        .unwrap()
        .memory_info()
        .unwrap();
    println!(
        "\n\
    Current memory state\n\
    -----------------------------------------\n\
    Resident memory: {} MB\n\
    Virtual memory: {} MB\n",
        mem.rss() as f32 / 1000000.0,
        mem.vms() as f32 / 1000000.0,
    );
}
#[cfg(not(feature = "psutil"))]
/// Shows the memory information of the process with a fully loaded sim instance.
pub fn test_mem() {
    println!("feature not available")
}
/// Computes and shows average number of ticks per second with
/// the given sim instance on the current machine.
pub fn test_proc(mut sim: &mut Sim, secs: usize) {
    println!(
        "\
         Average processing speed\n\
         -----------------------------------------"
    );
    let mut ticks_avg: f32 = 0.0;
    let mut ticks_vec = Vec::new();
    let mut loop_count = 0;
    loop {
        let mut counter = 0;
        let loop_start_time = time::Instant::now();
        let mut duration = time::Duration::new(0, 0);
        loop {
            counter += 1;

            //        let tick_start = time::Instant::now();

            sim.step();
            //            sim_instance.process_tick_single_thread();

            //        let tick_end = time::Instant::now();
            //        let dur = tick_end.duration_since(tick_start);
            //        println!("Time to process 1 tick: {}",
            //                 dur.as_secs() as f64
            //                     + dur.subsec_nanos() as f64 * 1e-9);
            duration = time::Instant::now().duration_since(loop_start_time);
            if duration.as_nanos() >= 1000000000 {
                break;
            }
        }
        println!(
            "{} ticks in {} second(s)",
            counter,
            format!("{}.{}", duration.as_secs(), duration.subsec_millis())
        );
        loop_count += 1;
        ticks_vec.push(counter);
        // count the average
        let mut sum = 0;
        for t in &ticks_vec {
            sum += *t;
        }
        ticks_avg = sum as f32 / ticks_vec.len() as f32;

        if loop_count == secs {
            break;
        }
    }
    //    println!("-----------------------------------------");
    println!("Average ticks per second: {}\n", ticks_avg);
}
