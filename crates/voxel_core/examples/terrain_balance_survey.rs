// SPDX-License-Identifier: MIT OR Apache-2.0
//! Offline geographical sampling. Raw density surface by default; --final includes water beds/ecology.
use voxel_core::worldgen::{GenerationConfig, Generator, biome_v2, terrain_v5};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let arg = |name: &str, default: &str| {
        args.iter()
            .find_map(|s| s.strip_prefix(name))
            .unwrap_or(default)
            .to_owned()
    };
    let version = arg("--generator=", "12").parse().unwrap();
    let step: usize = arg("--step=", "16").parse().unwrap();
    assert!(matches!(step, 16 | 32 | 64) && matches!(version, 12 | 13));
    let final_surface = args.iter().any(|v| v == "--final");
    println!("seed,region,x,z,height,slope,c,e,w,offset,amplitude,biome");
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        for (region, (ox, oz, size)) in [
            (-4096, -4096, 8192),
            (-24000, 20000, 2048),
            (28000, -30000, 2048),
        ]
        .into_iter()
        .enumerate()
        {
            for z in (oz..oz + size).step_by(step) {
                for x in (ox..ox + size).step_by(step) {
                    let p = terrain_v5::parameters_for(version, seed, x, z);
                    let height = |px, pz| {
                        if final_surface {
                            g.height(px, pz).unwrap()
                        } else {
                            terrain_v5::height_for(version, seed, px, pz)
                        }
                    };
                    let h = height(x, z);
                    let slope = [[-1, 0], [1, 0], [0, -1], [0, 1]]
                        .into_iter()
                        .map(|[dx, dz]| (height(x + dx, z + dz) - h).abs())
                        .max()
                        .unwrap();
                    let b = if final_surface {
                        biome_v2::sample(&g, x, z).unwrap().biome as u8
                    } else {
                        255
                    };
                    println!(
                        "{seed},{region},{x},{z},{h},{slope},{},{},{},{},{},{b}",
                        p.continentalness, p.erosion, p.weirdness, p.offset, p.amplitude
                    );
                }
            }
        }
    }
}
