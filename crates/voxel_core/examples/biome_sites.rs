// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded genuine Grove root search for camera siting, not benchmark selection.
//! biome_sites work/quality12.csv; retains every inspected candidate in CSV.
use std::{collections::BTreeMap, io::BufRead};
use voxel_core::{
    Block, BlockPos,
    worldgen::{Biome, GenerationConfig, Generator},
};
fn main() {
    let path = std::env::args().nth(1).expect("surface survey CSV");
    let meadow = std::env::args().nth(2).as_deref() == Some("meadow");
    let target_id = if meadow { "5" } else { "8" };
    let target = if meadow { Biome::Meadow } else { Biome::Grove };
    let mut inspected = BTreeMap::<u64, u32>::new();
    let mut candidates = Vec::new();
    let mut generators = BTreeMap::new();
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .skip(1)
    {
        let line = line.unwrap();
        let v: Vec<_> = line.split(',').collect();
        if v[4] != target_id
            || v[6].parse::<i32>().unwrap() > 1
            || v[8].parse::<i32>().unwrap() < 80
            || v[5].parse::<i32>().unwrap() + 6 > v[13].parse::<i32>().unwrap()
        {
            continue;
        }
        let seed = v[0].parse::<u64>().unwrap();
        let n = inspected.entry(seed).or_default();
        if *n >= 32 {
            continue;
        }
        *n += 1;
        let x = v[2].parse::<i32>().unwrap();
        let z = v[3].parse::<i32>().unwrap();
        let c = [x.div_euclid(16), z.div_euclid(16)];
        let g = generators
            .entry(seed)
            .or_insert_with(|| Generator::new(GenerationConfig { seed, version: 12 }).unwrap());
        let (mut grove, mut spruce) = (0, 0);
        for cz in c[1] - 1..=c[1] + 1 {
            for cx in c[0] - 1..=c[0] + 1 {
                let s = g.sampler([cx, cz]);
                for i in 0..256 {
                    let px = cx * 16 + i % 16;
                    let pz = cz * 16 + i / 16;
                    let b = s.biome_sample(px, pz).unwrap();
                    if b.biome == target {
                        grove += 1;
                        let block = s.block(BlockPos([px, b.height + 1, pz]));
                        spruce += i32::from(if meadow {
                            matches!(block, Block::RedFlower | Block::YellowFlower)
                        } else {
                            block == Block::SpruceWood
                        });
                    }
                }
            }
        }
        candidates.push((spruce, grove, seed, x, z));
    }
    candidates.sort_by(|a, b| b.cmp(a));
    println!("seed,x,z,target_surface_cells,feature_count");
    for (spruce, grove, seed, x, z) in candidates {
        println!("{seed},{x},{z},{grove},{spruce}");
    }
}
