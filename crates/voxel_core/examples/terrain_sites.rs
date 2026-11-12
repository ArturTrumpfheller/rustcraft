// SPDX-License-Identifier: MIT OR Apache-2.0
use voxel_core::worldgen::{GenerationConfig, Generator, terrain_v3};
fn main() {
    println!("seed,x,z,raw,final,biome");
    for (seed, x, z) in [
        (42, -2880, 720),
        (42, -2880, 592),
        (42, -2880, 848),
        (0, -3984, -3152),
        (42, -3664, -2704),
        (2026, 112, 3008),
        (42, -144, 752),
        (0, -2176, -4096),
        (2026, -2560, -4096),
        (0, -1936, 176),
    ] {
        let g = Generator::new(GenerationConfig { seed, version: 10 }).unwrap();
        println!(
            "{seed},{x},{z},{},{},{:?}",
            terrain_v3::height(seed, x, z),
            g.height(x, z).unwrap(),
            g.biome(x, z)
        );
    }
}
