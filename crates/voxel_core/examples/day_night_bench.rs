// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched Slice20/21 CPU fixture; intentionally uses only the common API.
use std::{collections::BTreeSet, hint::black_box, time::Instant};
use voxel_core::{Block, BlockPos, ChunkPos, World, lighting::Skylight, mesh_chunk_lit};
fn main() {
    println!("radius,scene,light_us,mesh_us,quads,mesh_bytes,light_bytes,cache_bytes");
    for radius in 1..=3 {
        for torches in [false, true] {
            let mut world = World::default();
            let generator =
                voxel_core::worldgen::Generator::new(voxel_core::worldgen::GenerationConfig {
                    seed: 2026,
                    version: 3,
                })
                .unwrap();
            let mut targets = BTreeSet::new();
            for z in -radius - 1..=radius + 1 {
                for x in -radius - 1..=radius + 1 {
                    let sampler = generator.sampler([x, z]);
                    for y in -1..=1 {
                        let p = ChunkPos([x, y, z]);
                        world.insert_chunk(p, sampler.chunk(p).unwrap());
                        if x.abs() <= radius && z.abs() <= radius {
                            targets.insert(p);
                        }
                    }
                }
            }
            if torches {
                for x in [-6, 0, 6] {
                    let y = (-15..31)
                        .rev()
                        .find(|&y| world.block(BlockPos([x, y, 0])).is_cube())
                        .unwrap();
                    world
                        .set_block(BlockPos([x, y + 1, 0]), Block::Torch)
                        .unwrap();
                }
            }
            let mut light_times = Vec::new();
            let mut mesh_times = Vec::new();
            let mut quads = 0;
            let mut bytes = 0;
            let mut cache_bytes = 0;
            for n in 0..15 {
                let start = Instant::now();
                let light = black_box(Skylight::for_chunks(black_box(&world), &targets));
                let light_us = start.elapsed().as_secs_f64() * 1e6;
                bytes = light.bytes();
                cache_bytes = targets
                    .iter()
                    .map(|&p| light.chunk(p).0.len())
                    .sum::<usize>();
                let start = Instant::now();
                quads = targets
                    .iter()
                    .map(|&p| black_box(mesh_chunk_lit(&world, p, &light)).indices.len() / 6)
                    .sum::<usize>();
                if n > 1 {
                    light_times.push(light_us);
                    mesh_times.push(start.elapsed().as_secs_f64() * 1e6);
                }
            }
            light_times.sort_by(f64::total_cmp);
            mesh_times.sort_by(f64::total_cmp);
            println!(
                "{radius},{},{:.2},{:.2},{quads},{},{bytes},{cache_bytes}",
                if torches { "torches" } else { "natural" },
                light_times[6],
                mesh_times[6],
                quads * 248
            );
        }
    }
}
