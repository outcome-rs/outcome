//! Defines an interactive interface for the command line.
//!
//! ## Local or remote
//!
//! `SimDriver` enum is used to differentiate between local and remote
//! modes. Local mode will operate directly on a `Sim` struct, while
//! remote mode will use a `Client` connected to an `outcome` server.
//! `Client` interface from the `outcome-net` crate is used.

extern crate toml;

mod compl;
mod local;
mod remote;

#[cfg(feature = "img_print")]
mod img_print;

use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{fs, io, thread};

use anyhow::Result;
use linefeed::inputrc::parse_text;
use linefeed::{Interface, ReadResult};

use outcome::Sim;
use outcome_net::{Client, SocketEvent, SocketEventType};

use self::compl::MainCompleter;
use outcome_net::msg::SpawnEntitiesRequest;
use std::time::Instant;

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

pub struct OnChange {
    pub trigger: Arc<Mutex<bool>>,
    pub action: OnChangeAction,
}

pub enum OnChangeAction {
    Restart,
    UpdateModel,
}

pub struct OnSignal {
    pub trigger: Arc<AtomicBool>,
    pub action: OnSignalAction,
}

pub enum OnSignalAction {
    Custom,
    Quit,
}

pub enum InterfaceType {
    Scenario(String),
    Snapshot(String),
    Remote(Client),
}

/// Variant without the external change trigger.
pub fn start_simple(_type: InterfaceType, config_path: &str) -> Result<()> {
    start(_type, config_path, None, None)
}

// TODO signal handling
/// Entry point for the interactive interface.
///
/// # Introducing runtime changes
///
/// Interactive interface supports updating simulation model or outright
/// restarting the simulation using an external trigger. This is used for
/// supporting a "watch" mode where changes to project files result in
/// triggering actions such as restarting the simulation using newly
/// introduced changes.
pub fn start(
    _type: InterfaceType,
    config_path: &str,
    on_change: Option<OnChange>,
    on_signal: Option<OnSignal>,
) -> Result<()> {
    let path = match &_type {
        InterfaceType::Scenario(path) => Some(path.clone()),
        InterfaceType::Snapshot(path) => Some(path.clone()),
        _ => None,
    };
    let mut sim_driver = match _type {
        InterfaceType::Scenario(path) => SimDriver::Local(Sim::from_scenario_at(&path)?),
        InterfaceType::Snapshot(path) => SimDriver::Local(Sim::from_snapshot_at(&path)?),
        InterfaceType::Remote(client) => SimDriver::Remote(client),
        _ => unimplemented!(),
    };
    let driver_arc = Arc::new(Mutex::new(sim_driver));
    'outer: loop {
        // check remote trigger at the start of the loop, so that we can
        // wait until all fired events get processed
        if let Some(ocm) = &on_change {
            let mut oc = ocm.trigger.lock().unwrap();
            if *oc == true {
                *oc = false;
                continue;
            }
        }

        let interface = Arc::new(Interface::new("interactive")?);

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
        let mut do_run = false;
        let mut do_run_loop = false;
        let mut do_run_freq = None;
        let mut last_time_insta = std::time::Instant::now();
        let mut just_left_loop = false;

        let mut run_loop_count = 0;

        interface.set_report_signal(Signal::Interrupt, true);
        interface.set_report_signal(Signal::Break, true);
        interface.set_report_signal(Signal::Quit, true);

        // start main loop
        loop {
            // check for incoming network events
            let mut driver = driver_arc.lock().unwrap();
            match driver.deref_mut() {
                SimDriver::Remote(ref mut client) => {
                    client.connection.manual_poll();
                    match client.connection.try_recv() {
                        Ok((_, event)) => match event.type_ {
                            SocketEventType::Disconnect => {
                                println!("\nServer terminated the connection...");
                                break 'outer;
                            }
                            _ => (),
                        },
                        _ => (),
                    }
                }
                _ => (),
            }

            if do_run_loop || do_run_freq.is_some() {
                if let Some(hz) = do_run_freq {
                    let target_delta_time = Duration::from_secs(1) / hz;
                    let time_now = std::time::Instant::now();
                    let delta_time = time_now - last_time_insta;
                    if delta_time >= target_delta_time {
                        last_time_insta = time_now;
                        do_run = true;
                    } else {
                        do_run = false;
                    }
                }

                if do_run_loop {
                    if run_loop_count <= 0 {
                        do_run = false;
                        do_run_loop = false;
                        run_loop_count = 0;
                    } else {
                        run_loop_count -= 1;
                        do_run = true;
                    }
                }

                // let mut driver = driver_arc.lock().unwrap();

                if do_run {
                    match driver.deref_mut() {
                        SimDriver::Local(ref mut sim) => {
                            sim.step()?;
                            interface.set_prompt(create_prompt(&mut driver, &config)?.as_str())?;
                        }
                        SimDriver::Remote(client) => {
                            let msg = client.server_step_request(1)?;
                            interface.set_prompt(create_prompt(&mut driver, &config)?.as_str())?;
                        }
                    }

                    if let Some(on_sig) = &on_signal {
                        if on_sig.trigger.load(Ordering::SeqCst) {
                            on_sig.trigger.store(false, Ordering::SeqCst);
                            do_run_freq = None;
                            do_run_loop = false;
                            continue;
                        }
                    }

                    // if let Ok(res) = interface.read_line_step(Some(Duration::from_micros(1))) {
                    //
                    // } else {
                    //     continue;
                    // }
                    let read_result = match interface.read_line_step(Some(Duration::from_micros(1)))
                    {
                        Ok(res) => match res {
                            Some(r) => r,
                            None => continue,
                        },
                        Err(e) => continue,
                    };
                    match read_result {
                        _ => {
                            do_run = false;
                            do_run_freq = None;
                            do_run_loop = false;
                            run_loop_count = 0;
                            continue;
                        }
                    }
                }

                continue;
            }

            if let Some(res) = interface.read_line_step(Some(Duration::from_millis(300)))? {
                match res {
                    ReadResult::Input(line) => {
                        // let mut driver = driver_arc.lock().unwrap();

                        if !line.trim().is_empty() {
                            interface.add_history_unique(line.clone());
                        }

                        let (cmd, args) = split_first_word(&line);
                        match cmd {
                            "run" => {
                                do_run_loop = true;
                                run_loop_count = args.parse::<i32>().unwrap();
                                // interface.set_prompt("")?;
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
                                    SimDriver::Remote(client) => {
                                        client.server_step_request(loop_count)?;
                                    }
                                }
                                interface
                                    .set_prompt(create_prompt(&mut driver, &config)?.as_str())?;
                            }
                            //TODO
                            "runf-until" => {
                                unimplemented!();
                            }
                            "run-freq" => {
                                let hz = args.parse::<usize>().unwrap_or(10);
                                do_run_freq = Some(hz as u32);
                            }
                            "runf-hz" => {
                                let hz = args.parse::<usize>().unwrap_or(10);
                                let mut last = Instant::now();
                                loop {
                                    if Instant::now() - last
                                        < Duration::from_millis(1000 / hz as u64)
                                    {
                                        std::thread::sleep(Duration::from_millis(1));
                                        continue;
                                    }
                                    if let Some(on_sig) = &on_signal {
                                        if on_sig.trigger.load(Ordering::SeqCst) {
                                            on_sig.trigger.store(false, Ordering::SeqCst);
                                            do_run_freq = None;
                                            do_run_loop = false;
                                            break;
                                        }
                                    }
                                    last = Instant::now();
                                    match driver.deref_mut() {
                                        SimDriver::Local(ref mut sim) => {
                                            sim.step()?;
                                            interface.set_prompt(
                                                create_prompt(&mut driver, &config)?.as_str(),
                                            )?;
                                        }
                                        SimDriver::Remote(client) => {
                                            let msg = client.server_step_request(1)?;
                                            interface.set_prompt(
                                                create_prompt(&mut driver, &config)?.as_str(),
                                            )?;
                                        }
                                    }
                                }
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
                            // spawn entity
                            "spawn" => {
                                let split = args.split(" ").collect::<Vec<&str>>();
                                match driver.deref_mut() {
                                    SimDriver::Remote(client) => {
                                        client.connection.send_payload(
                                            SpawnEntitiesRequest {
                                                entity_prefabs: vec![split[0].to_string()],
                                                entity_names: vec![split[1].to_string()],
                                            },
                                            None,
                                        )?;
                                        client.connection.recv_msg()?;
                                    }
                                    SimDriver::Local(sim) => {
                                        sim.spawn_entity(
                                            Some(&outcome::StringId::from(split[0]).unwrap()),
                                            Some(outcome::StringId::from(split[1]).unwrap()),
                                        )?;
                                    }
                                }
                            }
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
                                file.write(&data)?;
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
                                file.write(&data)?;
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
                                SimDriver::Local(sim) => {
                                    local::print_show_grid(&sim, &config, args)
                                }
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
                                SimDriver::Remote(client) => {
                                    remote::process_step(client, &config).unwrap()
                                }
                            },

                            "quit" => break 'outer,

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
                            "model" => match driver.deref_mut() {
                                SimDriver::Local(sim) => {
                                    println!("{:#?}", sim.model);
                                }
                                _ => (),
                            },

                            _ => println!("couldn't recognize input: {:?}", line),
                        }
                        if do_run_freq.is_none() && !do_run_loop {
                            interface.set_prompt(create_prompt(&mut driver, &config)?.as_str())?;
                        }
                        std::mem::drop(driver);
                    }
                    // handle quitting using signals and eof
                    ReadResult::Signal(Signal::Break)
                    | ReadResult::Signal(Signal::Interrupt)
                    | ReadResult::Eof => {
                        interface.cancel_read_line();
                        // if do_run_freq.is_none() && !do_run_loop {
                        break 'outer;
                        // }
                        do_run = false;
                        do_run_freq = None;
                        do_run_loop = false;
                        run_loop_count = 0;
                        interface.cancel_read_line();
                        // interface.set_prompt(create_prompt(&mut driver, &config).as_str())?;
                    }
                    _ => (),
                }
            }

            // check remote trigger
            if let Some(ocm) = &on_change {
                let mut oc = ocm.trigger.lock().unwrap();
                if *oc == true {
                    interface.cancel_read_line();
                    match ocm.action {
                        OnChangeAction::Restart => {
                            warn!("changes to project files detected: restarting...");
                            *oc = false;
                            continue 'outer;
                        }
                        OnChangeAction::UpdateModel => {
                            warn!("changes to project files detected: updating model...");
                            if let SimDriver::Local(sim) = driver_arc.lock().unwrap().deref_mut() {
                                let new_model =
                                    Sim::from_scenario_at(&path.clone().unwrap())?.model;

                                sim.model = new_model;
                            }
                            *oc = false;

                            // let new_model;
                        }
                    }
                }
            }
        }
        if let SimDriver::Remote(client) = &mut driver_arc.lock().unwrap().deref_mut() {
            println!("Disconnecting...");
            client.disconnect();
            thread::sleep(Duration::from_millis(500));
        }
    }
    println!("Leaving interactive mode.");
    Ok(())
}

pub fn create_prompt(driver: &mut SimDriver, cfg: &Config) -> Result<String> {
    match driver {
        SimDriver::Local(sim) => Ok(local::create_prompt(&sim, &cfg)),
        SimDriver::Remote(client) => remote::create_prompt(client, &cfg),
    }
}

static APP_COMMANDS: &[(&str, &str)] = &[
    ("run", "Run a number of simulation ticks (hours), takes in an integer number"),
    ("runf", "Similar to `run` but doesn't listen to interupt signals, `f` stands for \"fast\" \
        (it's faster, but you will have to wait until it's finished processing)"),
    ("run-freq", "Run simulation at a constant pace, using the provided frequency"),
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
