// SPDX-License-Identifier: MIT OR Apache-2.0
//! Repeatable CPU fluid workload. Release recommended; no renderer involved.
#[path = "support/water_fixture.rs"]
mod fixture;
use std::{collections::BTreeSet, time::Instant};
use voxel_core::{fluid::FluidConfig, lighting::Skylight, mesh_chunk_lit};
fn main() {
    println!(
        "scene,water_cells,ticks,updates,tick_p50_us,tick_p95_us,tick_max_us,idle_ns,terrain_meshes,water_meshes,mesh_bytes,mesh_ms"
    );
    for scene in ["dry", "pool", "waterfall", "several", "boundary"] {
        let terrain = fixture::fixture(scene);
        let mut w = if std::env::args().any(|a| a == "--regional") {
            let mut w = voxel_core::World::regional(voxel_core::regions::RegionIdentity {
                seed: 42,
                generator: 3,
                nonce: [22; 16],
            });
            for (p, chunk) in terrain.chunks() {
                let region = voxel_core::regions::RegionPos::of(
                    p.block_position(voxel_core::LocalPos::new([0; 3]).unwrap())
                        .unwrap(),
                );
                w.install_region(
                    region,
                    std::sync::Arc::new(std::collections::BTreeMap::new()),
                );
                w.insert_chunk(p, chunk.clone());
            }
            w
        } else {
            terrain
        };
        fixture::add_water(&mut w, scene);
        let mut changes = Vec::with_capacity(256);
        let mut ticks = Vec::new();
        for _ in 0..10000 {
            let start = Instant::now();
            w.tick_fluids(FluidConfig::default(), &mut changes);
            ticks.push(start.elapsed().as_secs_f64() * 1e6);
            if w.fluid_updates().pending() == 0 {
                break;
            }
        }
        assert_eq!(w.fluid_updates().pending(), 0);
        ticks.sort_by(f64::total_cmp);
        let n = ticks.len();
        let start = Instant::now();
        for _ in 0..10000 {
            w.tick_fluids(FluidConfig::default(), &mut changes);
            std::hint::black_box(&w);
        }
        let idle = start.elapsed().as_nanos() / 10000;
        let targets: BTreeSet<_> = w
            .chunks()
            .map(|(p, _)| p)
            .filter(|p| p.0[0].abs() <= 2 && p.0[2].abs() <= 2)
            .collect();
        let light = Skylight::for_chunks(&w, &targets);
        let start = Instant::now();
        let (mut terrain, mut water, mut bytes) = (0, 0, 0);
        for &p in &targets {
            let m = mesh_chunk_lit(&w, p, &light);
            terrain += usize::from(!m.indices.is_empty());
            water += usize::from(!m.water.indices.is_empty());
            bytes += m.positions.len() * 56
                + m.indices.len() * 4
                + m.water.positions.len() * 48
                + m.water.indices.len() * 4;
        }
        let mesh_ms = start.elapsed().as_secs_f64() * 1000.;
        println!(
            "{scene},{},{n},{},{:.3},{:.3},{:.3},{idle},{terrain},{water},{bytes},{mesh_ms:.3}",
            w.water_cells(),
            w.fluid_updates().total_processed,
            ticks[n / 2],
            ticks[n * 95 / 100],
            ticks[n - 1]
        );
    }
}
