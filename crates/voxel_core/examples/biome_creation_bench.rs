// SPDX-License-Identifier: MIT OR Apache-2.0
//! Real catalog creation, persistence, load and initial spawn; no user saves.
use std::{
    hint::black_box,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use voxel_core::{
    persistence::catalog::WorldCatalog,
    physics::{PhysicsConfig, spawn_position},
    streaming,
};
fn percentile(values: &mut [f64], p: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() - 1) * p / 100]
}
fn main() {
    let version: u32 = std::env::args().nth(1).unwrap().parse().unwrap();
    let directory = std::env::temp_dir().join(format!(
        "rustcraft-biome-creation-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let catalog = WorldCatalog::new(directory.clone()).unwrap();
    println!("generator,seed,creation_p50_us,creation_p95_us,creation_p99_us");
    for seed in [0, 42, 2026] {
        let mut times = Vec::with_capacity(30);
        for i in 0..33 {
            let start = Instant::now();
            let id = catalog.create("Biome benchmark", seed).unwrap();
            let (store, mut world, _) = catalog.open(&id).unwrap();
            let inventory = store.load_player(&mut world).unwrap();
            assert_eq!(world.identity().unwrap().generator, version);
            store.prepare_regional_spawn(&mut world).unwrap();
            streaming::prepare_column(&mut world, [0, 0]).unwrap();
            black_box(spawn_position(
                &world,
                &PhysicsConfig::default(),
                [0.5, 28., 10.5],
            ));
            black_box(inventory);
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            drop(store);
            if i >= 3 {
                times.push(elapsed);
            }
        }
        println!(
            "{version},{seed},{:.2},{:.2},{:.2}",
            percentile(&mut times, 50),
            percentile(&mut times, 95),
            percentile(&mut times, 99)
        );
    }
    // This unique directory is created and owned exclusively by this harness.
    std::fs::remove_dir_all(directory).unwrap();
}
