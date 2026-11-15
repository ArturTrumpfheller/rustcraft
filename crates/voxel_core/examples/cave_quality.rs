// SPDX-License-Identifier: MIT OR Apache-2.0
mod support {
    pub mod cave_quality;
}
use support::cave_quality::*;
fn main() {
    println!(
        "version,seed,site,x,z,n,underground,air,solid_percent,depth_abs_air,depth_relative_air,components,largest,tiny_components,entrance_components,vertical_extent,horizontal_extent,standing_nodes,standing_components,largest_standing,width_bins,height_bins,uniform_air,uniform_stone,mixed,water,invalid_water_faces,air_geodesic_lower_bound,standing_geodesic_lower_bound,interior_solid_face_area"
    );
    for seed in SEEDS {
        for (site, min) in [
            ("central", [-64, -64]),
            ("negative", [-256, -256]),
            ("distant", [28000, -30000]),
            ("mountain", mountain(seed)),
        ] {
            for version in [13, 14] {
                let v = Volume::new(seed, version, min, 128);
                let mask = v.underground();
                let air = mask.iter().filter(|b| **b != 0).count();
                let total = v
                    .heights
                    .iter()
                    .map(|h| (h + 61).max(0) as usize)
                    .sum::<usize>();
                let mut depth = [0; 6];
                let mut relative = [0; 6];
                for (i, b) in mask.iter().enumerate() {
                    if *b == 0 {
                        continue;
                    }
                    let y = i % 256;
                    depth[(y / 32).min(5)] += 1;
                    relative[((v.heights[i / 256] + 64 - y as i32).max(0) as usize / 16).min(5)] +=
                        1;
                }
                let air_geodesic = v.geodesic(mask.clone(), false);
                let components = v.components(mask, false);
                let stands = v.standing();
                let stand_count = stands.iter().filter(|b| **b != 0).count();
                let stand_geodesic = v.geodesic(stands.clone(), true);
                let walk = v.components(stands, true);
                let area = v.interior_area();
                let (width, height) = v.widths();
                let join = |v: &[usize]| {
                    v.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(";")
                };
                println!(
                    "{version},{seed},{site},{},{},128,{total},{air},{:.4},{},{},{},{},{},{},{},{},{stand_count},{},{},{},{},{},{},{},{},{},{air_geodesic},{stand_geodesic},{area}",
                    min[0],
                    min[1],
                    100. * (total - air) as f64 / total.max(1) as f64,
                    join(&depth),
                    join(&relative),
                    components.len(),
                    components.first().map_or(0, |c| c.cells),
                    components.iter().filter(|c| c.cells < 64).count(),
                    components
                        .iter()
                        .filter(|c| c.surface && c.max[1] - c.min[1] > 12)
                        .count(),
                    components
                        .iter()
                        .map(|c| c.max[1] - c.min[1] + 1)
                        .max()
                        .unwrap_or(0),
                    components
                        .iter()
                        .map(|c| (c.max[0] - c.min[0] + 1).max(c.max[2] - c.min[2] + 1))
                        .max()
                        .unwrap_or(0),
                    walk.len(),
                    walk.first().map_or(0, |c| c.cells),
                    join(&width),
                    join(&height),
                    v.uniform[0],
                    v.uniform[1],
                    v.uniform[2],
                    v.water,
                    v.leaks
                );
            }
        }
    }
}
