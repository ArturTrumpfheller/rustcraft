// SPDX-License-Identifier: MIT OR Apache-2.0
//! Macro geography grid, actual one-block slopes and representative sites.
//! CSV is diagnostic data, never presented as a client capture.
use voxel_core::worldgen::{GenerationConfig, Generator, terrain_v3, terrain_v4};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let step: usize = args
        .iter()
        .find_map(|v| v.strip_prefix("--step="))
        .map_or(16, |v| v.parse().unwrap());
    let version: u32 = args
        .iter()
        .find_map(|v| v.strip_prefix("--generator="))
        .map_or(10, |v| v.parse().unwrap());
    assert!(matches!(step, 16 | 32 | 64) && matches!(version, 9..=11));
    let final_surface = args.iter().any(|v| v == "--final");
    println!("seed,x,z,height,slope,east16,north16");
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let height = |seed, x, z| {
            if final_surface {
                g.height(x, z).unwrap()
            } else if version == 11 {
                terrain_v4::height(seed, x, z)
            } else {
                terrain_v3::height(seed, x, z)
            }
        };
        for (ox, oz, size) in [
            (-4096, -4096, 8192),
            (-24000, 20000, 2048),
            (28000, -30000, 2048),
        ] {
            for z in (oz..oz + size).step_by(step) {
                for x in (ox..ox + size).step_by(step) {
                    let h = height(seed, x, z);
                    let slope = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                        .into_iter()
                        .map(|[dx, dz]| (height(seed, x + dx, z + dz) - h).abs())
                        .max()
                        .unwrap();
                    println!(
                        "{seed},{x},{z},{h},{slope},{},{}",
                        height(seed, x + 16, z),
                        height(seed, x, z + 16)
                    );
                }
            }
        }
    }
}
