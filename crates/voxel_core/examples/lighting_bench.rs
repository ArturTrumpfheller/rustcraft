// SPDX-License-Identifier: MIT OR Apache-2.0
#[path = "support/lighting_fixture.rs"]
mod lighting_fixture;
use std::{collections::BTreeSet, hint::black_box, time::Instant};
use voxel_core::{Block, BlockPos, lighting::Skylight, mesh_chunk_lit};
fn main() {
    println!("scene,light_us,mesh_us,chunks,quads,channel_bytes,block_steps");
    for scene in ["dark", "one", "several", "many", "edits", "seam", "mounts"] {
        let mut w = lighting_fixture::room(if scene == "edits" { "one" } else { scene });
        let targets: BTreeSet<_> = w.chunks().map(|(p, _)| p).collect();
        let mut light_times = Vec::new();
        let mut mesh_times = Vec::new();
        let mut quads = 0;
        let mut bytes = 0;
        let mut steps = 0;
        for sample in 0..7 {
            let mut light_us = 0.;
            let mut mesh_us = 0.;
            for n in 0..20 {
                if scene == "edits" {
                    w.set_block(
                        BlockPos([-2, 1, -3]),
                        if n % 2 == 0 { Block::Air } else { Block::Torch },
                    )
                    .unwrap();
                }
                let t = Instant::now();
                let light = black_box(Skylight::for_chunks(black_box(&w), &targets));
                light_us += t.elapsed().as_secs_f64() * 1e6;
                bytes = light.bytes();
                steps = light.block_propagated;
                let t = Instant::now();
                quads = targets
                    .iter()
                    .map(|&p| black_box(mesh_chunk_lit(&w, p, &light)).indices.len() / 6)
                    .sum();
                mesh_us += t.elapsed().as_secs_f64() * 1e6;
            }
            if sample > 0 {
                light_times.push(light_us / 20.);
                mesh_times.push(mesh_us / 20.);
            }
        }
        light_times.sort_by(f64::total_cmp);
        mesh_times.sort_by(f64::total_cmp);
        println!(
            "{scene},{:.2},{:.2},{},{quads},{bytes},{steps}",
            light_times[3],
            mesh_times[3],
            targets.len()
        );
    }
}
