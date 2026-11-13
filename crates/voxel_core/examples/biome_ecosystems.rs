// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded full-resolution surface/vegetation observations, not estimated masks.
use voxel_core::{
    Block, BlockPos,
    worldgen::{Biome, GenerationConfig, Generator},
};
const SITES: [(&str, u64, i32, i32); 16] = [
    ("plains", 0, -3520, -4032),
    ("forest", 0, 2048, -4032),
    ("desert", 0, -2688, -3200),
    ("snowy-plains", 0, 384, -2752),
    ("rocky-highlands", 0, -3712, -1152),
    ("meadow", 0, 3328, -1856),
    ("birch-forest", 0, -1920, -4032),
    ("taiga", 0, 1600, -4032),
    ("grove-steep", 2026, 3520, 1728),
    ("grove", u64::MAX, -1616, -4032),
    ("snowy-slopes", 0, -960, 2496),
    ("stony-peaks", 42, 768, -896),
    ("beach", 42, 3328, -3264),
    ("ocean", 0, -2816, -4032),
    ("river", u64::MAX, -4032, 2112),
    ("lake", 0, 3200, 704),
];
fn main() {
    println!(
        "scene,seed,biome,columns,surface_cells,oak,birch,spruce,grass,ferns,red,yellow,bush,invalid_roots,invalid_plants,trees_in_treeline_fade,trees_above_fade,grass_on_steep,sand_on_highland,water_label_conflicts"
    );
    for (scene, seed, x, z) in SITES {
        let g = Generator::new(GenerationConfig { seed, version: 12 }).unwrap();
        let center = [x.div_euclid(16), z.div_euclid(16)];
        let mut counts = std::collections::BTreeMap::<u8, [u32; 16]>::new();
        for cz in center[1] - 3..=center[1] + 3 {
            for cx in center[0] - 3..=center[0] + 3 {
                let s = g.sampler([cx, cz]);
                for dz in 0..16 {
                    for dx in 0..16 {
                        let px = cx * 16 + dx;
                        let pz = cz * 16 + dz;
                        let sample = s.biome_sample(px, pz).unwrap();
                        let y = sample.height + 1;
                        let b = s.block(BlockPos([px, y, pz]));
                        let below = s.block(BlockPos([px, y - 1, pz]));
                        let counts = counts.entry(sample.biome as u8).or_default();
                        counts[0] += 1;
                        counts[13] += u32::from(below == Block::Grass && sample.slope >= 3);
                        counts[14] += u32::from(below == Block::Sand && sample.height >= 38);
                        let marine =
                            matches!(sample.biome, Biome::Ocean | Biome::River | Biome::Lake);
                        let actual_water = s.block(BlockPos([px, 2, pz])) == Block::WaterSource;
                        counts[15] += u32::from(marine != actual_water);
                        let kind = match b {
                            Block::Wood => Some(1),
                            Block::BirchWood => Some(2),
                            Block::SpruceWood => Some(3),
                            Block::TallGrass => Some(4),
                            Block::Fern => Some(5),
                            Block::RedFlower => Some(6),
                            Block::YellowFlower => Some(7),
                            Block::DeadBush => Some(8),
                            _ => None,
                        };
                        if let Some(kind) = kind {
                            counts[kind] += 1;
                            if kind <= 3 && !matches!(below, Block::Grass | Block::SnowyGround) {
                                counts[9] += 1;
                            }
                            if kind <= 3 {
                                counts[11] +=
                                    u32::from(sample.height >= sample.climate.treeline() - 4);
                                counts[12] +=
                                    u32::from(sample.height >= sample.climate.treeline() + 4);
                            }
                            if b.is_plant() && !b.supported_by(below) {
                                counts[10] += 1;
                            }
                        }
                    }
                }
            }
        }
        for (b, c) in counts {
            println!(
                "{scene},{seed},{b},49,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                c[0],
                c[1],
                c[2],
                c[3],
                c[4],
                c[5],
                c[6],
                c[7],
                c[8],
                c[9],
                c[10],
                c[11],
                c[12],
                c[13],
                c[14],
                c[15]
            );
        }
    }
}
