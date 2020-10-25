use criterion::{black_box, criterion_group, criterion_main, Criterion};
use outcome_core::model::EntityPrefabModel;
use outcome_core::{ShortString, Sim, SimModel, StringId};

const SCENARIO_PATH: &'static str = "../scenarios/barebones";

criterion_group!(barebones, add_entity, step, sim_from_scenario_at);
criterion_main!(barebones);

fn add_entity(c: &mut Criterion) {
    let mut sim = Sim::from_scenario_at(SCENARIO_PATH).unwrap();
    sim.model.entities.push(EntityPrefabModel {
        name: ShortString::from("bench_ent").unwrap(),
        components: vec![],
    });

    c.bench_function("add_entity_100", |b| {
        b.iter(|| {
            for n in 0..100 {
                sim.spawn_entity(
                    &StringId::from("bench_ent").unwrap(),
                    &StringId::from(&format!("ent_{}", n)).unwrap(),
                );
            }
        })
    });
}

fn step(c: &mut Criterion) {
    let mut sim = Sim::from_scenario_at(SCENARIO_PATH).unwrap();
    c.bench_function("step_1", |b| b.iter(|| black_box(sim.step().unwrap())));
}

fn sim_from_scenario_at(c: &mut Criterion) {
    let mut sim = Sim::from_scenario_at(SCENARIO_PATH).unwrap();
    c.bench_function("sim_from_scenario_at", |b| {
        b.iter(|| black_box(Sim::from_scenario_at(SCENARIO_PATH).unwrap()))
    });
}
