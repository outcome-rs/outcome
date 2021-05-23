//! Measurements of the local `Sim` interface.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use outcome_core::model::{ComponentModel, EntityPrefab, Scenario, VarModel};
use outcome_core::{string, Sim, SimModel, StringId, Var, VarType};

criterion_group!(sim, spawn_entities);
criterion_main!(sim);

/// Measures how much time does it take to spawn a 1000 entities.
fn spawn_entities(c: &mut Criterion) {
    let mut sim = Sim::new();

    let mut comp_model = ComponentModel::default();
    comp_model.name = string::new_truncate("bench_comp");
    comp_model.vars.push(VarModel {
        name: string::new_truncate("id"),
        type_: VarType::Int,
        default: Some(Var::Int(42)),
    });
    sim.model.components.push(comp_model);
    sim.model.entities.push(EntityPrefab {
        name: string::new_truncate("bench_ent"),
        components: vec![string::new_truncate("bench_comp")],
    });

    println!("once");

    c.bench_function("spawn_entities_1000", |b| {
        b.iter(|| {
            for n in 0..1000 {
                sim.spawn_entity(
                    Some(&string::new_truncate("bench_ent")),
                    // None,
                    Some(string::new_truncate(&format!("ent_{}", n))),
                )
                .expect("failed spawning entity");
            }
            sim.entities.clear();
            sim.entity_idx.clear();
        })
    });
}
