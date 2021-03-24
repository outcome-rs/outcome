//! Command line program for working with `outcome` simulations.

#![allow(unused)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

extern crate anyhow;
extern crate clap;
extern crate colored;
extern crate linefeed;

extern crate outcome_core as outcome;

pub mod cli;
pub mod init;
pub mod interactive;
pub mod test;
mod util;

use colored::*;

fn main() {
    // Run the program based on user input
    match cli::start(cli::app_matches()) {
        Ok(_) => (),
        Err(e) => {
            println!("{}{}", "error: ".red(), e);
            if e.root_cause().to_string() != e.to_string() {
                println!("Caused by:\n{}", e.root_cause())
            }
        }
    }
}
