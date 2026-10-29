// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reproducible height survey; candidates are coordinates, never stored world plans.
use voxel_core::worldgen::{GenerationConfig, Generator};
fn main() {
    println!("seed,kind,x,z,min,max,mean");
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 4 }).unwrap();
        let mut flat = (i32::MAX, 0, 0, 0, 0, 0);
        let mut mountain = (0, 0, 0, 0, 0, 0);
        let mut valley = (0, 0, 0, 0, 0, 0);
        let mut basin = (0, 0, 0, 0, 0, 0);
        let mut hills = (i32::MAX, 0, 0, 0, 0, 0);
        for z in (-2048..2048).step_by(32) {
            for x in (-2048..2048).step_by(32) {
                let mut lo = 32;
                let mut hi = -16;
                let mut sum = 0;
                for dz in (0..32).step_by(4) {
                    for dx in (0..32).step_by(4) {
                        let h = g.height(x + dx, z + dz).unwrap();
                        lo = lo.min(h);
                        hi = hi.max(h);
                        sum += h;
                    }
                }
                let mean = sum / 64;
                let rough = hi - lo;
                let h = g.height(x, z).unwrap();
                let sides = g
                    .height(x - 32, z)
                    .unwrap()
                    .min(g.height(x + 32, z).unwrap());
                let depth = sides - h;
                if h >= 8 && depth > basin.0 {
                    basin = (depth, x, z, h, sides, h);
                }
                let score = rough * 100 + mean;
                if score < flat.0 {
                    flat = (score, x, z, lo, hi, mean);
                }
                if mean > mountain.0 {
                    mountain = (mean, x, z, lo, hi, mean);
                }
                if rough > valley.0 {
                    valley = (rough, x, z, lo, hi, mean);
                }
                let score = (rough - 4).abs() * 100 + (mean - 9).abs();
                if score < hills.0 {
                    hills = (score, x, z, lo, hi, mean);
                }
            }
        }
        for (kind, (_, x, z, lo, hi, mean)) in [
            ("plain", flat),
            ("hills", hills),
            ("mountain", mountain),
            ("transition", valley),
            ("valley", basin),
        ] {
            println!("{seed},{kind},{x},{z},{lo},{hi},{mean}");
        }
    }
}
