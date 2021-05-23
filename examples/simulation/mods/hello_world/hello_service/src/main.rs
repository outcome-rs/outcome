#![allow(unused)]

use anyhow::Result;

use outcome_core::machine::cmd::CommandResult;
use outcome_core::{
    entity::Storage, entity::StorageIndex, string::new_truncate, CompName, EntityId, EntityName,
    VarName,
};
use outcome_net::msg::{
    DataPullRequest, DataTransferRequest, DataTransferResponse, Message, MessageType,
    PullRequestData, TransferResponseData, TurnAdvanceRequest, TurnAdvanceResponse,
    TypedSimDataPack, VarSimDataPack, VarSimDataPackOrdered,
};
use outcome_net::{Client, ClientConfig, CompressionPolicy, SocketEvent, SocketEventType};
use std::convert::TryFrom;
use std::time::{Duration, Instant};

pub fn main() -> Result<()> {
    let mut hello_string = "starting hello_service with the following arguments:".to_string();
    let mut args = Vec::new();
    let _args = std::env::args().skip(1);
    for arg in _args {
        hello_string.push_str(&format!(" {}", arg));
        args.push(arg);
    }

    println!("[hello_service] {}", hello_string);

    let mut client = Client::new_with_config(ClientConfig {
        name: "hello_service".to_string(),
        heartbeat: Some(Duration::from_secs(1)),
        is_blocking: true,
        ..Default::default()
    })?;

    // TODO get server address from stdin
    client.connect(&args[0], None);

    let mut advanced_turn = false;
    let mut received_data = true;

    // println!("connected");

    client.connection.send_payload(
        TurnAdvanceRequest {
            step_count: 1,
            wait: true,
        },
        None,
    )?;

    let start = Instant::now();

    loop {
        // println!("loop");
        // println!("advanced_turn: {}", advanced_turn);
        if advanced_turn {
            let mut data = VarSimDataPack {
                vars: Default::default(),
            };
            data.vars.insert(
                // "2:hello_greetable:str:hello".to_string(),
                (
                    EntityName::from("2").unwrap(),
                    CompName::from("greeting").unwrap(),
                    VarName::from("hello").unwrap(),
                ),
                outcome_core::Var::String(format!(
                    "hello since {}",
                    Instant::now().duration_since(start).as_millis()
                )),
            );
            client.connection.send_payload(
                DataPullRequest {
                    data: PullRequestData::NativeAddressedVars(data),
                },
                None,
            )?;
            client.connection.send_payload(
                TurnAdvanceRequest {
                    step_count: 1,
                    wait: true,
                },
                None,
            )?;
            // println!("[hello_service] just sent turn advance request");
            advanced_turn = false;
        }

        loop {
            if let Ok((addr, event)) = client.connection.try_recv() {
                match event.type_ {
                    SocketEventType::Bytes => {
                        let msg = Message::from_bytes(event.bytes, client.connection.encoding())?;
                        match MessageType::try_from(msg.type_)? {
                            MessageType::TurnAdvanceResponse => {
                                let resp: TurnAdvanceResponse =
                                    msg.unpack_payload(client.connection.encoding())?;

                                // println!(
                                //     "[hello_service] received turn advance response: {:?}",
                                //     resp
                                // );

                                if resp.error.is_empty() {
                                    // println!("[{:?}] advanced turn", std::time::SystemTime::now());
                                    advanced_turn = true;
                                }
                                // else {
                                //     // println!("{}", resp.error);
                                //     client.connection.send_payload(
                                //         TurnAdvanceRequest {
                                //             step_count: 1,
                                //             wait: true,
                                //         },
                                //         None,
                                //     )?;
                                // }
                            }
                            MessageType::DataTransferResponse => {
                                println!("received data transfer response");
                                // let resp: DataTransferResponse =
                                //     msg.unpack_payload(client.connection.encoding())?;
                                // if let Some(resp_data) = resp.data {
                                //     match resp_data {
                                //         TransferResponseData::VarOrdered(ord_id, d) => {
                                //             order_id = Some(ord_id);
                                //             data = d
                                //         }
                                //         _ => (),
                                //     }
                                // }
                                received_data = true;
                            }
                            MessageType::DataPullResponse => {
                                // println!("received pull response");
                            }
                            _ => (),
                        }
                    }
                    SocketEventType::Disconnect => {
                        println!("server disconnected");
                        return Ok(());
                    }
                    SocketEventType::Heartbeat => (),
                    _ => println!("unhandled event: {:?}", event),
                }
            } else {
                break;
            }
        }

        // std::thread::sleep(std::time::Duration::from_millis(1));

        // let data_pack = client.get_vars()?;
        // println!("data_pack: {:?}", data_pack);
        // if let Ok(resp) = client.server_step_request(4) {
        //     println!(
        //         "{:?}",
        //         resp.unpack_payload::<TurnAdvanceResponse>(client.connection.encoding())
        //     );
        // }
    }

    Ok(())
}

fn match_msg(msg: Message) {}

fn data_transfer_request() -> DataTransferRequest {
    DataTransferRequest {
        transfer_type: "SelectVarOrdered".to_string(),
        // selection: vec!["*:velocity:float:x".to_string()],
        selection: vec![],
    }
}

pub fn calculate_entity(
    ent_uid: &EntityId,
    entity: &mut Storage,
    import: &mut Storage,
) -> Result<CommandResult> {
    // let bounds: Bounds<Vec2> = windows.get_primary().unwrap().into();

    // let import: Vec<f64> = entity
    //     .get_vars(("flock_sync/avg_pos_x", "flock_sync/avg_pos_y"))
    //     .unwrap()
    //     .as_float()
    //     .unwrap();
    let average_forward = entity
        .get_var(&(new_truncate("flock_sync"), new_truncate("avg_fwd")))?
        .as_float()?;
    //
    // for (flock, children) in query.iter() {
    //     let mut average_position = Vec2::zero();
    //     let mut average_forward = Vec2::zero();
    //     let mut boids = Vec::new();
    //
    //     for child in children.iter() {
    //         if let Ok((velocity, transform, params)) = child_query.get_mut(*child) {
    //             let mut current_average = average_position;
    //             if boids.len() > 0 {
    //                 current_average =
    //                     (current_average / boids.len() as f32).bound_to(Vec2::zero(), bounds);
    //             }
    //
    //             average_position += transform
    //                 .translation
    //                 .truncate()
    //                 .bound_to(current_average, bounds);
    //             average_forward += velocity.0;
    //             boids.push((child.id(), transform.translation.truncate(), params.clone()));
    //         }
    //     }
    //
    //     if boids.len() > 0 {
    //         average_position /= boids.len() as f32;
    //         average_forward /= boids.len() as f32;
    //
    //         for (_, mut position, _) in boids.iter_mut() {
    //             position.clone_from(&position.bound_to(average_position, bounds));
    //         }
    //
    //         for child in children.iter() {
    //             if let Ok((mut velocity, transform, params)) = child_query.get_mut(*child) {
    //                 let position = transform
    //                     .translation
    //                     .truncate()
    //                     .bound_to(average_position, bounds);
    //
    //                 let alignment = flock.alignment_strength
    //                     * Self::calculate_alignment(params.max_speed, average_forward);
    //                 let cohesion = flock.cohesion_strength
    //                     * Self::calculate_cohesion(position, average_position, flock.flock_radius);
    //                 let separation = flock.separation_strength
    //                     * Self::calculate_separation(child.id(), params, position, &boids);
    //
    //                 let mut acceleration: Vec2 =
    //                     params.max_speed * (alignment + cohesion + separation);
    //
    //                 if acceleration.length_squared() > params.max_accel * params.max_accel {
    //                     acceleration = acceleration.normalize() * params.max_accel;
    //                 }
    //
    //                 velocity.0 += acceleration * time.delta_seconds();
    //
    //                 if velocity.0.length_squared() > params.max_speed + params.max_speed {
    //                     velocity.0 = velocity.0.normalize() * params.max_speed;
    //                 }
    //             }
    //         }
    //     }
    // }
    Ok(CommandResult::Continue)
}
