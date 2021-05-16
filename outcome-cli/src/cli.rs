//! Application definition.

extern crate simplelog;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::time::Duration;
use std::{env, thread};

use anyhow::{Error, Result};
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use outcome::Sim;
use outcome_net::{
    CompressionPolicy, Organizer, Server, ServerConfig, SimConnection, SocketEvent,
    SocketEventType, Worker,
};

#[cfg(feature = "watcher")]
use notify::{RecommendedWatcher, Watcher};

use crate::interactive::{OnSignal, OnSignalAction};
use crate::util::{
    find_project_root, format_elements_list, get_scenario_paths, get_snapshot_paths,
};
use crate::{interactive, test};
use std::str::FromStr;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

pub fn app_matches() -> ArgMatches<'static> {
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
            .value_name("level")
            .default_value("info")
            .global(true)
            .help("Set the verbosity of the log output \
            [possible values: trace, debug, info, warn, error, none]"))

        // new
        .subcommand(SubCommand::with_name("new")
            .about("Create new project using a default template")
            .display_order(10)
            .arg(Arg::with_name("name")
                .required(true))
        )

        // test
        .subcommand(SubCommand::with_name("test")
            .about("Test for memory requirements and average processing speed")
            .display_order(12)
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

        // run
        .subcommand(SubCommand::with_name("run")
            .about("Run a simulation locally")
            .display_order(20)
            .long_about("Run simulation from scenario, snapshot or experiment.\n\
                If there are no arguments supplied the program will look for a scenario,\n\
                snapshot or proof (in that order) in the current working directory.")
            .arg(Arg::with_name("path")
                .value_name("path"))
            .arg(Arg::with_name("scenario")
                .long("scenario")
                .short("s")
                .help("Start new simulation run using a scenario manifest file"))
            .arg(Arg::with_name("snapshot")
                .long("snapshot")
                .short("n")
                .help("Start new simulation run using a snapshot file"))
            .arg(Arg::with_name("server")
                .long("server")
                .help("Enable server-backed local simulation, allowing for attaching services")
                .takes_value(true)
                .default_value("127.0.0.1:0"))
            .arg(Arg::with_name("interactive")
                .long("interactive")
                .short("i")
                .default_value("true"))
            .arg(Arg::with_name("icfg")
                .long("icfg")
                .help("specify path to interactive mode configuration file")
                .takes_value(true)
                .value_name("path")
                .default_value("./interactive.yaml"))
            .arg(Arg::with_name("watch")
                .long("watch")
                .help("Watch project directory for changes")
                .value_name("on-change")
                .default_value("restart")
                .possible_values(&["restart", "update"]))

        )


        // server
        .subcommand(SubCommand::with_name("server")
            .about("Start a server")
            .long_about("Start a server. Server listens to incoming client connections \n\
            and fulfills client requests, anything from data transfers to entity spawning.\n\n\
            `server` subcommand allows for quickly starting either a local- or union-backed \n\
            server. Simulation can be started with either a path to scenario or a snapshot.\n\n\
            `outcome server -s ./scenarios/hello_world` \n    \
            (starts a server backed by local simulation process, based on a selected scenario)\n\n\
            NOTE: data sent between client and server is not encrypted, connection is not \n\
            secure! Basic authentication methods are provided, but they are more of \n\
            a convenience than a serious security measure.")
            .display_order(21)
            .arg(Arg::with_name("scenario")
                .long("scenario")
                .short("s")
                .display_order(1)
                .required(false)
                .value_name("scenario-path"))
            .arg(Arg::with_name("snapshot")
                .long("snapshot")
                .display_order(2)
                .required(false)
                .value_name("snapshot-path"))
            .arg(Arg::with_name("address")
                .long("address")
                .short("a")
                .help("Set the address of the server")
                .display_order(3)
                .required(false)
                .default_value("127.0.0.1:9123")
                .value_name("address"))
            .arg(Arg::with_name("keep-alive")
                .long("keep-alive")
                .short("k")
                .display_order(4)
                .help("Server process will quit if it doesn't receive any messages within \
                the specified time frame (seconds)")
                .takes_value(true)
                .value_name("seconds"))
            .arg(Arg::with_name("client-keep-alive")
                .long("client-keep-alive")
                .help("Server process will remove client if it doesn't receive any messages \
                 from that client the specified time frame (seconds)")
                .display_order(5)
                .takes_value(true)
                .value_name("seconds"))
            .arg(Arg::with_name("compress")
                .long("compress")
                .short("c")
                .help("Use lz4 compression based on selected policy")
                .display_order(6)
                .takes_value(true)
                .value_name("compression-policy")
                .possible_values(&["all", "bigger_than_[n_bytes]"]))
            .arg(Arg::with_name("organizer")
                .long("organizer")
                .short("o")
                .help("Start a server backed by a union organizer")
                .display_order(100)
                .takes_value(true)
                .value_name("organizer-address"))
            .arg(Arg::with_name("union")
                .long("union")
                .short("u")
                .help("Start a server backed by an organizer and a workplace")
                .display_order(101)
                .takes_value(true)
                .value_name("coordinator-address"))
            .arg(Arg::with_name("workers")
                .long("workers")
                .short("w")
                .help("List of known union workers' addresses, only applicable if \
                `--organizer`or `--union` option is also present")
                .display_order(102)
                .takes_value(true)
                .value_name("worker-addresses"))
            .arg(Arg::with_name("encodings")
                .long("encodings")
                .short("e")
                .help("List of supported encodings")
                .takes_value(true)
                .value_name("encodings-list"))
            .arg(Arg::with_name("transports")
                .long("transports")
                .short("t")
                .help("List of supported transports")
                .takes_value(true)
                .value_name("transports-list"))
        )

        // client
        .subcommand(SubCommand::with_name("client")
            .about("Start an interactive client session")
            .long_about("Start an interactive client session.\n\n\
            Establishes a client connection to a server at specified address, \n\
            and provides a REPL-style interface for interacting with that \n\
            server. \n\n\
            NOTE: Data sent between client and server is not encrypted, \n\
            connection is not secure! Passwords are used, but they are more of \n\
            a convenience than a serious security measure.")
            .display_order(22)
            .arg(Arg::with_name("server-addr")
                .long("server")
                .short("s")
                .help("Address of the server")
                .required(true)
                .value_name("address"))
            .arg(Arg::with_name("client-addr")
                .long("address")
                .help("Address of this client")
                .value_name("address"))
            .arg(Arg::with_name("auth")
                .long("auth")
                .short("a")
                .help("Authentication pair used when connecting to server \
                [example value: user,password]")
                .takes_value(true))
            .arg(Arg::with_name("icfg")
                .long("icfg")
                .help("Path to interactive config file")
                .takes_value(true)
                .value_name("path")
                .default_value("./interactive.yaml"))
            .arg(Arg::with_name("name")
                .long("name")
                .short("n")
                .help("Name for the client")
                .takes_value(true)
                .value_name("string"))
            .arg(Arg::with_name("blocking")
                .long("blocking")
                .short("b")
                .help("Sets the client as blocking, requiring it to explicitly \
                agree to step simulation forward")
            )
            .arg(Arg::with_name("compress")
                .long("compress")
                .short("c")
                .help("Sets whether outgoing messages should be compressed, \
                and based on what policy [possible values: all, bigger_than_n, data, none]")
                .takes_value(true)
                .value_name("policy")
                .default_value("none"))
            .arg(Arg::with_name("heartbeat")
                .long("heartbeat")
                .help("Set the heartbeat frequency in heartbeat per n seconds")
                .takes_value(true)
                .value_name("secs")
                .default_value("1"))
            .arg(Arg::with_name("encodings")
                .long("encodings")
                .short("e")
                .help("Supported encodings that can be used when talking to server")
                .default_value("bincode"))
            .arg(Arg::with_name("transports")
                .long("transports")
                .short("t")
                .help("Supported transports that can be used when talking to server")
                .default_value("tcp"))
        )

        .subcommand(SubCommand::with_name("worker")
            .about("Start a worker")
            .long_about("Start a worker. Worker is the smallest independent part\n\
            of a system where a collection of worker nodes collaboratively\n\
            simulate a larger world.\n\n\
            Worker must have a connection to the main organizer, whether direct\n\
            or indirect. Indirect connection to organizer can happen through another\n\
            worker or a relay.")
            .display_order(23)
            .arg(Arg::with_name("address")
                .long("address")
                .short("a")
                .help("Set the address for the worker")
                .value_name("address"))
            .arg(Arg::with_name("organizer")
                .long("organizer")
                .short("o")
                .help("Address of the union organizer to connect to")
                .takes_value(true)
                .value_name("address"))
            .arg(Arg::with_name("server")
                .long("server")
                .short("s")
                .help("Establish a server backed by this worker")
                .takes_value(true)
                .min_values(0)
                .value_name("address"))
        )

        .subcommand(SubCommand::with_name("workplace")
            .about("Start a workplace")
            .long_about("Start a workplace. Workplace is a collection of\n\
            workers grouped under a single relay.\n\n\
            Workplace is intended to represent a single machine. Organizers\n\
            from different unions can request workplace workers to join their union.\n\n\
            Machines intended for simulation work can be setup as workplaces,\n\
            and then be left for the automated access from authorized organizers.")
            .display_order(24)
            .arg(Arg::with_name("address")
                .long("address")
                .short("a")
                .help("Set the address of the workplace")
                .value_name("address"))
            .arg(Arg::with_name("organize")
                .long("organize")
                .short("o")
                .help("Address of the union organizer to contact")
                .takes_value(true)
                .value_name("address"))
            .arg(Arg::with_name("server")
                .long("server")
                .short("s")
                .help("Establish a server at the level of the workplace")
                .takes_value(true)
                .min_values(0)
                .value_name("address"))
        )

        .subcommand(SubCommand::with_name("union")
            .about("Start a union")
            .long_about("Start a union. Union is a collection of workers that work \n\
            together to simulate a single world, coordinated by an organizer.\n\n\
            Workers and organizers can join and leave the union at runtime.\n\
            At any given time, only a single organizer is considered a central\n\
            authority on key issues like model mutation.")
            .display_order(25)
            .arg(Arg::with_name("config")
                .long("config")
                .short("c")
                .help("Specify path to union configuration file")
                .value_name("path"))
            .arg(Arg::with_name("address")
                .long("address")
                .help("Set the address for the worker")
                .value_name("address"))
            .arg(Arg::with_name("organizer")
                .long("organizer")
                .short("o")
                .help("Set the address of the union organizer")
                .takes_value(true)
                .value_name("address"))
            .arg(Arg::with_name("server")
                .long("server")
                .short("s")
                .help("Make the worker into a server able to handle clients")
                .takes_value(true)
                .min_values(0)
                .value_name("address"))
        )

        .subcommand(SubCommand::with_name("organizer")
            .about("Start a union organizer")
            .display_order(26)
            .arg(Arg::with_name("address")
                .long("address")
                .help("Set the address for the worker")
                .value_name("address"))
            .arg(Arg::with_name("coord")
                .long("coord")
                .short("c")
                .help("Address of the cluster coordinator to connect to")
                .takes_value(true)
                .value_name("address"))
            .arg(Arg::with_name("server")
                .long("server")
                .short("s")
                .help("Make the worker into a server able to handle clients")
                .takes_value(true)
                .min_values(0)
                .value_name("address"))
        );

    app.get_matches()
}

/// Runs based on specified subcommand.
pub fn start(matches: ArgMatches) -> Result<()> {
    setup_log_verbosity(&matches);
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

fn start_new(matches: &ArgMatches) -> Result<()> {
    let name = matches.value_of("name").unwrap();
    unimplemented!();
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

/// Starts a new simulation run, using a scenario or a snapshot file.
///
/// # Resolving ambiguity
///
/// If an explicit option for loading either scenario or snapshot is not
/// provided, this function chooses which one is more appropriate based on
/// directory structure and file name analysis.
///
/// If the path argument is not provided, this function will scan the project
/// directory and print possible choices to the user.
fn start_run(matches: &ArgMatches) -> Result<()> {
    let mut path = env::current_dir()?;
    match matches.value_of("path") {
        Some(p_str) => {
            let p = PathBuf::from(p_str);

            // if the argument provides a simple name, use it in combination
            // with project root
            if matches.is_present("snapshot") {
                if !p_str.contains("/") {
                    let root = find_project_root(path.clone(), 4)?;
                    let available = get_snapshot_paths(root).unwrap();
                    for snap_path in &available {
                        if snap_path.file_stem().unwrap() == p_str {
                            return start_run_snapshot(snap_path.clone(), matches);
                        }
                    }
                    return Err(Error::msg(format!(
                        "snapshot not found in project: {}, available snapshots: {}",
                        p_str,
                        format_elements_list(&available)
                    )));
                }
            } else {
                if !p_str.contains("/") && !p_str.ends_with(".toml") {
                    let root = find_project_root(path, 4)?;
                    let available = get_scenario_paths(root).unwrap();
                    for scenario_path in &available {
                        if scenario_path.file_stem().unwrap() == p_str {
                            return start_run_scenario(scenario_path.clone(), matches);
                        }
                    }
                    return Err(Error::msg(format!(
                        "scenario not found in project: {}, available scenarios: {}",
                        p_str,
                        format_elements_list(&available)
                    )));
                }
            }

            // if provided path is relative, append it to current working directory
            if p.is_relative() {
                path = path.join(p);
            }
            // otherwise if it's absolute then just set it as the path
            else {
                path = p;
            }
        }
        // choose what to do if no path was provided
        None => {
            let root = find_project_root(path.clone(), 4)?;
            if matches.is_present("scenario") && matches.is_present("snapshot") {
                return Err(Error::msg("choose to run either scenario or snapshot"));
            } else if matches.is_present("snapshot") {
                let available = get_snapshot_paths(root)?;
                if available.len() == 1 {
                    return start_run_snapshot(available[0].clone(), matches);
                } else if available.len() > 0 {
                    return Err(Error::msg(format!(
                        "choose one of the available snapshots: {}",
                        format_elements_list(&available)
                    )));
                } else {
                    return Err(Error::msg(format!("no snapshots available in project",)));
                }
            } else {
                let available = get_scenario_paths(root)?;
                if available.len() == 1 {
                    return start_run_scenario(available[0].clone(), matches);
                } else {
                    return Err(Error::msg(format!(
                        "choose one of the available scenarios: {}",
                        format_elements_list(&available)
                    )));
                }
                return Err(Error::msg("must provide path to scenario manifest file"));
            }
        }
    }

    path = path.canonicalize().unwrap_or(path);

    if matches.is_present("scenario") {
        return start_run_scenario(path, matches);
    } else if matches.is_present("snapshot") {
        return start_run_snapshot(path, matches);
    } else {
        if path.is_file() {
            // decide whether the path looks more like scenario or snapshot
            if let Some(ext) = path.extension() {
                if ext == "toml" {
                    return start_run_scenario(path, matches);
                }
            }
            return start_run_snapshot(path, matches);
        }
        // path is provided but it's a directory
        else {
            let root = find_project_root(path.clone(), 4)?;
            let scenario_paths = get_scenario_paths(path.clone())?;
            if scenario_paths.len() == 1 {
                return start_run_scenario(scenario_paths[0].clone(), matches);
            } else if scenario_paths.len() > 0 {
                return Err(Error::msg(format!(
                    "choose one of the available scenarios: {}",
                    format_elements_list(&scenario_paths)
                )));
            } else {
                return Err(Error::msg("no scenarios found"));
            }
            return Err(Error::msg("failed to find scenarios"));
        }
    }

    Ok(())
}

fn start_run_scenario(path: PathBuf, matches: &ArgMatches) -> Result<()> {
    if matches.is_present("interactive") {
        info!("Running interactive session using scenario at: {:?}", path);

        let config_path = matches
            .value_of("icfg")
            .unwrap_or(interactive::CONFIG_FILE)
            .to_string();

        let mut on_change = None;

        if matches.is_present("watch") {
            #[cfg(feature = "watcher")]
            {
                use std::sync::Mutex;
                let watch_path = find_project_root(path.clone(), 4)?;
                info!(
                    "watching changes at project path: {}",
                    watch_path.to_string_lossy()
                );
                // let driver = interactive::SimDriver::Local(sim);
                let change_detected = Arc::new(Mutex::new(false));
                let change_detected_clone = change_detected.clone();
                let mut watcher: RecommendedWatcher =
                    Watcher::new_immediate(move |res: Result<notify::Event, notify::Error>| {
                        match res {
                            Ok(event) => {
                                debug!("change detected: {:?}", event);
                                *change_detected_clone.lock().unwrap() = true;
                            }
                            Err(e) => {
                                error!("watch error: {:?}", e);
                                *change_detected_clone.lock().unwrap() = true;
                            }
                        }
                    })?;
                watcher.watch(watch_path, notify::RecursiveMode::Recursive)?;

                on_change = match matches.value_of("watch") {
                    Some("restart") => Some(interactive::OnChange {
                        trigger: change_detected.clone(),
                        action: interactive::OnChangeAction::Restart,
                    }),
                    Some("update") => Some(interactive::OnChange {
                        trigger: change_detected.clone(),
                        action: interactive::OnChangeAction::UpdateModel,
                    }),
                    Some(_) | None => None,
                };
            }

            #[cfg(not(feature = "watcher"))]
            {
                warn!("tried to use watcher, but that feature is not enabled")
            }
        }

        // run a loop allowing signal handling
        let triggered = Arc::new(AtomicBool::new(false));
        let r = triggered.clone();
        ctrlc::set_handler(move || {
            r.store(true, Ordering::SeqCst);
        })
        .expect("error setting ctrlc handler");

        interactive::start(
            interactive::InterfaceType::Scenario(path.to_string_lossy().to_string()),
            &config_path,
            on_change,
            Some(OnSignal {
                trigger: triggered,
                action: OnSignalAction::Custom,
            }),
        )?;
    }
    Ok(())
}

fn start_run_snapshot(path: PathBuf, matches: &ArgMatches) -> Result<()> {
    info!("Running interactive session using snapshot at: {:?}", path);
    if matches.is_present("interactive") {
        interactive::start(
            interactive::InterfaceType::Snapshot(path.to_string_lossy().to_string()),
            matches.value_of("icfg").unwrap_or(interactive::CONFIG_FILE),
            None,
            None,
        );
    }
    Ok(())
}

fn start_server(matches: &ArgMatches) -> Result<()> {
    let server_address = match matches.value_of("address") {
        Some(addr) => addr,
        None => unimplemented!(),
    };

    if let Some(cluster_addr) = matches.value_of("cluster") {
        info!("listening for new workers on: {}", &cluster_addr);
    }

    let default = ServerConfig::default();
    println!("default transports list: {:?}", default.transports);
    let config = ServerConfig {
        name: match matches.value_of("name") {
            Some(n) => n.to_string(),
            None => "outcome_server".to_string(),
        },
        description: match matches.value_of("description") {
            Some(d) => d.to_string(),
            None => "It's a server alright.".to_string(),
        },
        self_keepalive: match matches.value_of("keep-alive") {
            Some(millis) => match millis.parse::<usize>() {
                Ok(ka) => match ka {
                    // 0 means keep alive forever
                    0 => None,
                    _ => Some(Duration::from_millis(ka as u64)),
                },
                Err(e) => panic!("failed parsing keep-alive (millis) value: {}", e),
            },
            // nothing means keep alive forever
            None => None,
        },
        poll_wait: Duration::from_millis(1),
        accept_delay: Duration::from_millis(100),

        client_keepalive: match matches
            .value_of("client-keep-alive")
            .map(|v| v.parse().unwrap())
        {
            None => Some(Duration::from_secs(2)),
            Some(0) => None,
            Some(v) => Some(Duration::from_secs(v)),
        },

        use_auth: false,
        use_compression: matches.is_present("use-compression"),
        auth_pairs: vec![],
        transports: match matches.value_of("transports") {
            Some(trans) => {
                println!("trans: {}", trans);
                let split = trans.split(',').collect::<Vec<&str>>();
                let mut transports = Vec::new();
                for transport_str in split {
                    if !transport_str.is_empty() {
                        transports.push(transport_str.parse()?);
                    }
                }
                transports
            }
            None => default.transports,
        },
        encodings: match matches.value_of("encodings") {
            Some(enc) => {
                let split = enc.split(',').collect::<Vec<&str>>();
                let mut encodings = Vec::new();
                for encoding_str in split {
                    if !encoding_str.is_empty() {
                        encodings.push(encoding_str.parse()?);
                    }
                }
                encodings
            }
            None => default.encodings,
        },
    };

    let worker_addrs = match matches.value_of("workers") {
        Some(wstr) => wstr
            .split(',')
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
        None => Vec::new(),
    };

    let sim_instance = match matches.value_of("cluster") {
        Some(addr) => {
            if let Some(scenario_path) = matches.value_of("scenario") {
                SimConnection::UnionOrganizer(Organizer::new_with_path(
                    &scenario_path,
                    addr,
                    worker_addrs,
                )?)
            } else if let Some(snapshot_path) = matches.value_of("snapshot") {
                unimplemented!()

                // SimConnection::ClusterCoord(Coord::new_with_path());
            } else {
                panic!()
            }
        }
        None => {
            if let Some(scenario_path) = matches.value_of("scenario") {
                SimConnection::Local(Sim::from_scenario_at(&scenario_path)?)
            } else if let Some(snapshot_path) = matches.value_of("snapshot") {
                SimConnection::Local(Sim::from_snapshot_at(&snapshot_path)?)
            } else {
                unimplemented!()
            }
        }
    };

    let mut server = Server::new_with_config(server_address, config, sim_instance)?;
    server.initialize_services()?;

    // run a loop allowing graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("error setting ctrlc handler");

    server.start_polling(running)?;
    println!("Initiating graceful shutdown...");
    for (client_id, client) in &mut server.clients {
        client.connection.disconnect(None);
    }
    match &server.sim {
        SimConnection::UnionOrganizer(coord) => {
            for (_, worker) in &coord.net.workers {
                worker
                    .connection
                    .send_event(SocketEvent::new(SocketEventType::Disconnect), None)?;
            }
        }
        _ => (),
    }

    // server.manual_poll()?;
    server.cleanup()?;

    thread::sleep(Duration::from_secs(1));

    Ok(())
}

fn start_client(matches: &ArgMatches) -> Result<()> {
    let mut client = outcome_net::Client::new_with_config(
        // matches.value_of("public-addr").map(|s| s.to_string()),
        outcome_net::ClientConfig {
            name: matches.value_of("name").unwrap_or("cli-client").to_string(),
            heartbeat: match matches.value_of("heartbeat") {
                Some(h) => Some(Duration::from_secs(h.parse()?)),
                None => None,
            },
            is_blocking: matches.is_present("blocking"),
            compress: CompressionPolicy::from_str(matches.value_of("compress").unwrap())?,
            //matches.is_present("compress"),
            encodings: match matches.value_of("encodings") {
                Some(encodings_str) => {
                    let split = encodings_str.split(',').collect::<Vec<&str>>();
                    let mut transports = Vec::new();
                    for transport_str in split {
                        if !transport_str.is_empty() {
                            transports.push(transport_str.parse()?);
                        }
                    }
                    transports
                }
                None => Vec::new(),
            },
            transports: match matches.value_of("transports") {
                Some(transports_str) => {
                    let split = transports_str.split(',').collect::<Vec<&str>>();
                    let mut transports = Vec::new();
                    for transport_str in split {
                        if !transport_str.is_empty() {
                            transports.push(transport_str.parse()?);
                        }
                    }
                    transports
                }
                None => Vec::new(),
            },
        },
    )?;

    client.connect(
        &matches
            .value_of("server-addr")
            .map(|s| s.to_string())
            .ok_or(Error::msg("server adddress must be provided"))?,
        matches.value_of("password").map(|s| s.to_string()),
    )?;

    // run a loop allowing signal handling
    let triggered = Arc::new(AtomicBool::new(false));
    let r = triggered.clone();
    ctrlc::set_handler(move || {
        r.store(true, Ordering::SeqCst);
    })
    .expect("error setting ctrlc handler");

    interactive::start(
        interactive::InterfaceType::Remote(client),
        matches.value_of("icfg").unwrap_or(interactive::CONFIG_FILE),
        None,
        Some(OnSignal {
            trigger: triggered,
            action: OnSignalAction::Custom,
        }),
    );
    Ok(())
}

fn start_worker(matches: &ArgMatches) -> Result<()> {
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

    let mut worker = Worker::new(matches.value_of("address"))?;
    println!("Now listening on {}", worker.greeter.listener_addr()?);

    if let Some(coord_addr) = matches.value_of("coord") {
        print!("initiating connection with coordinator... ");
        std::io::stdout().flush()?;

        match worker.initiate_coord_connection(coord_addr, Duration::from_millis(2000)) {
            Ok(_) => print!("success\n"),
            Err(e) => print!("failed ({:?})", e),
        }
    }
    worker.handle_coordinator()?;

    // allow graceful shutdown on signal
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("error setting ctrlc handler");

    if matches.is_present("server") {
        let server_addr = match matches.value_of("server") {
            Some(s) => s,
            None => "127.0.0.1:0",
        };

        // TODO get server address
        let mut server = Server::new(server_addr, SimConnection::UnionWorker(worker))?;
        server.initialize_services()?;

        server.start_polling(running);

        println!("Initiating graceful shutdown...");
        for (client_id, client) in &mut server.clients {
            client.connection.disconnect(None);
        }
        // server.manual_poll()?;
        server.cleanup()?;

        thread::sleep(Duration::from_secs(1));
    } else {
        loop {
            // terminate loop if the `running` bool gets flipped to false
            if !running.load(Ordering::SeqCst) {
                break;
            }

            worker.manual_poll()?;
            // if let Err(e) = worker.manual_poll() {
            //     println!("{}", e);
            // }

            // wait a little to reduce polling overhead
            thread::sleep(Duration::from_millis(3));
        }
    }

    Ok(())
}

/// Sets up logging based on settings from the matches.
fn setup_log_verbosity(matches: &ArgMatches) {
    use self::simplelog::{LevelFilter, TermLogger};
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
        .set_location_level(LevelFilter::Error)
        //.set_location_level(LevelFilter::Trace)
        .set_time_format_str("%H:%M:%S%.6f")
        .build();
    TermLogger::init(level_filter, logger_conf, simplelog::TerminalMode::Mixed);
}
