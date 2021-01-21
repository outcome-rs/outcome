use outcome_core::machine::cmd::CommandResult;
use outcome_core::{
    arraystring::new_truncate, entity::Storage, entity::StorageIndex, machine::Result, EntityUid,
};

#[no_mangle]
pub fn minicall() -> u8 {
    3u8
}

#[no_mangle]
pub fn calculate_entity(ent_uid: &EntityUid, entity: &mut Storage) -> Result<CommandResult> {
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
