// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{
    fluid::{FluidConfig, NEIGHBORS},
    inventory::{Inventory, Item, SLOT_COUNT, Stack, edit_inventory},
    physics::{self, MovementInput, PhysicsConfig, Player},
    raycast::Ray,
    regions::{RegionIdentity, RegionPos},
};
fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig { seed, version: 8 }).unwrap()
}
fn area(g: &Generator, center: [i32; 2], radius: i32) -> World {
    let mut world = World::regional(RegionIdentity {
        seed: g.config.seed,
        generator: 8,
        nonce: [28; 16],
    });
    for z in center[1] - radius..=center[1] + radius {
        for x in center[0] - radius..=center[0] + radius {
            world.install_region(
                RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            let s = g.sampler([x, z]);
            for y in g.world_height().chunks() {
                let cp = ChunkPos([x, y, z]);
                world.insert_chunk(cp, s.chunk(cp).unwrap());
            }
        }
    }
    world
}
fn crc(g: &Generator, c: [i32; 2]) -> u32 {
    let mut crc = crc32fast::Hasher::new();
    let s = g.sampler(c);
    for y in g.world_height().chunks() {
        let chunk = s.chunk(ChunkPos([c[0], y, c[1]])).unwrap();
        for i in 0..4096 {
            let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
            let id = chunk.get(l) as u16;
            crc.update(&id.to_le_bytes());
        }
    }
    crc.finalize()
}
#[test]
fn independent_v8_goldens_point_chunk_negative_extremes_and_parallel_order() {
    for (seed, c, expected) in [
        (0, [0, 0], 2155091821),
        (42, [-28, 1], 2101020021),
        (2026, [6, -16], 3417203662),
        (u64::MAX, [-2048, 2047], 3382729508),
        (42, [2047, -2048], 1524942302),
    ] {
        let g = generator(seed);
        let actual = crc(&g, c);
        println!("GOLDEN seed={seed} column={c:?} crc={actual}");
        assert_eq!(actual, expected);
        let s = g.sampler(c);
        for y in [-4, -3, -2, -1, 0, 1, 11] {
            let cp = ChunkPos([c[0], y, c[1]]);
            let chunk = s.chunk(cp).unwrap();
            for l in [[0, 0, 0], [15, 15, 15], [0, 15, 15], [15, 0, 0], [7, 7, 7]] {
                let l = LocalPos::new(l).unwrap();
                let p = cp.block_position(l).unwrap();
                assert_eq!(chunk.get(l), g.block(p), "{p:?}");
                assert_eq!(s.block(p), chunk.get(l));
            }
        }
        let reverse: Vec<_> = g
            .world_height()
            .chunks()
            .rev()
            .map(|y| s.chunk(ChunkPos([c[0], y, c[1]])).unwrap())
            .collect();
        std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..2).map(|_| scope.spawn(|| crc(&g, c))).collect();
            for task in tasks {
                assert_eq!(task.join().unwrap(), actual);
            }
        });
        for (y, ch) in g.world_height().chunks().rev().zip(reverse) {
            assert_eq!(ch, g.chunk(ChunkPos([c[0], y, c[1]])).unwrap());
        }
    }
}
#[test]
fn pristine_ocean_lake_pond_river_and_cave_pools_are_stable_without_save_edits() {
    for (seed, x, z) in [
        (42, 100_i32, 100_i32),
        (42, -434, 31),
        (2026, -97, -466),
        (2026, 107, -243),
        (0, -97, -20),
    ] {
        let g = generator(seed);
        let mut w = area(&g, [x.div_euclid(16), z.div_euclid(16)], 1);
        assert!(w.water_cells() > 0, "seed={seed} x={x} z={z}");
        assert_eq!(
            w.fluid_updates().pending(),
            0,
            "pristine body must not wake: seed={seed} at {x},{z}"
        );
        let revision = w.revision();
        let water = w.water_cells();
        let mut changes = Vec::new();
        for _ in 0..64 {
            w.tick_fluids(FluidConfig::default(), &mut changes);
            assert!(changes.is_empty());
            assert_eq!(w.fluid_updates().processed, 0);
        }
        assert_eq!(w.edit_count(), 0);
        assert_eq!(w.revision(), revision);
        assert_eq!(w.water_cells(), water);
        for (cp, ch) in w.chunks() {
            for i in 0..4096 {
                let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                if ch.get(l) != Block::WaterSource {
                    continue;
                }
                let p = cp.block_position(l).unwrap();
                for d in NEIGHBORS.into_iter().filter(|d| *d != [0, 1, 0]) {
                    let n = p.offset(d).unwrap();
                    if w.in_bounds(n) && w.ready(n) {
                        assert!(
                            w.block(n).is_collidable() || w.block(n) == Block::WaterSource,
                            "unsupported/exposed natural water {p:?} neighbor {n:?}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn water_geography_varies_with_seed_has_irregular_banks_and_crosses_region_seams() {
    use hydrology::WaterBody;
    let mut counts = [0usize; 5];
    let mut seams = 0;
    let mut connections = 0;
    let mut depths = std::collections::BTreeSet::new();
    for seed in [0, 42, 2026, u64::MAX] {
        for cz in -32..32 {
            for cx in -32..32 {
                let tile = hydrology::surface_tile(seed, [cx, cz]);
                for (i, s) in tile.iter().enumerate() {
                    let k = match s.body {
                        WaterBody::Dry => 0,
                        WaterBody::Pond => 1,
                        WaterBody::Lake => 2,
                        WaterBody::River => 3,
                        WaterBody::Ocean => 4,
                    };
                    counts[k] += 1;
                    if s.depth() > 0 {
                        depths.insert(s.depth());
                        if i % 16 == 0 && cx % 4 == 0 {
                            seams += 1;
                        }
                    }
                    if i % 16 < 15
                        && s.body == WaterBody::River
                        && matches!(tile[i + 1].body, WaterBody::Lake | WaterBody::Ocean)
                    {
                        connections += 1;
                    }
                }
            }
        }
    }
    assert!(counts.iter().all(|n| *n > 100));
    assert!(seams > 100 && connections > 0 && depths.len() > 15);
    assert!(counts[0] > counts.iter().sum::<usize>() / 4);
    assert_ne!(
        hydrology::surface_tile(42, [-28, 1])
            .iter()
            .map(|s| s.bed)
            .collect::<Vec<_>>(),
        hydrology::surface_tile(2026, [-28, 1])
            .iter()
            .map(|s| s.bed)
            .collect::<Vec<_>>()
    );
}
#[test]
fn natural_water_swims_collects_and_flows_into_mined_pockets_then_recedes() {
    let g = generator(2026);
    let [x, z]: [i32; 2] = [107, -243];
    let mut w = area(&g, [x.div_euclid(16), z.div_euclid(16)], 1);
    let config = PhysicsConfig::default();
    let mut player = Player::new([x as f64 + 0.5, 0., z as f64 + 0.5]);
    assert!(player.in_water(&w, &config));
    assert!(physics::submerged(&w, [x as f64 + 0.5, 1., z as f64 + 0.5]));
    let initial = player.position[1];
    for _ in 0..10 {
        player.tick(
            &w,
            MovementInput {
                jump: true,
                ..Default::default()
            },
            &config,
        );
    }
    assert!(player.position[1] > initial);
    let mut slots = [None; SLOT_COUNT];
    slots[0] = Stack::new(Item::EmptyBucket, 1);
    let mut inventory = Inventory::from_parts(slots, None, 0).unwrap();
    let p = BlockPos([x, 2, z]);
    edit_inventory(
        &mut w,
        &mut inventory,
        Ray {
            origin: [x as f64 + 0.5, 4., z as f64 + 0.5],
            direction: [0., -1., 0.],
            reach: 5.,
        },
        true,
        &Player::new([x as f64 + 8., 4., z as f64 + 8.]),
        &config,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        inventory.selected_stack().unwrap().item(),
        Item::WaterBucket
    );
    assert_eq!(w.block(p), Block::Air);
    let bed = g.height(x, z).unwrap();
    let pocket = BlockPos([x, bed, z]);
    w.set_block(pocket, Block::Air).unwrap();
    let mut changes = Vec::new();
    for _ in 0..128 {
        w.tick_fluids(FluidConfig::default(), &mut changes);
        if w.fluid_updates().pending() == 0 {
            break;
        }
    }
    assert_eq!(w.block(pocket), Block::WaterFalling);
    w.set_block(pocket, Block::Torch).unwrap();
    for _ in 0..16 {
        w.tick_fluids(FluidConfig::default(), &mut changes);
    }
    assert_eq!(w.block(pocket), Block::WaterFalling);
    assert_eq!(w.drops.len(), 1);
    assert_ne!(w.block(p), Block::WaterSource);
    assert_eq!(w.fluid_updates().pending(), 0);
    // Blocking a river displaces only the edited source; no generator authority
    // can restore it during fluid ticks or cache regeneration.
    w.set_block(p, Block::Stone).unwrap();
    for _ in 0..32 {
        w.tick_fluids(FluidConfig::default(), &mut changes);
    }
    assert_eq!(w.block(p), Block::Stone);
}
#[test]
fn v8_cache_eviction_and_overlay_remain_bounded_and_authoritative() {
    let g = generator(42);
    let c = [-28, 1];
    let mut w = area(&g, c, 0);
    let p = BlockPos([-434, 2, 31]);
    assert_eq!(w.block(p), Block::WaterSource);
    w.set_block(p, Block::Air).unwrap();
    let edits = w.overlay_snapshot();
    w.evict_column(c);
    crate::streaming::prepare_column(&mut w, c).unwrap();
    assert_eq!(w.block(p), Block::Air);
    assert_eq!(w.overlay_snapshot().get(&p), edits.get(&p));
    for x in -80..80 {
        let _ = g.sampler([x * 2, 48]);
    }
    assert!(g.water_planning_bytes() <= 64 * 8192 * std::mem::size_of::<BlockPos>());
}

#[test]
fn maximum_resident_uniform_source_volume_does_not_enqueue_or_allocate_voxel_arrays() {
    let mut w = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 8,
    });
    for z in -3..=3 {
        for x in -3..=3 {
            for y in -4..12 {
                w.insert_chunk(ChunkPos([x, y, z]), Chunk::filled(Block::WaterSource));
            }
        }
    }
    assert_eq!(w.chunk_count(), 49 * 16);
    assert_eq!(w.allocated_voxel_bytes(), 0);
    assert_eq!(w.fluid_updates().pending(), 0);
    let mut changes = Vec::new();
    w.tick_fluids(FluidConfig::default(), &mut changes);
    assert_eq!(w.fluid_updates().total_processed, 0);
    w.evict_column([0, 0]);
    assert_eq!(w.fluid_updates().pending(), 0);
}

#[test]
fn uniform_water_interiors_skip_mesh_and_water_faces_remain_seam_consistent() {
    let mut w = World::default();
    for z in -1..=1 {
        for x in -1..=1 {
            for y in -1..=1 {
                w.insert_chunk(ChunkPos([x, y, z]), Chunk::filled(Block::WaterSource));
            }
        }
    }
    let cp = ChunkPos([0, 0, 0]);
    assert!(!crate::streaming::needs_mesh(&w, cp));
    let light = crate::lighting::Skylight::for_chunks(&w, &std::collections::BTreeSet::from([cp]));
    assert!(
        crate::water_mesh::mesh_water(&w, cp, &light)
            .indices
            .is_empty()
    );
    let mut w = area(&generator(42), [6, 6], 1);
    let a = BlockPos([111, 2, 104]);
    let b = BlockPos([112, 2, 104]);
    assert_eq!(w.block(a), Block::WaterSource);
    assert_eq!(
        crate::water_mesh::corner_height(&w, a, 1, 0),
        crate::water_mesh::corner_height(&w, b, 0, 0)
    );
    w.set_block(a, Block::Stone).unwrap();
    assert_eq!(w.block(a), Block::Stone);
}

#[test]
fn dry_v7_surface_entrance_remains_open_in_v8() {
    let g = generator(2026);
    let old = Generator::new(GenerationConfig {
        seed: 2026,
        version: 7,
    })
    .unwrap();
    let mut apertures = 0;
    for cz in -4..=-2 {
        for cx in 1..=3 {
            let a = g.sampler([cx, cz]);
            let b = old.sampler([cx, cz]);
            for z in cz * 16..cz * 16 + 16 {
                for x in cx * 16..cx * 16 + 16 {
                    let h = old.height(x, z).unwrap();
                    if h != g.height(x, z).unwrap() {
                        continue;
                    }
                    let p = BlockPos([x, h, z]);
                    if b.block(p) == Block::Air
                        && [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]]
                            .iter()
                            .all(|d| hydrology::surface(2026, x + d[0], z + d[1]).depth() == 0)
                    {
                        assert_eq!(a.block(p), Block::Air);
                        apertures += 1;
                    }
                }
            }
        }
    }
    assert!(apertures > 0);
}
