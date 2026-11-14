// SPDX-License-Identifier: MIT OR Apache-2.0
//! Frozen column/biome baseline. Optional VERSION selects one supported generator.
use voxel_core::{
    ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator},
};
fn main() {
    println!("version,seed,cx,cz,crc,biome");
    let requested = std::env::args()
        .nth(1)
        .map(|v| v.parse::<u32>().expect("VERSION"));
    for version in requested.map_or(1..=13, |v| v..=v) {
        for seed in [0, 42, 2026, u64::MAX] {
            let g = Generator::new(GenerationConfig { seed, version }).unwrap();
            let mut sites = if version < 3 {
                vec![[0, 0], [-1, -1]]
            } else {
                vec![
                    [0, 0],
                    [-1, -1],
                    [-1511, 1250],
                    [2047, -2048],
                    [-249, -197],
                    [208, -204],
                ]
            };
            if version == 14 {
                sites.extend([[-12, -12], [-45, -36]]);
            }
            for c in sites {
                let s = g.sampler(c);
                let mut crc = crc32fast::Hasher::new();
                for y in g.world_height().chunks() {
                    let ch = s.chunk(ChunkPos([c[0], y, c[1]])).unwrap();
                    for i in 0..4096 {
                        crc.update(
                            &(ch.get(LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap())
                                as u16)
                                .to_le_bytes(),
                        );
                    }
                }
                println!(
                    "{version},{seed},{},{},{},{}",
                    c[0],
                    c[1],
                    crc.finalize(),
                    g.biome(c[0] * 16 + 8, c[1] * 16 + 8)
                        .map_or(255, |b| b as u8)
                );
            }
        }
    }
}
