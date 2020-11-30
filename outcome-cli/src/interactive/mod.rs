//! Defines an interactive interface for the command line.
//!
//! ## Local or remote
//!
//! `SimDriver` enum is used to differentiate between local and remote
//! modes. Local mode will operate directly on a `Sim` struct, while
//! remote mode will use a `Client` connected to an `outcome` server.
//! `Client` definition used is the one from the `outcome-net` crate.

#![allow(unused)]

extern crate toml;

mod compl;
mod local;
mod remote;

#[cfg(feature = "img_print")]
mod img_print;

use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{fs, thread};

use anyhow::Result;
use linefeed::inputrc::parse_text;
use linefeed::{Interface, Prompter, ReadResult};
use outcome::{Address, Sim, SimModel};
use outcome_net::Client;

use self::compl::MainCompleter;
use std::io::Write;
use std::ops::{Deref, DerefMut};

// TODO switch to use toml instead of yaml
pub const CONFIG_FILE: &str = "interactive.yaml";

// Adding new cfg var:
// 1. add here
// 2. add to Config struct
// 3. add to Config impl fn get and set
// 4. add to cfg-list command
static CFG_VARS: &[&str] = &["turn_ticks", "show_on", "show_list"];

/// Serializable configuration for the interactive interface.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub turn_ticks: i32,
    #[serde(default)]
    pub show_on: bool,
    #[serde(default)]
    pub show_list: Vec<String>,
    #[serde(default)]
    pub prompt_format: String,
    #[serde(default)]
    pub prompt_vars: Vec<String>,
}
impl Config {
    fn new() -> Config {
        Config {
            turn_ticks: 1,
            show_on: true,
            show_list: Vec::new(),
            prompt_format: "".to_string(),
            prompt_vars: Vec::new(),
        }
    }

    fn new_from_file(path: &str) -> Result<Config, io::Error> {
        let file_str = match fs::read_to_string(path) {
            Ok(f) => f,
            Err(e) => return Err(e),
        };
        match toml::from_str(&file_str) {
            Ok(c) => Ok(c),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }

    fn save_to_file(&self, path: &str) -> Result<(), io::Error> {
        let file_str = toml::to_string(self).unwrap();
        fs::write(path, file_str)
    }

    fn get(&self, name: &str) -> Result<String, io::Error> {
        match name {
            "turn_ticks" => Ok(format!("{}", self.turn_ticks)),
            "show_on" => Ok(format!("{}", self.show_on)),
            "show_list" => Ok(format!("{:?}", self.show_list)),
            "prompt_format" => Ok(self.prompt_format.clone()),
            "prompt_vars" => Ok(format!("{:?}", self.prompt_vars)),
            _ => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Cfg variable doesn't exist",
            )),
        }
    }

    // Set cfg var by name
    fn set(&mut self, name: &str, value: &str) -> Result<(), io::Error> {
        match name {
            // TODO handle unwrap
            "turn_ticks" => {
                self.turn_ticks = match value.parse::<i32>() {
                    Ok(i) => i,
                    Err(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "Failed parsing value",
                        ))
                    }
                }
            }
            "show_on" => {
                self.show_on = match value.parse::<bool>() {
                    Ok(b) => b,
                    Err(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "Failed parsing value",
                        ))
                    }
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Cfg variable doesn't exist",
                ))
            }
        };
        Ok(())
    }

    // Add one address to the "show" list
    fn show_add(&mut self, addr: &str) -> Result<(), io::Error> {
        // TODO check if address is legit
        //
        self.show_list.push(addr.to_string());
        Ok(())
    }
    // Remove one address from the "show" list at index
    fn show_remove(&mut self, index_str: &str) -> Result<(), &str> {
        let index = match index_str.parse::<usize>() {
            Err(e) => return Err("Failed parsing string argument to an integer index"),
            Ok(i) => i,
        };
        self.show_list.remove(index);
        Ok(())
    }
}

pub enum SimDriver {
    Local(Sim),
    Remote(Client),
}

/// Entry point for the interactive interface.
pub fn start(mut sim_driver: SimDriver, config_path: &str) -> Result<()> {
    let interface = Arc::new(Interface::new("interactive")?);
    let driver_arc = Arc::new(Mutex::new(sim_driver));
    //interface.set_completer(Arc::new(MainCompleter));
    interface.set_completer(Arc::new(MainCompleter {
        driver: driver_arc.clone(),
    }));

    // try loading config from file, else get a new default one
    let mut config = match Config::new_from_file(config_path) {
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                println!(
                    "Config file {} doesn't exist, loading default config settings",
                    CONFIG_FILE
                );
                Config::new()
            } else {
                eprintln!("There was a problem parsing the config file, loading default config settings ({})", e);
                Config::new()
            }
        }
        Ok(mut c) => {
            println!("Loading config settings from file (found {})", CONFIG_FILE);
            if c.turn_ticks == 0 {
                c.turn_ticks = 1;
            }
            c
        }
    };

    println!("\nYou're now in interactive mode.");
    println!("See possible commands with \"help\". Exit using \"quit\" or ctrl-d.");

    match &mut driver_arc.lock().unwrap().deref_mut() {
        SimDriver::Local(sim) => {
            interface.set_prompt(local::create_prompt(&sim, &config).as_str())?;
        }
        SimDriver::Remote(client) => {
            interface.set_prompt(remote::create_prompt(client, &config).unwrap().as_str())?
        }
    };

    use linefeed::Signal;
    use std::time::Duration;
    let mut do_run_loop = false;
    let mut run_loop_count = 0;
    interface.set_report_signal(Signal::Interrupt, true);
    interface.set_report_signal(Signal::Break, true);
    interface.set_report_signal(Signal::Quit, true);
    // start main loop
    loop {
        // this is a loop used for the "safer" version of `run` command
        // (basically it listens for a signal while it's processing so
        // you can go back to the prompt at any moment)
        if do_run_loop {
            let mut driver = driver_arc.lock().unwrap();
            // let model = model_arc.lock().unwrap();
            //thread.spawn()
            if run_loop_count > 0 {
                match driver.deref_mut() {
                    SimDriver::Local(ref mut sim) => sim.step().unwrap(),
                    SimDriver::Remote(client) => {
                        client.server_step_request(1)?;
                    }
                }
                run_loop_count -= 1;
                //                let r = match interface.lock_reader().
                let read_result = match interface.read_line_step(Some(Duration::from_micros(10))) {
                    Ok(res) => match res {
                        Some(r) => r,
                        None => continue,
                    },
                    Err(e) => continue,
                };
                //                match interface.read_line_step(Some(Duration::from_millis(1))).unwrap() {
                match read_result {
                    // handle quitting using signals and eof
                    ReadResult::Signal(Signal::Break) => {
                        do_run_loop = false;
                        run_loop_count = 0;
                        //                        interface.cancel_read_line();
                        interface.set_prompt(create_prompt(&driver, &config).as_str())?;
                    }
                    ReadResult::Signal(Signal::Interrupt) => {
                        //                        interface.cancel_read_line();
                        do_run_loop = false;
                        run_loop_count = 0;
                        //                        interface.cancel_read_line();
                        interface.set_prompt(create_prompt(&driver, &config).as_str())?;
                    }
                    ReadResult::Eof => {
                        do_run_loop = false;
                        run_loop_count = 0;
                        //                        interface.cancel_read_line();
                        interface.set_prompt(create_prompt(&driver, &config).as_str())?;
                    }
                    _ => (),
                }
            //                interface.lock_reader();
            } else {
                do_run_loop = false;
                run_loop_count = 0;
                interface.set_prompt(create_prompt(&driver, &config).as_str())?;
            }
            continue;
        }

        match interface.read_line()? {
            ReadResult::Input(line) => {
                let mut driver = driver_arc.lock().unwrap();
                // let model = model_arc.lock().unwrap();
                //                interface.set_prompt(create_prompt(&sim, &config).as_str())?;

                if !line.trim().is_empty() {
                    interface.add_history_unique(line.clone());
                }

                let (cmd, args) = split_first_word(&line);
                match cmd {
                    "run" => {
                        do_run_loop = true;
                        run_loop_count = args.parse::<i32>().unwrap();
                        interface.set_prompt("")?;
                    }
                    //TODO implement using clock int (it was using data string format before)
                    "run-until" => {
                        unimplemented!();
                    }
                    "runf" => {
                        interface.lock_reader();
                        let mut loop_count = args.parse::<u32>().unwrap();
                        match driver.deref_mut() {
                            SimDriver::Local(ref mut sim) => {
                                while loop_count > 0 {
                                    sim.step();
                                    loop_count -= 1;
                                }
                            }
                            SimDriver::Remote(client) => client.server_step_request(loop_count)?,
                        }
                        interface.set_prompt(create_prompt(&driver, &config).as_str())?;
                    }
                    //TODO
                    "runf-until" => {
                        unimplemented!();
                    }
                    "test" => {
                        let secs = args.parse::<usize>().unwrap_or(2);
                        match driver.deref_mut() {
                            SimDriver::Local(ref mut sim) => {
                                super::test::test_sim_struct(&sim);
                                super::test::test_mem();
                                super::test::test_proc(sim, secs);
                            }
                            SimDriver::Remote(client) => {
                                //
                            }
                            _ => unimplemented!(),
                        }
                    }
                    // list variables
                    "ls" => match driver.deref_mut() {
                        SimDriver::Local(sim) => {
                            let map = sim.get_all_as_strings();
                            for (k, v) in map {
                                let s = format!("{}: {}", k, v);
                                if s.contains(args) || args == "" {
                                    println!("{}", s);
                                }
                            }
                        }
                        SimDriver::Remote(client) => {
                            let data = client.get_vars();
                            // TODO proper formatting
                            println!("{:?}", data);
                        }
                    },
                    // Write an uncompressed snapshot to disk.
                    "snap" => {
                        if args.contains(" ") {
                            println!("Snapshot file path cannot contain spaces.");
                            continue;
                        }
                        let target_path = PathBuf::from(args);
                        let mut file = match fs::File::create(target_path) {
                            Ok(f) => f,
                            Err(e) => {
                                println!("{}", e);
                                continue;
                            }
                        };
                        let data = match driver.deref_mut() {
                            SimDriver::Local(sim) => match sim.to_snapshot(false) {
                                Ok(d) => d,
                                Err(e) => {
                                    println!("{}", e);
                                    continue;
                                }
                            },
                            _ => unimplemented!(),
                        };
                        let data = file.write(&data);
                    }
                    // Write a compressed snapshot to disk.
                    "snapc" => {
                        if args.contains(" ") {
                            println!("Snapshot file path cannot contain spaces.");
                            continue;
                        }
                        let target_path = PathBuf::from(args);
                        let mut file = match fs::File::create(target_path) {
                            Ok(f) => f,
                            Err(e) => {
                                println!("{}", e);
                                continue;
                            }
                        };
                        let data = match driver.deref_mut() {
                            SimDriver::Local(sim) => match sim.to_snapshot(true) {
                                Ok(d) => d,
                                Err(e) => {
                                    println!("{}", e);
                                    continue;
                                }
                            },
                            _ => unimplemented!(),
                        };
                        file.write(&data);
                    }

                    "help" => {
                        println!("available commands:");
                        println!();
                        for &(cmd, help) in APP_COMMANDS {
                            println!("  {:15} - {}", cmd, help);
                        }
                        println!();
                    }
                    "cfg-list" => {
                        println!(
                            "\n\
turn_ticks              {turn_ticks}
show_on                 {show_on}
show_list               {show_list}
",
                            turn_ticks = config.turn_ticks,
                            show_on = config.show_on,
                            show_list = format!("{:?}", config.show_list),
                        );
                    }
                    "cfg-get" => match config.get(args) {
                        Err(e) => println!("Error: {} doesn't exist", args),
                        Ok(c) => println!("{}: {}", args, c),
                    },
                    "cfg" => {
                        let (var, val) = split_first_word(&args);
                        match config.set(var, val) {
                            Err(e) => println!("Error: couldn't set {} to {}", var, val),
                            Ok(()) => println!("Setting {} to {}", var, val),
                        }
                    }
                    "cfg-save" => {
                        println!("Exporting current configuration to file {}", CONFIG_FILE);
                        config.save_to_file(CONFIG_FILE).unwrap();
                    }
                    "cfg-reload" => {
                        config = match Config::new_from_file(CONFIG_FILE) {
                            Err(e) => {
                                if e.kind() == io::ErrorKind::NotFound {
                                    println!(
                                        "Config file {} doesn't exist, loading default config settings",
                                        CONFIG_FILE
                                    );
                                    Config::new()
                                } else {
                                    eprintln!("There was a problem parsing the config file, loading default config settings. Details: {}", e);
                                    Config::new()
                                }
                            }
                            Ok(c) => {
                                println!(
                                    "Successfully reloaded configuration settings (found {})",
                                    CONFIG_FILE
                                );
                                c
                            }
                        };
                    }

                    #[cfg(feature = "outcome_core/grids")]
                    "show-grid" => match driver.deref() {
                        SimDriver::Local(sim) => local::print_show_grid(&sim, &config, args),
                        _ => unimplemented!(),
                    },

                    "show" => match driver.deref() {
                        SimDriver::Local(sim) => local::print_show(&sim, &config),
                        _ => unimplemented!(),
                    },

                    "show-toggle" => {
                        if config.show_on {
                            config.show_on = false
                        } else {
                            config.show_on = true
                        };
                    }

                    "show-add" => {
                        // TODO handle unwrap
                        config.show_add(args).unwrap();
                    }

                    "show-remove" => {
                        // TODO handle unwrap
                        config.show_remove(args).unwrap();
                    }

                    "show-clear" => {
                        config.show_list.clear();
                    }

                    "history" => {
                        let w = interface.lock_writer_erase()?;

                        for (i, entry) in w.history().enumerate() {
                            println!("{}: {}", i, entry);
                        }
                    }

                    "" => match driver.deref_mut() {
                        SimDriver::Local(ref mut sim) => local::process_step(sim, &config),
                        SimDriver::Remote(client) => remote::process_step(client, &config).unwrap(),
                    },

                    "quit" => break,

                    // hidden commands
                    "interface-set" => {
                        let d = parse_text("<input>", &line);
                        interface.evaluate_directives(d);
                    }
                    "interface-get" => {
                        if let Some(var) = interface.get_variable(args) {
                            println!("{} = {}", args, var);
                        } else {
                            println!("no variable named `{}`", args);
                        }
                    }
                    "interface-list" => {
                        for (name, var) in interface.lock_reader().variables() {
                            println!("{:30} = {}", name, var);
                        }
                    }
                    "spawn" => {
                        let num = args.parse::<usize>().unwrap_or(100);
                        match driver.deref_mut() {
                            SimDriver::Local(sim) => {
                                for _ in 0..num {
                                    sim.spawn_entity(None, None);
                                }
                            }
                            _ => (),
                        }
                    }

                    _ => println!("couldn't recognize input: {:?}", line),
                }
                if !do_run_loop {
                    interface.set_prompt(create_prompt(&driver, &config).as_str())?;
                }
                std::mem::drop(driver);
            }
            // handle quitting using signals and eof
            ReadResult::Signal(Signal::Break)
            | ReadResult::Signal(Signal::Interrupt)
            | ReadResult::Eof => {
                interface.cancel_read_line();
                break;
            }
            _ => (),
        }
    }

    if let SimDriver::Remote(client) = &mut driver_arc.lock().unwrap().deref_mut() {
        println!("Disconnecting...");
        client.disconnect();
        thread::sleep(Duration::from_millis(500));
    }

    println!("Leaving interactive mode.");

    Ok(())
}

pub fn create_prompt(driver: &SimDriver, cfg: &Config) -> String {
    match driver {
        SimDriver::Local(sim) => local::create_prompt(&sim, &cfg),
        SimDriver::Remote(client) => remote::create_prompt(&client, &cfg).unwrap(),
    }
}

static APP_COMMANDS: &[(&str, &str)] = &[
    ("run", "Run a number of simulation ticks (hours), takes in an integer number"),
    ("runf", "Similar to `run` but doesn't listen to interupt signals, `f` stands for \"fast\" \
        (it's faster, but you will have to wait until it's finished processing)"),
    ("test", "Run quick mem+proc test. Takes in a number of secs to run the average processing speed test (default=2)"),
    ("ls", "List simple variables (no lists or grids). Takes in a string argument, returns only vars that contain that string in their address"),
    ("snap", "Export current sim state to snapshot file. Takes a path to target file, relative to where endgame is running."),
    ("snapc", "Same as snap but applies compression"),
    ("cfg", "Set config variable"),
    ("cfg-get", "Print the value of one config variable"),
    ("cfg-list", "Get a list of all config variables"),
    ("cfg-save", "Save current configuration to file"),
    ("cfg-reload", "Reload current configuration from file"),
    ("show", "Print selected simulation data"),
    ("show-add", "Add to the list of simulation data to be shown"),
    (
        "show-remove",
        "Remove from the list of simulation data to be shown (by index, starting at 0)",
    ),
    (
        "show-clear",
        "Clear the list of simulation data to be shown",
    ),
    ("show-toggle", "Toggle automatic printing after each turn"),
    ("history", "Print input history"),
    ("help", "Show available commands"),
    (
        "quit",
        "Quit (NOTE: all unsaved data will be lost)",
    ),
];

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim();

    match s.find(|ch: char| ch.is_whitespace()) {
        Some(pos) => (&s[..pos], s[pos..].trim_start()),
        None => (s, ""),
    }
}
