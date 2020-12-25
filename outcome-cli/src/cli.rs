//! Application definition.

#![allow(dead_code)]
#![allow(unused)]

extern crate simplelog;

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::{env, thread};

use anyhow::{Error, Result};
use clap::{App, AppSettings, Arg, ArgGroup, ArgMatches, SubCommand};
use outcome::Sim;
use outcome_net::{Coord, Server, ServerSettings, SimConnection, Worker};

use self::simplelog::{Level, LevelPadding};
use crate::init;
use crate::interactive;
use crate::test;
use core::mem;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

pub fn app<'a, 'b>() -> App<'a, 'b> {
    let mut app = App::new("outcome-cli")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(VERSION)
        .author(AUTHORS)
        .about("Create, run and analyze outcome simulations from the command line.\n\
                Learn more at https://theoutcomeproject.com")
        .arg(Arg::with_name("verbosity")
            .long("verbosity")
            .short("v")
            .takes_value(true)
            .default_value("info")
            .value_name("verb")
            .global(true)
            .help("Set the verbosity of the log output"))
        //init subcommand
        .subcommand(SubCommand::with_name("new")
            .setting(AppSettings::DisableHelpSubcommand)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .display_order(10)
            .about("Create new scenario, module or experiment")
            .subcommand(SubCommand::with_name("module")
                .about("Initialize new module")
                .arg(Arg::with_name("path")
                    .required(true)
                    .value_name("path"))
                .arg(Arg::with_name("name")
                    .help("Set the name for the new module (defaults to directory name)")
                    .short("n")
                    .long("name"))
                .arg(Arg::with_name("template")
                    .possible_values(&["barebones", "commented", "elaborate", "tutorial"])
                    .takes_value(true)
                    .default_value("commented")
                    .help("Init with a template")
                    .long("template")
                    .short("t")))
            .subcommand(SubCommand::with_name("scenario")
                .about("Initialize new scenario")
                .arg(Arg::with_name("path")
                    .required(true)
                    .value_name("path"))
                .arg(Arg::with_name("name")
                    .help("Set the name for the new scenario (defaults to directory name)")
                    .short("n")
                    .long("name"))
                .arg(Arg::with_name("template")
                    .possible_values(&["commented", "tutorial"])
                    .takes_value(true)
                    .default_value("commented")
                    .help("Init with a template")
                    .long("template")
                    .short("t")))
            .subcommand(SubCommand::with_name("proof")
                .about("Initialize new proof")
                .arg(Arg::with_name("path")
                    .required(true)
                    .value_name("path"))
                .arg(Arg::with_name("name")
                    .help("Set the name for the new proof (defaults to directory name)")
                    .short("n")
                    .long("name"))
                .arg(Arg::with_name("template")
                    .possible_values(&["commented"])
                    .takes_value(true)
                    .default_value("commented")
                    .help("Init with a template")
                    .long("template")
                    .short("t")
                )
            )
        )

        // test subcommand
        .subcommand(SubCommand::with_name("test")
            .display_order(12)
            .about("Test for memory requirements and average processing speed")
            .arg(Arg::with_name("path")
                .value_name("path")
                .required(true)
                .default_value("./")
                .help("Path to the scenario manifest"))
            .arg(Arg::with_name("memory")
                .display_order(0)
                .help("Test memory requirements")
                .short("m"))
            .arg(Arg::with_name("processing")
                .display_order(1)
                .help("Test average processing speed")
                .short("p"))
        )

        // run subcommand
        .subcommand(SubCommand::with_name("run")
//            .setting(AppSettings::DisableHelpSubcommand)
            .display_order(20)
            .about("Run simulation from scenario, snapshot or experiment")
            // Note: If there are no arguments supplied \
            //     the program will look for a scenario, snapshot or proof \
            //     (in that order) in the current working directory.")
            .arg(Arg::with_name("path")
                .value_name("path"))
            .arg(Arg::with_name("interactive")
                .default_value("true")
                .short("i")
                .long("interactive"))
            .arg(Arg::with_name("iconfig")
                .takes_value(true)
                .value_name("path")
                .default_value("./interactive.yaml")
                .long("iconfig")
                .help("specify path to interactive config file"))

            .subcommand(SubCommand::with_name("scenario")
                .about("Run simulation from a scenario")
                .arg(Arg::with_name("interactive")
                    .default_value("true")
                    .short("i")
                    .long("interactive"))
                .arg(Arg::with_name("path")
                    .value_name("scenario-path"))
                .arg(Arg::with_name("iconfig")
                    .takes_value(true)
                    .value_name("path")
                    .default_value("./interactive.yaml")
                    .long("iconfig")
                    .help("specify path to interactive config file"))
            )
            .subcommand(SubCommand::with_name("snapshot")
                .about("Run simulation from a snapshot")
                .arg(Arg::with_name("interactive")
                    .default_value("true")
                    .short("i")
                    .long("interactive"))
                .arg(Arg::with_name("path")
                    .value_name("snapshot-path"))
                .arg(Arg::with_name("iconfig")
                    .takes_value(true)
                    .value_name("path")
                    .default_value("./interactive.yaml")
                    .long("iconfig")
                    .help("specify path to interactive config file"))
            )
        )


        // server subcommand
        .subcommand(SubCommand::with_name("server")
            .display_order(21)
            .about("Start a server")
            .long_about("Start a server\n\n\
            NOTE: data sent between client and server is not encrypted, connection \n\
            is not secure! Passwords are used, but they are more of a convenience than a \n\
            serious security measure.")
            .arg(Arg::with_name("scenario-path")
                .display_order(1)
                .required(true)
                .long("scenario")
                .value_name("scenario-path"))
            .arg(Arg::with_name("ip-address")
                .display_order(2)
                .required(false)
                .long("ip")
                .help("Set the ip address of the server, together with port (e.g. 127.0.0.1:9123)")
                .default_value("127.0.0.1:9123")
                .value_name("ip-address"))
            .arg(Arg::with_name("password")
                .display_order(3)
                .takes_value(true)
                .long("password")
                .short("p")
                .help("Set the password used for new client authentication"))
            .arg(Arg::with_name("keep-alive")
                .display_order(4)
                .long("keep-alive")
                .short("k")
                .takes_value(true)
                .value_name("seconds")
                .help("Server process will quit if it doesn't receive any messages within \
                the specified time frame (seconds)"))
            .arg(Arg::with_name("no-delay")
                .display_order(5)
                .long("no-delay")
                .short("n")
                .help("Set to true to disable Nagle's algorithm and decrease overall latency \
                for messages."))
            .arg(Arg::with_name("use-compression")
                .display_order(6)
                .long("use-compression")
                .short("c")
                .help("Flag specifying whether lz4 compression should be used to compress \
                all messages. With compression on all incoming messages have to be compressed"))
            .arg(Arg::with_name("cluster")
                .display_order(100)
                .takes_value(true)
                .value_name("coordinator-ip")
                .long("cluster")
                .help("Run the sim in cluster mode, using multiple worker nodes instead of a single machine."))
            .arg(Arg::with_name("workers")
                .display_order(101)
                .takes_value(true)
                .value_name("worker-ip-addresses")
                .long("workers")
                .help("List of cluster workers' addresses. Only applicable if `--cluster` option is also present."))
        )

        // client subcommand
        .subcommand(SubCommand::with_name("client")
            .display_order(22)
            .about("Start an interactive client session")
            .long_about("Start an interactive client session.\n\n\
            Establishes a client connection to a server at specified address, \n\
            and provides a REPL-style interface for interacting with that \n\
            server. \n\n\
            NOTE: Data sent between client and server is not encrypted, \n\
            connection is not secure! Passwords are used, but they are more of \n\
            a convenience than a serious security measure.")
            .arg(Arg::with_name("server-addr")
                .required(true)
                .long("server")
                .short("s")
                .value_name("address")
                .help("Address of the server, together with port (e.g. 127.0.0.1:9999)"))
            .arg(Arg::with_name("client-addr")
                .required(false)
                .long("addr")
                .short("a")
                .value_name("address")
                .long_help("Long Help")
                .hidden_long_help(true)
                .help("Address of this client, together with port (e.g. 127.0.0.1:9999)"))
                // .default_value("127.0.0.1:3123")
            .arg(Arg::with_name("password")
                .long("password")
                .takes_value(true)
                .short("p")
                .help("Password used for authentication"))
            .arg(Arg::with_name("iconfig")
                .long("iconfig")
                .takes_value(true)
                .value_name("path")
                .default_value("./interactive.yaml")
                .help("Specify path to interactive config file"))
            .arg(Arg::with_name("name")
                .takes_value(true)
                .long("name")
                .value_name("string")
                .help("Name for the client"))
            .arg(Arg::with_name("no-delay")
                .long("no-delay")
                .short("n")
                .help("Disables Nagle's algorithm and decreases overall latency for messages"))
            .arg(Arg::with_name("blocking")
                .long("blocking")
                .short("b")
                .help("Sets the client as blocking, requiring it to explicitly agree to advance simulation")
                // .long_help("Set to true to make the client blocking when processing turn advance requests.\
                // This is used for game-like behavior with synchronisation between clients")
            )
            .arg(Arg::with_name("compress")
                .long("compress")
                .short("c")
                .help("Flag specifying whether lz4 compression should be used by default"))
        )

        // worker subcommand
        .subcommand(SubCommand::with_name("worker")
            .display_order(23)
            .about("Start a worker node")
            .arg(Arg::with_name("ip")
                .required(false)
                .long("ip")
                .help("Set the ip address for the worker, together with port")
                .value_name("ip-address"))
            .arg(Arg::with_name("coord")
                .takes_value(true)
                .long("coord")
                .short("c")
                .help("Set the address of the cluster coordinator")
                .value_name("address"))
            .arg(Arg::with_name("passwd")
                .takes_value(true)
                .long("passwd")
                .short("p")
                .help("Set the password used for new client authentication"))
        );

    app
}

pub fn init() -> ArgMatches<'static> {
    app().get_matches()
}

/// Runs based on specified subcommand.
pub fn start(matches: ArgMatches) -> Result<()> {
    match matches.subcommand() {
        ("new", Some(m)) => start_new(m),
        ("test", Some(m)) => start_test(m),
        ("run", Some(m)) => start_run(m),
        ("server", Some(m)) => start_server(m),
        ("client", Some(m)) => start_client(m),
        ("worker", Some(m)) => start_worker(m),
        _ => Ok(()),
    }
}

// Initiate new content structure template based on input args
fn start_new(matches: &ArgMatches) -> Result<()> {
    // get the current `new` subcommand type t and it's matches m
    let (subcmd, m) = match matches.subcommand() {
        (t, Some(m)) => (t, m),
        _ => return Err(Error::msg(String::from("Failed to get init subcommand"))),
    };

    // get the data from matches, panic if can't get the data from matches for some reason
    let sub_matches = matches
        .subcommand_matches(subcmd)
        .expect(&format!("Failed to get \"{}\" subcommand matches", subcmd));
    let module_path = sub_matches
        .value_of("path")
        .expect(&format!("Failed to get {} path", subcmd));
    let module_template = sub_matches
        .value_of("template")
        .expect(&format!("Failed to get {} template", subcmd));

    // execute the init, raise any errors that may arise
    if let Err(e) = init::init_at_path(subcmd, module_path, module_template) {
        return Err(Error::msg(e));
    }

    Ok(())
}

fn start_test(matches: &ArgMatches) -> Result<()> {
    let mut path = match matches.value_of("path") {
        Some(p_str) => PathBuf::from(p_str),
        None => env::current_dir()?,
    };
    path = path.canonicalize().unwrap_or(path);
    let mut mem = matches.is_present("memory");
    let mut pro = matches.is_present("processing");
    if mem == false && pro == false {
        mem = true;
        pro = true;
    }
    test::scenario(path, mem, pro);
    Ok(())
}

fn start_run(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        ("scenario", Some(m)) => return start_run_scenario(m),
        ("snapshot", Some(m)) => return start_run_snapshot(m),
        // by default run scenario
        _ => return start_run_scenario(matches),
    };
    Ok(())
}

fn start_run_scenario(matches: &ArgMatches) -> Result<()> {
    let mut path = env::current_dir()?;
    match matches.value_of("path") {
        Some(p_str) => {
            let p = PathBuf::from(p_str);
            if p.is_relative() {
                path = path.join(p);
            } else {
                path = p;
            }
        }
        None => {
            println!("path arg not provided");
        }
    }
    path = path.canonicalize().unwrap_or(path);

    setup_log_verbosity(matches);

    if matches.is_present("interactive") {
        println!("Running interactive session using scenario at: {:?}", path);
        let sim = outcome::Sim::from_scenario_at_path(path)?;
        let driver = interactive::SimDriver::Local(sim);
        interactive::start(
            driver,
            matches
                .value_of("iconfig")
                .unwrap_or(interactive::CONFIG_FILE),
        );
    }
    Ok(())
}
fn start_run_snapshot(matches: &ArgMatches) -> Result<()> {
    setup_log_verbosity(matches);
    let mut path = env::current_dir()?;
    match matches.value_of("path") {
        Some(p_str) => {
            let p = PathBuf::from(p_str);
            if p.is_relative() {
                path = path.join(p);
            } else {
                path = p;
            }
        }
        None => {
            println!("path arg not found");
        }
    }
    path = path.canonicalize().unwrap_or(path);
    println!("Running interactive session using snapshot at: {:?}", path);
    if matches.is_present("interactive") {
        use self::simplelog::{Config, LevelFilter, TermLogger};
        let mut config_builder = simplelog::ConfigBuilder::new();
        let logger_conf = config_builder
            .set_time_level(LevelFilter::Error)
            .set_target_level(LevelFilter::Debug)
            .set_location_level(LevelFilter::Trace)
            .build();
        TermLogger::init(
            LevelFilter::Debug,
            logger_conf,
            simplelog::TerminalMode::Mixed,
        );
        // first try uncompressed, then compressed
        // TODO match errors properly
        // let sim =
        //     Sim::from_snapshot_at(&path, true).unwrap_or(Sim::from_snapshot_at(&path, false)?);

        let sim = Sim::from_snapshot_at(&path)?;
        // let sim = match Sim::from_snapshot_at(&path, false) {
        //     Ok(s) => s,
        //     Err(_) => match Sim::from_snapshot_at(&path, true) {
        //         Ok(ss) => ss,
        //         Err(_) => return Err("fail".to_string()),
        //     },
        // };
        let driver = interactive::SimDriver::Local(sim);
        //TODO
        interactive::start(
            driver,
            matches
                .value_of("iconfig")
                .unwrap_or(interactive::CONFIG_FILE),
        );
    }
    Ok(())
}

/// Starts a new server based on the passed arguments.
fn start_server(matches: &ArgMatches) -> Result<()> {
    setup_log_verbosity(matches);

    let server_address = match matches.value_of("ip-address") {
        Some(addr) => addr,
        None => unimplemented!(),
    };

    let mut use_auth = matches.is_present("password");

    let passwd_list = match matches.value_of("password") {
        //TODO support multiple passwords separated by ','
        Some(passwd_str) => vec![String::from(passwd_str)],
        None => Vec::new(),
    };

    if use_auth && passwd_list.len() == 0 {
        println!("Disabling authentication because there were no passwords provided.");
        use_auth = false;
    } else if !use_auth && passwd_list.len() > 0 {
        use_auth = true;
    }

    println!("listening for new clients on: {}", server_address);
    if let Some(cluster_addr) = matches.value_of("cluster") {
        println!("listening for new workers on: {}", &cluster_addr);
    }

    ServerSettings {
        name: match matches.value_of("name") {
            Some(n) => n.to_string(),
            None => "outcome_server".to_string(),
        },
        description: match matches.value_of("description") {
            Some(d) => d.to_string(),
            None => "It's a server alright.".to_string(),
        },
        address: server_address.to_string(),
        project_path: match matches.value_of("scenario-path") {
            Some(path) => path.to_string(),
            None => unimplemented!(),
        },
        use_auth,
        passwd_list,
        use_compression: matches.is_present("use-compression"),
        keepalive_millis: match matches.value_of("keep-alive") {
            Some(millis) => match millis.parse::<usize>() {
                Ok(f) => f,
                Err(e) => panic!("failed parsing keep-alive (millis) value: {}", e),
            },
            // 0 means keep alive forever
            None => 0,
        },
        cluster: matches.value_of("cluster").map(|s| s.to_string()),
        workers: match matches.value_of("workers") {
            Some(workers_str) => workers_str
                .split(",")
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            None => Vec::new(),
        },
    }
    .build()?
    .start()?;

    // Server::start(sim_instance, server_address)?;

    Ok(())
}

fn start_client(matches: &ArgMatches) -> Result<()> {
    setup_log_verbosity(matches);
    let mut client = outcome_net::Client::new(
        matches.value_of("name").unwrap_or("cli-client"),
        matches.is_present("blocking"),
        matches.is_present("compress"),
        matches.value_of("public-addr").map(|s| s.to_string()),
        // TODO
        Some(1000),
    )?;
    println!("created new client");
    client.connect(
        matches
            .value_of("server-addr")
            .map(|s| s.to_string())
            .ok_or(Error::msg("server address must be provided"))?,
        matches.value_of("password").map(|s| s.to_string()),
    )?;
    interactive::start(
        interactive::SimDriver::Remote(client),
        matches
            .value_of("iconfig")
            .unwrap_or(interactive::CONFIG_FILE),
    );
    Ok(())
}

fn start_worker(matches: &ArgMatches) -> Result<()> {
    setup_log_verbosity(matches);
    let my_address = match matches.value_of("ip") {
        Some(addr) => addr,
        // None => outcome_net::cluster::worker::WORKER_ADDRESS,
        None => unimplemented!(),
    };
    let mut use_auth = matches.is_present("use_auth");
    let passwd_list = match matches.value_of("passwd") {
        //TODO support multiple passwords separated by ','
        Some(passwd_str) => vec![String::from(passwd_str)],
        None => Vec::new(),
    };
    if use_auth && passwd_list.len() == 0 {
        println!("Disabling authentication because there were no passwords provided.");
        use_auth = false;
    } else if !use_auth && passwd_list.len() > 0 {
        use_auth = true;
    }

    // unimplemented!();
    // let listener = TcpListener::bind(my_address).expect("failed to bind listener");
    // let mut worker_arc = Arc::new(Mutex::new(Worker::new(my_address)));
    println!("Now listening on {}", my_address);

    let mut worker = Worker::new(my_address)?;

    if let Some(coord_addr) = matches.value_of("coord") {
        print!("initiating connection with coordinator... ");
        std::io::stdout().flush()?;

        match worker.initiate_coord_connection(coord_addr, Duration::from_millis(2000)) {
            Ok(_) => print!("success\n"),
            Err(e) => print!("failed ({:?})", e),
        }
    }
    worker.handle_coordinator()?;
    // first connection is made by the coordinator
    // thread::spawn(move || {

    // worker_arc
    //     .lock()
    //     .unwrap()
    //     .as_mut()
    //     .unwrap()
    //     .handle_coordinator();

    // });

    // listener.set_nonblocking(true);
    //
    // let mut counter = 0;
    // let listener_accept_count = 1000;
    // loop {
    //     counter += 1;
    //     if counter == listener_accept_count {
    //         let worker = worker_mutex.clone();
    //         //println!("do other things");
    //         counter = 0;
    //         //            thread::sleep(Duration::from_millis(2000));
    //         sleep(Duration::from_millis(1));
    //
    //         match listener.accept() {
    //             Ok((stream, addr)) => {
    //                 stream.set_read_timeout(Some(Duration::from_secs(1)));
    //                 thread::spawn(move || {
    //                     handle_comrade(worker.clone(), stream);
    //                     //                        serv.lock().unwrap().prune_clients();
    //                 });
    //                 //                    stream.set_nonblocking(true);
    //             }
    //             Err(e) => {
    //                 if e.kind() == ErrorKind::WouldBlock {
    //                     //...
    //                 } else {
    //                     println!("couldn't get client: {:?}", e);
    //                 }
    //             }
    //         }
    //     }
    // }

    Ok(())
}

fn setup_log_verbosity(matches: &ArgMatches) {
    use self::simplelog::{Config, LevelFilter, TermLogger};
    let level_filter = match matches.value_of("verbosity") {
        Some(s) => match s {
            "0" | "none" => LevelFilter::Off,
            "1" | "err" | "error" | "min" => LevelFilter::Error,
            "2" | "warn" | "warning" | "default" => LevelFilter::Warn,
            "3" | "info" => LevelFilter::Info,
            "4" | "debug" => LevelFilter::Debug,
            "5" | "trace" | "max" | "all" => LevelFilter::Trace,
            _ => LevelFilter::Warn,
        },
        _ => LevelFilter::Warn,
    };
    let mut config_builder = simplelog::ConfigBuilder::new();
    let logger_conf = config_builder
        .set_time_level(LevelFilter::Error)
        .set_target_level(LevelFilter::Debug)
        .set_location_level(LevelFilter::Trace)
        .set_time_format_str("%H:%M:%S%.6f")
        .build();
    TermLogger::init(level_filter, logger_conf, simplelog::TerminalMode::Mixed);
}
