// SPDX-License-Identifier: MIT OR Apache-2.0
//! Read-only deterministic site finder. CSV coordinates are chunk origins.
use voxel_core::{
    Block, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator, sample_biome},
};
fn main() {
    println!("seed,x,z,biome,grass,fern,red,yellow,bush,oak,birch,spruce");
    for seed in [0, 42, 2026] {
        let g = Generator::new(GenerationConfig { seed, version: 9 }).unwrap();
        let mut found = std::collections::BTreeMap::new();
        for z in (-4096..4096).step_by(64) {
            for x in (-4096..4096).step_by(64) {
                let biome = sample_biome(seed, x, z).unwrap();
                if found.get(&biome).copied().unwrap_or(0) >= 8 {
                    continue;
                }
                if voxel_core::worldgen::hydrology::surface_tile(seed, [x / 16, z / 16])
                    .iter()
                    .filter(|s| s.depth() > 0)
                    .count()
                    > 24
                {
                    continue;
                }
                let s = g.sampler([x / 16, z / 16]);
                let mut counts = [0; 8];
                for y in 0..3 {
                    let c = s.chunk(ChunkPos([x / 16, y, z / 16])).unwrap();
                    for ly in 0..16 {
                        for lz in 0..16 {
                            for lx in 0..16 {
                                let b = c.get(LocalPos::new([lx, ly, lz]).unwrap());
                                let i = match b {
                                    Block::TallGrass => 0,
                                    Block::Fern => 1,
                                    Block::RedFlower => 2,
                                    Block::YellowFlower => 3,
                                    Block::DeadBush => 4,
                                    Block::Wood => 5,
                                    Block::BirchWood => 6,
                                    Block::SpruceWood => 7,
                                    _ => continue,
                                };
                                counts[i] += 1;
                                if i >= 5 && std::env::args().any(|a| a == "--anchors") {
                                    let p = voxel_core::BlockPos([
                                        x + lx as i32,
                                        y * 16 + ly as i32,
                                        z + lz as i32,
                                    ]);
                                    if matches!(
                                        s.block(p.offset([0, -1, 0]).unwrap()),
                                        Block::Grass | Block::SnowyGround
                                    ) {
                                        eprintln!(
                                            "ANCHOR,{seed},{},{},{},{b:?}",
                                            p.0[0], p.0[1], p.0[2]
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                let empty = std::env::args().any(|a| a == "--empty");
                if (counts.iter().sum::<usize>() == 0) != empty {
                    continue;
                }
                println!(
                    "{seed},{x},{z},{},{},{},{},{},{},{},{},{}",
                    biome.name(),
                    counts[0],
                    counts[1],
                    counts[2],
                    counts[3],
                    counts[4],
                    counts[5],
                    counts[6],
                    counts[7]
                );
                *found.entry(biome).or_insert(0) += 1;
            }
        }
    }
}
