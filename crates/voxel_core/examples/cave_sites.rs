// SPDX-License-Identifier: MIT OR Apache-2.0
//! Print final-air camera candidates near bounded feature landmarks.
use voxel_core::{
    Block, BlockPos,
    worldgen::{GenerationConfig, Generator, caves_v3},
};
fn main() {
    println!("kind,seed,x,feet_y,z,anchor_x,anchor_y,anchor_z,surface_y");
    for (seed, min, max) in [
        (42, [-256, -256], [256, 256]),
        (0, [-256, -256], [256, 256]),
        (2026, [-256, -256], [256, 256]),
        (0, [-640, -3328], [-128, -2816]),
        (42, [-896, -896], [-384, -384]),
    ] {
        let g = Generator::new(GenerationConfig { seed, version: 14 }).unwrap();
        for (kind, p) in caves_v3::landmarks(&g, min, max) {
            if !matches!(
                kind,
                "cavern" | "chamber" | "shaft" | "entrance-route" | "narrow" | "tunnel"
            ) {
                continue;
            }
            if matches!(kind, "narrow" | "tunnel" | "shaft" | "entrance-route")
                && (p[0] + p[2]).rem_euclid(13) != 0
            {
                continue;
            }
            // Search a small floor footprint; never move Stone for a camera.
            let mut found = None;
            for dz in (-3..=3).step_by(3) {
                for dx in (-3..=3).step_by(3) {
                    let x = p[0] + dx;
                    let z = p[2] + dz;
                    let s = g.sampler([x.div_euclid(16), z.div_euclid(16)]);
                    for y in (p[1] - 26).max(-59)..=(p[1] + 8).min(180) {
                        let at = |y| s.block(BlockPos([x, y, z]));
                        if at(y) == Block::Air && at(y + 1) == Block::Air && at(y - 1).is_solid() {
                            let distance = (y - p[1]).abs() + dx.abs() + dz.abs();
                            if found.is_none_or(|(_, _, _, d)| distance < d) {
                                found = Some((x, y, z, distance));
                            }
                        }
                    }
                }
            }
            if let Some((x, y, z, _)) = found {
                println!(
                    "{kind},{seed},{x},{y},{z},{},{},{},{}",
                    p[0],
                    p[1],
                    p[2],
                    g.height(x, z).unwrap()
                );
            }
        }
    }
}
