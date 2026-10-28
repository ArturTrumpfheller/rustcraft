// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched Slice22/23 core benchmark: copy this unchanged into the baseline.
#[path = "support/water_fixture.rs"]
mod fixture;
use std::time::Instant;
use voxel_core::{Block, BlockPos, World, fluid::FluidConfig};

fn edit(w: &mut World, p: [i32; 3], b: Block) {
    w.set_block(BlockPos(p), b).unwrap();
}
fn scene(name: &str) -> World {
    let base = match name {
        "settled" => "pool",
        "obstacles" | "holes" | "drain-edits" | "waterfall" => "flat",
        other => other,
    };
    let mut w = fixture::fixture(base);
    if name == "obstacles" {
        for z in -3..=2 {
            edit(&mut w, [1, 1, z], Block::Stone);
        }
        edit(&mut w, [3, 0, 2], Block::Air);
    }
    if name == "holes" {
        for x in [-3, 2] {
            edit(&mut w, [x, 0, 0], Block::Air);
        }
    }
    if name == "boundary" {
        edit(&mut w, [18, 0, 0], Block::Air);
    }
    if name == "waterfall" {
        for z in -6..=6 {
            edit(&mut w, [0, 25, z], Block::WaterSource);
        }
    } else {
        fixture::add_water(&mut w, base);
    }
    w
}
#[derive(Default)]
struct Measurements {
    ticks: Vec<f64>,
    processed: u64,
    peak: usize,
}
fn settle(w: &mut World, changes: &mut Vec<voxel_core::BlockChange>, m: &mut Measurements) {
    for _ in 0..10000 {
        m.peak = m.peak.max(w.fluid_updates().pending());
        let start = Instant::now();
        w.tick_fluids(FluidConfig::default(), changes);
        m.ticks.push(start.elapsed().as_secs_f64() * 1e6);
        m.processed += w.fluid_updates().processed as u64;
        if w.fluid_updates().pending() == 0 {
            return;
        }
    }
    panic!("fluid did not settle");
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let repeats = args
        .iter()
        .find_map(|a| a.strip_prefix("--repeats="))
        .map_or(9, |s| s.parse::<usize>().unwrap());
    let only = args.iter().find_map(|a| a.strip_prefix("--scene="));
    println!(
        "scene,repeat,ticks,processed,pending_peak,pending_final,tick_total_us,tick_p50_us,tick_p95_us,tick_max_us,event_us,idle_ns,water_cells"
    );
    for name in [
        "dry",
        "pool",
        "several",
        "waterfall",
        "obstacles",
        "holes",
        "drain-edits",
        "boundary",
        "settled",
    ] {
        if only.is_some_and(|s| s != name) {
            continue;
        }
        for repeat in 0..repeats {
            let mut w = scene(name);
            let mut changes = Vec::with_capacity(256);
            let mut m = Measurements::default();
            if name == "settled" {
                settle(&mut w, &mut changes, &mut Measurements::default());
            }
            settle(&mut w, &mut changes, &mut m);
            let mut event_us = 0.;
            if name == "drain-edits" {
                for _ in 0..4 {
                    for block in [Block::Air, Block::Grass] {
                        let start = Instant::now();
                        edit(&mut w, [4, 0, 0], block);
                        event_us += start.elapsed().as_secs_f64() * 1e6;
                        settle(&mut w, &mut changes, &mut m);
                    }
                }
            }
            let start = Instant::now();
            for _ in 0..10000 {
                w.tick_fluids(FluidConfig::default(), &mut changes);
                std::hint::black_box(&w);
            }
            let idle = start.elapsed().as_nanos() / 10000;
            assert_eq!(w.fluid_updates().pending(), 0);
            assert_eq!(w.fluid_updates().processed, 0);
            let total: f64 = m.ticks.iter().sum();
            m.ticks.sort_by(f64::total_cmp);
            let n = m.ticks.len();
            println!(
                "{name},{repeat},{n},{},{},0,{total:.3},{:.3},{:.3},{:.3},{event_us:.3},{idle},{}",
                m.processed,
                m.peak,
                m.ticks[n / 2],
                m.ticks[n * 95 / 100],
                m.ticks[n - 1],
                w.water_cells()
            );
        }
    }
}
