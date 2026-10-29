// SPDX-License-Identifier: MIT OR Apache-2.0
//! Offline reproducible broad survey; never invoked by client startup.
use std::collections::BTreeMap;
use voxel_core::worldgen::{Biome, GenerationConfig, Generator, sample_biome};
fn main() {
    println!("seed,biome,samples,site_x,site_z,site_height");
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 5 }).unwrap();
        let mut counts = BTreeMap::new();
        let mut sites = BTreeMap::new();
        for z in (-4096..4096).step_by(32) {
            for x in (-4096..4096).step_by(32) {
                let b = sample_biome(seed, x, z).unwrap();
                *counts.entry(b).or_insert(0) += 1;
                if !sites.contains_key(&b)
                    && [-48, 0, 48].into_iter().all(|dx| {
                        [-48, 0, 48]
                            .into_iter()
                            .all(|dz| sample_biome(seed, x + dx, z + dz) == Some(b))
                    })
                {
                    sites.insert(b, [x, z]);
                }
            }
        }
        for b in [
            Biome::Plains,
            Biome::Forest,
            Biome::Desert,
            Biome::SnowyPlains,
            Biome::RockyHighlands,
        ] {
            let [x, z] = sites[&b];
            println!(
                "{seed},{b:?},{},{x},{z},{}",
                counts[&b],
                g.height(x, z).unwrap()
            );
        }
        let mut hash = crc32fast::Hasher::new();
        for c in [
            [0, 0],
            [-1, 3],
            [4, -4],
            [120, -120],
            [1000, -1000],
            [2047, -2048],
        ] {
            let s = g.sampler(c);
            for cy in -1..2 {
                let p = voxel_core::ChunkPos([c[0], cy, c[1]]);
                let chunk = s.chunk(p).unwrap();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let b = chunk.get(voxel_core::LocalPos::new([x, y, z]).unwrap());
                            hash.update(&[match b {
                                voxel_core::Block::Air => 0,
                                voxel_core::Block::Grass => 1,
                                voxel_core::Block::Dirt => 2,
                                voxel_core::Block::Stone => 3,
                                voxel_core::Block::Wood => 4,
                                voxel_core::Block::Leaves => 5,
                                voxel_core::Block::Sand => 20,
                                voxel_core::Block::SnowyGround => 21,
                                _ => panic!("unexpected generated block"),
                            }]);
                        }
                    }
                }
            }
        }
        eprintln!("GOLDEN {seed} {:#010x}", hash.finalize());
        if seed == 42 {
            'site: for z in (-4096..4096).step_by(32) {
                for x in (-4096..4096).step_by(32) {
                    let c = voxel_core::worldgen::sample_climate(seed, x, z).unwrap();
                    if c.temperature > 200 && c.geography > 500 && g.height(x, z).unwrap() >= 15 {
                        eprintln!(
                            "WARM_HIGHLANDS {seed} x={x} z={z} height={}",
                            g.height(x, z).unwrap()
                        );
                        break 'site;
                    }
                }
            }
        }
        // Locate east/west boundaries with two substantial opposing interiors.
        for (a, b) in [
            (Biome::Plains, Biome::Forest),
            (Biome::Plains, Biome::Desert),
            (Biome::Plains, Biome::SnowyPlains),
        ] {
            'search: for z in (-3000..3000).step_by(16) {
                for x in (-3000..3000).step_by(16) {
                    if sample_biome(seed, x - 32, z) == Some(a)
                        && sample_biome(seed, x + 32, z) == Some(b)
                    {
                        eprintln!("BOUNDARY {seed} {a:?}/{b:?} x={x} z={z} yaw=90");
                        break 'search;
                    }
                }
            }
        }
    }
}
