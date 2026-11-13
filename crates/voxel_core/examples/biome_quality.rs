// SPDX-License-Identifier: MIT OR Apache-2.0
//! Actual surface climate/biome survey. CSV/maps are diagnostics, not renders.
use voxel_core::worldgen::{GenerationConfig, Generator, biome_v2};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let step: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--step="))
        .map_or(32, |s| s.parse().unwrap());
    assert!([16, 32, 64].contains(&step));
    println!(
        "seed,region,x,z,biome,height,slope,temperature,humidity,continentalness,erosion,weirdness,snowline,treeline,top,elevation_q10"
    );
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 12 }).unwrap();
        for (region, ox, oz, size) in [
            ("central", -4096, -4096, 8192),
            ("distant-west", -24000, 20000, 2048),
            ("distant-east", 28000, -30000, 2048),
        ] {
            for z in (oz..oz + size).step_by(step) {
                for x in (ox..ox + size).step_by(step) {
                    let s = biome_v2::sample(&g, x, z).unwrap();
                    let c = s.climate;
                    println!(
                        "{seed},{region},{x},{z},{},{},{},{},{},{},{},{},{},{},{},{}",
                        s.biome as u8,
                        s.height,
                        s.slope,
                        c.temperature,
                        c.humidity,
                        c.continentalness,
                        c.erosion,
                        c.weirdness,
                        c.snowline(),
                        c.treeline(),
                        s.top as u8,
                        c.elevation
                    );
                }
            }
        }
    }
}
