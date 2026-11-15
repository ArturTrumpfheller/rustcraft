// SPDX-License-Identifier: MIT OR Apache-2.0
//! Explicit post-design v14 snapshot refresh; never rewrites historical files.
#[allow(dead_code)]
#[path = "support/worldgen_audit.rs"]
mod audit;
use voxel_core::worldgen::{GenerationConfig, Generator};
fn main() {
    let version = std::env::args()
        .nth(1)
        .map_or(14, |v| v.parse::<u32>().unwrap());
    assert!(matches!(version, 14..=16));
    println!("version,seed,chunk_x,chunk_z,crc,biome");
    for line in include_str!("../tests/fixtures/columns-v14.csv")
        .lines()
        .skip(1)
    {
        let r: Vec<_> = line.split(',').collect();
        let seed = r[1].parse().unwrap();
        let c = [r[2].parse().unwrap(), r[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        println!(
            "{version},{seed},{},{},{},{}",
            c[0],
            c[1],
            audit::crc(&audit::column(&g, c)),
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8).unwrap() as u8
        );
    }
}
