use super::*;
use crate::physics::{PhysicsConfig, Player, spawn_position};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig { seed, version: 4 }).unwrap()
}
fn digest(g: &Generator, columns: &[[i32; 2]]) -> u32 {
    let mut hash = crc32fast::Hasher::new();
    for &c in columns {
        let sampler = g.sampler(c);
        for y in -1..2 {
            let p = ChunkPos([c[0], y, c[1]]);
            let chunk = sampler.chunk(p).unwrap();
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        hash.update(&[match chunk.get(LocalPos::new([x, y, z]).unwrap()) {
                            Block::Air => 0,
                            Block::Grass => 1,
                            Block::Dirt => 2,
                            Block::Stone => 3,
                            Block::Wood => 4,
                            Block::Leaves => 5,
                            _ => panic!("terrain must not generate gameplay blocks"),
                        }]);
                    }
                }
            }
        }
    }
    hash.finalize()
}
#[test]
fn v4_independent_goldens_order_threads_regeneration_and_point_sampling() {
    let columns = [
        [0, 0],
        [-1, 3],
        [4, -4],
        [120, -120],
        [1000, -1000],
        [2047, -2048],
    ];
    let mut digests = BTreeSet::new();
    for (seed, expected) in [
        (0, 0xc68b9de0),
        (42, 0xb3a7a9cc),
        (2026, 0xb05730b7),
        (u64::MAX, 0x123be232),
    ] {
        let g = generator(seed);
        let actual = digest(&g, &columns);
        println!("V4_GOLDEN {seed} {actual:#010x}");
        assert_eq!(actual, expected, "seed {seed}");
        assert!(digests.insert(actual));
        assert_eq!(g.planning_bytes(), 0);
        let forward: BTreeMap<_, _> = columns
            .iter()
            .map(|&c| {
                let s = g.sampler(c);
                (
                    c,
                    std::array::from_fn::<_, 3, _>(|i| {
                        s.chunk(ChunkPos([c[0], i as i32 - 1, c[1]])).unwrap()
                    }),
                )
            })
            .collect();
        // Scoped workers request columns in the opposite order; no shared RNG.
        std::thread::scope(|scope| {
            for &c in columns.iter().rev() {
                let expected = &forward[&c];
                let g = &g;
                scope.spawn(move || {
                    let s = g.sampler(c);
                    for (i, expected) in expected.iter().enumerate() {
                        let p = ChunkPos([c[0], i as i32 - 1, c[1]]);
                        assert_eq!(s.chunk(p).unwrap(), *expected);
                        for y in (0..16).step_by(3) {
                            for z in [0, 1, 7, 14, 15] {
                                for x in [0, 1, 7, 14, 15] {
                                    let l = LocalPos::new([x, y, z]).unwrap();
                                    let pos = p.block_position(l).unwrap();
                                    assert_eq!(expected.get(l), g.block(pos), "{seed} {pos:?}");
                                }
                            }
                        }
                    }
                });
            }
        });
        let mut world = crate::streaming::new_large_world(seed);
        for (&c, chunks) in &forward {
            for (i, chunk) in chunks.iter().enumerate() {
                world.insert_chunk(ChunkPos([c[0], i as i32 - 1, c[1]]), chunk.clone());
            }
            world.evict_column(c);
            assert_eq!(
                g.sampler(c).chunk(ChunkPos([c[0], 0, c[1]])).unwrap(),
                chunks[1]
            );
        }
    }
    let g = generator(42);
    for p in [
        BlockPos([i32::MIN; 3]),
        BlockPos([i32::MAX; 3]),
        BlockPos([-32769, 0, 0]),
        BlockPos([32768, 0, 0]),
    ] {
        assert_eq!(g.block(p), Block::Air);
        assert_eq!(g.sampler([0, 0]).block(p), Block::Air);
    }
    for c in [[i32::MAX; 2], [i32::MIN; 2], [-2049, 0], [2048, 0]] {
        assert!(g.sampler(c).chunk(ChunkPos([c[0], 0, c[1]])).is_none());
    }
}

fn largest_component(mut cells: BTreeSet<[i32; 2]>) -> usize {
    let mut largest = 0;
    while let Some(p) = cells.pop_first() {
        let mut queue = VecDeque::from([p]);
        let mut size = 0;
        while let Some([x, z]) = queue.pop_front() {
            size += 1;
            for p in [[x - 1, z], [x + 1, z], [x, z - 1], [x, z + 1]] {
                if cells.remove(&p) {
                    queue.push_back(p);
                }
            }
        }
        largest = largest.max(size);
    }
    largest
}
#[test]
fn v4_multiseed_geography_has_building_space_connected_uplands_and_smooth_bounds() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let mut lo = 32;
        let mut hi = -16;
        let mut uplands = BTreeSet::new();
        let mut lowlands = BTreeSet::new();
        let mut steps = [0usize; 5];
        let mut flat_patches = 0;
        for z in (-2048..2048).step_by(16) {
            for x in (-2048..2048).step_by(16) {
                let h = g.height(x, z).unwrap();
                lo = lo.min(h);
                hi = hi.max(h);
                if h >= 15 {
                    uplands.insert([x / 16, z / 16]);
                }
                if h <= 6 {
                    lowlands.insert([x / 16, z / 16]);
                }
                for (dx, dz) in [(1, 0), (0, 1)] {
                    let d = (h - g.height(x + dx, z + dz).unwrap()).unsigned_abs() as usize;
                    assert!(d <= 3, "abrupt transition {seed} ({x},{z}) {d}");
                    steps[d] += 1;
                }
                // A full 32x32 block footprint sampled every four cells.
                if x.rem_euclid(64) == 0 && z.rem_euclid(64) == 0 {
                    let mut a = 32;
                    let mut b = -16;
                    for dz in (0..32).step_by(4) {
                        for dx in (0..32).step_by(4) {
                            let h = g.height(x + dx, z + dz).unwrap();
                            a = a.min(h);
                            b = b.max(h);
                        }
                    }
                    flat_patches += usize::from(b - a <= 2);
                }
            }
        }
        let elevated = largest_component(uplands);
        let low = largest_component(lowlands);
        println!(
            "V4_QUALITY seed={seed} heights={lo}..{hi} steps={steps:?} flat32={flat_patches} elevated_cells={elevated} low_cells={low}"
        );
        assert!(lo >= -1 && hi <= 24 && hi - lo >= 18);
        assert!(hi < 25, "height ceiling must not be reached/clipped");
        assert!(flat_patches >= 100, "too little building space");
        assert!(
            elevated >= 256 && low >= 512,
            "regions must span many chunks"
        );
        assert!(steps[0] + steps[1] > steps.iter().sum::<usize>() * 99 / 100);
    }
}
#[test]
fn v4_spawn_trees_caves_foundation_and_canopy_clearance() {
    for seed in [0, 1, 42, 2026, 9999, u64::MAX] {
        let g = generator(seed);
        let mut w = World::default();
        // Match startup: only one prepared column, including the protected spawn.
        let s = g.sampler([0, 0]);
        for y in -1..2 {
            let p = ChunkPos([0, y, 0]);
            w.insert_chunk(p, s.chunk(p).unwrap());
        }
        let cfg = PhysicsConfig::default();
        let p = spawn_position(&w, &cfg, [0.5, 16., 10.5]);
        assert!(Player::new(p).fits_in_world(&w, &cfg));
        let foot = BlockPos([
            p[0].floor() as i32,
            p[1].floor() as i32 - 1,
            p[2].floor() as i32,
        ]);
        assert!(matches!(
            w.block(foot),
            Block::Grass | Block::Dirt | Block::Stone
        ));
        for dx in -2..=2 {
            for dz in -2..=2 {
                let h = g.height(foot.0[0] + dx, foot.0[2] + dz).unwrap();
                assert!((h - foot.0[1]).abs() <= 2, "spawn slope");
            }
        }
        let mut trees = 0;
        for z in -24..24 {
            for x in -24..24 {
                let h = g.height(x, z).unwrap();
                assert_eq!(g.block(BlockPos([x, -16, z])), Block::Stone);
                assert_eq!(g.block(BlockPos([x, -15, z])), Block::Stone);
                if g.block(BlockPos([x, h + 1, z])) == Block::Wood {
                    trees += 1;
                    assert_eq!(g.block(BlockPos([x, h, z])), Block::Grass);
                    let top = (h + 1..31)
                        .take_while(|&y| g.block(BlockPos([x, y, z])) == Block::Wood)
                        .last()
                        .unwrap();
                    for dx in -2..=2 {
                        for dz in -2..=2 {
                            assert!(g.height(x + dx, z + dz).unwrap() < top - 1);
                            assert!((g.height(x + dx, z + dz).unwrap() - h).abs() <= 2);
                        }
                    }
                    assert!(top < 30);
                }
                assert_eq!(g.block(BlockPos([x, 31, z])), Block::Air);
            }
        }
        assert!(trees > 0);
        // Established walkable tunnel and ramp, with v4's own entrance height.
        let entrance = g.height(-10, 20).unwrap() + 1;
        assert_eq!(g.block(BlockPos([-10, entrance, 22])), Block::Air);
        for z in -16..=20 {
            let floor = entrance - ((20 - z).clamp(0, 36) * (entrance + 6) / 32).min(entrance + 6);
            for y in floor..floor + 4 {
                assert_eq!(g.block(BlockPos([-10, y, z])), Block::Air);
            }
        }
        assert_eq!(g.block(BlockPos([-10, -4, -10])), Block::Air);
    }
}

#[test]
fn v4_tree_cave_and_edit_meshes_and_light_match_full_neighbors_at_region_seams() {
    use std::{collections::BTreeMap, sync::Arc};
    let g = Generator::new(GenerationConfig {
        seed: 2026,
        version: 4,
    })
    .unwrap();
    for center in [[1000, -1000], [-1001, 1003], [3, 4]] {
        let mut reference = World::default();
        for x in center[0] - 1..=center[0] + 1 {
            for z in center[1] - 1..=center[1] + 1 {
                for y in -1..2 {
                    let p = ChunkPos([x, y, z]);
                    reference.insert_chunk(p, g.chunk(p).unwrap());
                }
            }
        }
        let mut edits = BTreeMap::new();
        for p in [
            BlockPos([center[0] * 16, -15, center[1] * 16]),
            BlockPos([center[0] * 16 - 1, 25, center[1] * 16]),
            BlockPos([center[0] * 16 + 15, 15, center[1] * 16 + 15]),
        ] {
            let b = if p.0[1] < 0 { Block::Air } else { Block::Wood };
            edits.insert(p, b);
            reference.apply_cached_edit(p, b);
        }
        for (p, c, m) in crate::streaming::build_column(&g, Arc::new(edits), center, 0).chunks {
            assert_eq!(&c, reference.chunk(p).unwrap());
            assert_eq!(m, crate::mesh_chunk(&reference, p));
        }
        for p in [
            BlockPos([center[0] * 16, 4, center[1] * 16]),
            BlockPos([center[0] * 16 + 15, 20, center[1] * 16 + 15]),
        ] {
            assert_eq!(g.block(p), g.sampler(center).block(p));
        }
    }
}

#[test]
fn v4_carved_terrain_remains_connected_to_the_foundation() {
    const SIDE: usize = 48;
    let index = |[x, y, z]: [usize; 3]| (y * SIDE + z) * SIDE + x;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let mut solid = vec![false; SIDE * SIDE * SIDE];
        let mut reached = solid.clone();
        let mut queue = VecDeque::new();
        for z in 0..SIDE {
            for x in 0..SIDE {
                let [wx, wz] = [x as i32 - 16, z as i32 - 16];
                let h = g.height(wx, wz).unwrap();
                for y in 0..SIDE {
                    solid[index([x, y, z])] =
                        local::terrain(&g, BlockPos([wx, y as i32 - 16, wz]), h).is_cube();
                }
                assert!(solid[index([x, 0, z])]);
                reached[index([x, 0, z])] = true;
                queue.push_back([x, 0, z]);
            }
        }
        while let Some(p) = queue.pop_front() {
            for axis in 0..3 {
                for d in [-1, 1] {
                    let v = p[axis] as i32 + d;
                    if !(0..SIDE as i32).contains(&v) {
                        continue;
                    }
                    let mut q = p;
                    q[axis] = v as usize;
                    let i = index(q);
                    if solid[i] && !reached[i] {
                        reached[i] = true;
                        queue.push_back(q);
                    }
                }
            }
        }
        assert!(
            solid.iter().zip(reached).all(|(&s, r)| !s || r),
            "floating terrain seed {seed}"
        );
    }
}

#[test]
fn v4_all_generated_foliage_matches_points_on_slopes_and_canopy_edges() {
    let mut raised_crowns = 0;
    for (seed, center) in [(0, [-44, 32]), (42, [88, 34]), (2026, [4, -40])] {
        let g = generator(seed);
        for z in center[1] - 4..center[1] + 4 {
            for x in center[0] - 4..center[0] + 4 {
                let s = g.sampler([x, z]);
                for y in -1..2 {
                    let p = ChunkPos([x, y, z]);
                    let c = s.chunk(p).unwrap();
                    for ly in 0..16 {
                        for lz in 0..16 {
                            for lx in 0..16 {
                                let l = LocalPos::new([lx, ly, lz]).unwrap();
                                let b = c.get(l);
                                if matches!(b, Block::Leaves | Block::Wood) {
                                    let p = p.block_position(l).unwrap();
                                    raised_crowns +=
                                        usize::from(p.0[1] > g.height(p.0[0], p.0[2]).unwrap() + 7);
                                    assert_eq!(g.block(p), b, "slope foliage {seed} {p:?}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        raised_crowns > 0,
        "fixture must exercise the high neighboring crown case"
    );
}
