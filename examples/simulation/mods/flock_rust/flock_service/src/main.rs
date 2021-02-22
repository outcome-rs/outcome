#![allow(unused)]

use anyhow::Result;

use outcome_core::machine::cmd::CommandResult;
use outcome_core::{arraystring::new_truncate, entity::Storage, entity::StorageIndex, EntityId};
use outcome_net::msg::{
    DataPullRequest, DataTransferRequest, DataTransferResponse, MessageType, PullRequestData,
    TransferResponseData, TurnAdvanceRequest, TurnAdvanceResponse, TypedSimDataPack,
    VarSimDataPack, VarSimDataPackOrdered,
};
use outcome_net::{Client, ClientConfig, CompressionPolicy, SocketEvent};

pub fn main() -> Result<()> {
    // let mut client = Client::new("flock_service", true, false, None, Some(1000))?;
    let mut client = Client::new_with_config(
        None,
        ClientConfig {
            name: "flock_service".to_string(),
            heartbeat: None,
            is_blocking: true,
            ..Default::default()
        },
    )?;
    client.connect("127.0.0.1:9123".to_string(), None);

    let mut advanced_turn = true;
    let mut received_data = true;

    let mut data = VarSimDataPackOrdered::default();
    let mut order_id = None;

    client
        .connection
        .pack_send_msg_payload(TurnAdvanceRequest { tick_count: 1 }, None)?;
    client.connection.pack_send_msg_payload(
        DataTransferRequest {
            transfer_type: "SelectVarOrdered".to_string(),
            selection: vec!["4:velocity:float:x".to_string()],
            // selection: vec![],
        },
        None,
    )?;

    loop {
        // println!("loop");
        if advanced_turn && received_data && order_id.is_some() {
            // println!("{:?}", data);
            // for (addr, var) in data
            // .vars
            //     .iter_mut()
            //     .filter(|(a, b)| a.contains(":velocity:float:x"))
            for var in &mut data.vars {
                if let Ok(v) = var.as_float_mut() {
                    *v += 1.;
                }
            }

            client.connection.pack_send_msg_payload(
                DataPullRequest {
                    data: PullRequestData::VarOrdered(order_id.unwrap(), data.clone()),
                },
                None,
            )?;
            client
                .connection
                .pack_send_msg_payload(TurnAdvanceRequest { tick_count: 1 }, None)?;
            client
                .connection
                .pack_send_msg_payload(data_transfer_request(), None)?;
            advanced_turn = false;
            received_data = false;
        }

        loop {
            if let Ok((addr, event)) = client.connection.try_recv() {
                match event {
                    SocketEvent::Message(msg) => match msg.type_ {
                        MessageType::TurnAdvanceResponse => {
                            let resp: TurnAdvanceResponse =
                                msg.unpack_payload(client.connection.encoding())?;
                            if resp.error.is_empty() {
                                // println!("[{:?}] advanced turn", std::time::SystemTime::now());
                                advanced_turn = true;
                            } else {
                                // println!("{}", resp.error);
                                client.connection.pack_send_msg_payload(
                                    TurnAdvanceRequest { tick_count: 1 },
                                    None,
                                )?;
                            }
                        }
                        MessageType::DataTransferResponse => {
                            println!("received data transfer response");
                            let resp: DataTransferResponse =
                                msg.unpack_payload(client.connection.encoding())?;
                            if let Some(resp_data) = resp.data {
                                match resp_data {
                                    TransferResponseData::VarOrdered(ord_id, d) => {
                                        order_id = Some(ord_id);
                                        data = d
                                    }
                                    _ => (),
                                }
                            }
                            received_data = true;
                        }
                        MessageType::DataPullResponse => {
                            println!("received pull response");
                        }
                        _ => (),
                    },
                    SocketEvent::Disconnect => {
                        println!("server disconnected");
                        return Ok(());
                    }
                    SocketEvent::Heartbeat => (),
                    _ => println!("unhandled event: {:?}", event),
                }
            } else {
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(1));

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
