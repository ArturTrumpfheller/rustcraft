// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched follow-up workload; the export tool runs this same harness on PR39's baseline.
#[path = "support/water_fixture.rs"]
mod fixture;
use std::time::Instant;
use voxel_core::{Block, BlockPos, fluid::FluidConfig};
fn main() {
    eprintln!(
        "WORLD_LAYOUT bytes={}",
        std::mem::size_of::<voxel_core::World>()
    );
    println!(
        "scene,trial,ticks,updates,washed_torches,water_cells,active_total_us,tick_p50_us,tick_p95_us,tick_max_us,idle_ns"
    );
    for scene in ["dry", "pool", "waterfall", "torches", "boundary"] {
        for trial in 1..=5 {
            let mut w = fixture::fixture(scene);
            if scene == "torches" {
                for [x, z] in [
                    [1, 0],
                    [-1, 0],
                    [0, 1],
                    [0, -1],
                    [2, 0],
                    [-2, 0],
                    [0, 2],
                    [0, -2],
                    [3, 0],
                    [-3, 0],
                    [0, 3],
                    [0, -3],
                ] {
                    w.set_block(BlockPos([x, 1, z]), Block::Torch).unwrap();
                }
            }
            fixture::add_water(&mut w, scene);
            let mut times = Vec::new();
            let mut changes = Vec::with_capacity(256);
            let mut washed = 0;
            for _ in 0..10000 {
                let start = Instant::now();
                w.tick_fluids(FluidConfig::default(), &mut changes);
                times.push(start.elapsed().as_secs_f64() * 1e6);
                washed += changes
                    .iter()
                    .filter(|c| c.before.is_torch() && c.after.fluid().is_some())
                    .count();
                if w.fluid_updates().pending() == 0 {
                    break;
                }
            }
            assert_eq!(w.fluid_updates().pending(), 0);
            let total: f64 = times.iter().sum();
            times.sort_by(f64::total_cmp);
            let start = Instant::now();
            for _ in 0..10000 {
                w.tick_fluids(FluidConfig::default(), &mut changes);
                std::hint::black_box(&w);
            }
            let idle = start.elapsed().as_nanos() / 10000;
            let n = times.len();
            println!(
                "{scene},{trial},{n},{},{washed},{},{total:.3},{:.3},{:.3},{:.3},{idle}",
                w.fluid_updates().total_processed,
                w.water_cells(),
                times[n / 2],
                times[n * 95 / 100],
                times[n - 1]
            );
        }
    }
    // COMPLETION_ONLY_BEGIN
    // Item creation is timed inside torch ticks above; local pickup is isolated here.
    let mut w = fixture::fixture("dry");
    let mut inv = voxel_core::inventory::Inventory::empty();
    let mut creation = Vec::new();
    let mut pickup = Vec::new();
    for _ in 0..1000 {
        let p = BlockPos([1, 1, 1]);
        w.set_block(p, Block::Torch).unwrap();
        let start = Instant::now();
        w.set_block(p, Block::WaterSource).unwrap();
        creation.push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        assert_eq!(w.collect_items([1.5, 1., 1.5], &mut inv), 1);
        pickup.push(start.elapsed().as_secs_f64() * 1e6);
    }
    creation.sort_by(f64::total_cmp);
    pickup.sort_by(f64::total_cmp);
    assert!(w.dropped_items().is_empty());
    eprintln!(
        "ITEM_METRIC repeats=1000 create_p50_us={:.3} create_p95_us={:.3} pickup_p50_us={:.3} pickup_p95_us={:.3}",
        creation[500], creation[950], pickup[500], pickup[950]
    );
    let p = BlockPos([1, 1, 1]);
    w.set_block(p, Block::Torch).unwrap();
    w.set_block(p, Block::WaterSource).unwrap();
    let mut full = voxel_core::inventory::Inventory::empty();
    full.insert(voxel_core::inventory::Item::Stone, 36 * 64);
    let start = Instant::now();
    for _ in 0..10000 {
        assert_eq!(w.collect_items([1.5, 1., 1.5], &mut full), 0);
    }
    eprintln!(
        "ITEM_FULL attempts=10000 mean_ns={} retained_drops={}",
        start.elapsed().as_nanos() / 10000,
        w.dropped_items().len()
    );
    // COMPLETION_ONLY_END
}
