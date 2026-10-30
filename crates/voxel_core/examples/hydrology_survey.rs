// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic bounded survey of actual v8 surface geography and cave water.
use std::collections::VecDeque;
use voxel_core::{
    Block, ChunkPos, LocalPos,
    worldgen::{
        GenerationConfig, Generator,
        hydrology::{self, WaterBody},
    },
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--pools") {
        println!(
            "POOL_HEADER seed,tested_cells,accepted_pools,water_cells,surface_cells,max_pool_cells"
        );
        for seed in [0, 42, 2026, u64::MAX] {
            let g = Generator::new(GenerationConfig { seed, version: 8 }).unwrap();
            let (mut count, mut cells, mut surface, mut max) = (0, 0, 0, 0);
            for z in -8..8 {
                for x in -8..8 {
                    if let Some(p) = g.natural_pool_summary([x, z]) {
                        count += 1;
                        cells += p.cells;
                        surface += p.surface_cells;
                        max = max.max(p.cells);
                        let position = p.sample.0;
                        println!(
                            "POOL seed={seed} ownership={x},{z} cells={} surface={} min={:?} max={:?} sample={position:?} geography={:?}",
                            p.cells,
                            p.surface_cells,
                            p.min,
                            p.max,
                            hydrology::surface(seed, position[0], position[2]).body
                        );
                    }
                }
            }
            println!("POOL_SUMMARY {seed},256,{count},{cells},{surface},{max}");
        }
        return;
    }
    if args.first().is_some_and(|a| a == "--probe") {
        let seed = args[1].parse().unwrap();
        let x: i32 = args[2].parse().unwrap();
        let z: i32 = args[3].parse().unwrap();
        let g = Generator::new(GenerationConfig { seed, version: 8 }).unwrap();
        for dz in (-16..=16).step_by(4) {
            for dx in (-16..=16).step_by(4) {
                let px = x + dx;
                let pz = z + dz;
                let s = g.sampler([px.div_euclid(16), pz.div_euclid(16)]);
                let col = hydrology::surface(seed, px, pz);
                let air: Vec<_> = (-55..=-43)
                    .filter(|y| s.block(voxel_core::BlockPos([px, *y, pz])) == Block::Air)
                    .collect();
                println!(
                    "PROBE seed={seed} x={px} z={pz} bed={} body={:?} biome={:?} cave_air_y={air:?}",
                    col.bed,
                    col.body,
                    g.biome(px, pz)
                );
            }
        }
        return;
    }
    let n: usize = args.first().map_or(512, |s| s.parse().unwrap());
    assert!(n <= 2048 && n.is_multiple_of(16));
    println!(
        "seed,center_x,center_z,samples,water_percent,ocean_percent,pond_cells,lake_cells,river_cells,components,small_lt256,medium_lt4096,large_ge4096,largest,depth_p50,depth_p95,max_depth,shore_edges,river_lake_edges,cave_water_cells,cave_wet_columns"
    );
    for seed in [0, 42, 2026, u64::MAX] {
        for center in [[0, 0], [8192, -8192], [-16000, 16000]] {
            let min = [center[0] - n as i32 / 2, center[1] - n as i32 / 2];
            let mut cells = vec![0u8; n * n];
            let mut counts = [0usize; 5];
            let mut depths = Vec::new();
            let g = Generator::new(GenerationConfig { seed, version: 8 }).unwrap();
            let (mut underground, mut wet_columns) = (0, 0);
            let mut sites = [None; 5];
            let mut printed_cave = false;
            for z in (0..n).step_by(16) {
                for x in (0..n).step_by(16) {
                    let c = [
                        (min[0] + x as i32).div_euclid(16),
                        (min[1] + z as i32).div_euclid(16),
                    ];
                    let tile = hydrology::surface_tile(seed, c);
                    for (i, s) in tile.iter().enumerate() {
                        let kind = match s.body {
                            WaterBody::Dry => 0,
                            WaterBody::Pond => 1,
                            WaterBody::Lake => 2,
                            WaterBody::River => 3,
                            WaterBody::Ocean => 4,
                        };
                        cells[(z + i / 16) * n + x + i % 16] = kind;
                        counts[kind as usize] += 1;
                        if kind != 0 && sites[kind as usize].is_none_or(|(_, _, d)| s.depth() > d) {
                            sites[kind as usize] = Some((
                                min[0] + x as i32 + (i % 16) as i32,
                                min[1] + z as i32 + (i / 16) as i32,
                                s.depth(),
                            ));
                        }
                        if s.depth() > 0 {
                            depths.push(s.depth());
                        }
                    }
                    // Fixed 8x8-column subwindow per location. Cave statistics are volumes,
                    // separate from the much larger surface survey denominator.
                    if x < n / 2 && x + 128 >= n / 2 && z < n / 2 && z + 128 >= n / 2 {
                        let s = g.sampler(c);
                        let mut wet = 0;
                        for cy in -4..-3 {
                            let chunk = s.chunk(ChunkPos([c[0], cy, c[1]])).unwrap();
                            wet += chunk.water_cells();
                        }
                        underground += wet;
                        wet_columns += usize::from(wet > 0);
                        if wet > 0 && !printed_cave {
                            printed_cave = true;
                            let chunk = s.chunk(ChunkPos([c[0], -4, c[1]])).unwrap();
                            let pos = (0..4096)
                                .find_map(|i| {
                                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                                    (chunk.get(l) == Block::WaterSource).then(|| {
                                        ChunkPos([c[0], -4, c[1]]).block_position(l).unwrap()
                                    })
                                })
                                .unwrap();
                            println!("CAVE seed={seed} position={:?}", pos.0);
                        }
                    }
                }
            }
            for (kind, p) in sites.iter().enumerate() {
                if let Some((x, z, d)) = p {
                    println!("SITE kind={kind} seed={seed} x={x} z={z} depth={d}");
                }
            }
            let (mut shore, mut connections) = (0, 0);
            for z in 0..n {
                for x in 0..n {
                    let i = z * n + x;
                    for j in [
                        if x + 1 < n { Some(i + 1) } else { None },
                        if z + 1 < n { Some(i + n) } else { None },
                    ]
                    .into_iter()
                    .flatten()
                    {
                        shore += usize::from((cells[i] == 0) != (cells[j] == 0));
                        connections += usize::from(
                            (cells[i] == 3 && [2, 4].contains(&cells[j]))
                                || (cells[j] == 3 && [2, 4].contains(&cells[i])),
                        );
                    }
                }
            }
            let mut sizes = Vec::new();
            let mut queue = VecDeque::new();
            for i in 0..cells.len() {
                if cells[i] == 0 {
                    continue;
                }
                cells[i] = 0;
                queue.push_back(i);
                let mut size = 0;
                while let Some(i) = queue.pop_front() {
                    size += 1;
                    let x = i % n;
                    let z = i / n;
                    for j in [
                        if x > 0 { Some(i - 1) } else { None },
                        if x + 1 < n { Some(i + 1) } else { None },
                        if z > 0 { Some(i - n) } else { None },
                        if z + 1 < n { Some(i + n) } else { None },
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if cells[j] != 0 {
                            cells[j] = 0;
                            queue.push_back(j);
                        }
                    }
                }
                sizes.push(size);
            }
            depths.sort_unstable();
            let percentile = |p: usize| {
                depths
                    .get(depths.len().saturating_sub(1) * p / 100)
                    .copied()
                    .unwrap_or(0)
            };
            println!(
                "{seed},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                center[0],
                center[1],
                n * n,
                100. * (n * n - counts[0]) as f64 / (n * n) as f64,
                100. * counts[4] as f64 / (n * n) as f64,
                counts[1],
                counts[2],
                counts[3],
                sizes.len(),
                sizes.iter().filter(|s| **s < 256).count(),
                sizes.iter().filter(|s| (256..4096).contains(*s)).count(),
                sizes.iter().filter(|s| **s >= 4096).count(),
                sizes.iter().max().unwrap_or(&0),
                percentile(50),
                percentile(95),
                depths.last().unwrap_or(&0),
                shore,
                connections,
                underground,
                wet_columns
            );
        }
    }
}
