// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;

fn put(w: &mut World, p: [i32; 3], b: Block) {
    w.set_block(BlockPos(p), b).unwrap();
}
fn basin() -> World {
    let mut w = World::default();
    for x in -10..=10 {
        for z in -10..=10 {
            for y in -1..=0 {
                put(&mut w, [x, y, z], Block::Stone);
            }
        }
    }
    w
}
fn settle(w: &mut World, budget: usize) {
    let mut changes = Vec::new();
    for _ in 0..2000 {
        w.tick_fluids(FluidConfig { budget }, &mut changes);
        if w.fluid_updates().pending() == 0 {
            return;
        }
    }
    panic!(
        "route did not settle: {} pending",
        w.fluid_updates().pending()
    );
}
fn mask(w: &World, p: [i32; 3]) -> u8 {
    route_directions(w, BlockPos(p), w.block(BlockPos(p)).fluid().unwrap())
}
fn wet(w: &World, p: [i32; 3]) -> bool {
    w.block(BlockPos(p)).fluid().is_some()
}

#[test]
fn flat_terrain_keeps_all_four_directions_and_seven_levels() {
    let mut w = basin();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    assert_eq!(mask(&w, [0, 1, 0]), 15);
    for d in SIDES {
        for level in 1..=7 {
            assert_eq!(
                w.block(BlockPos([d[0] * level, 1, d[2] * level]))
                    .fluid()
                    .unwrap()
                    .state_id(),
                level as u8
            );
        }
    }
}

#[test]
fn left_and_right_drop_preference_excludes_unrelated_directions() {
    for (x, expected) in [(-2, 1), (2, 2)] {
        let mut w = basin();
        put(&mut w, [x, 0, 0], Block::Air);
        put(&mut w, [0, 1, 0], Block::WaterSource);
        settle(&mut w, 256);
        assert_eq!(mask(&w, [0, 1, 0]), expected);
        assert!(wet(&w, [x, 0, 0]));
        assert!(!wet(&w, [-x.signum(), 1, 0]));
        assert!(!wet(&w, [0, 1, 1]));
    }
}

#[test]
fn equal_routes_and_merging_shortest_paths_keep_all_first_directions() {
    let mut w = basin();
    put(&mut w, [-2, 0, 0], Block::Air);
    put(&mut w, [2, 0, 0], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    assert_eq!(mask(&w, [0, 1, 0]), 3);
    put(&mut w, [-2, 0, 0], Block::Stone);
    put(&mut w, [2, 0, 0], Block::Stone);
    put(&mut w, [2, 0, 2], Block::Air);
    assert_eq!(mask(&w, [0, 1, 0]), 2 | 8);
    settle(&mut w, 19);
    assert!(wet(&w, [2, 0, 2]));
}

#[test]
fn longer_route_loses_and_direct_downward_flow_has_priority() {
    let mut w = basin();
    put(&mut w, [-3, 0, 0], Block::Air);
    put(&mut w, [2, 0, 0], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    assert_eq!(mask(&w, [0, 1, 0]), 2);
    put(&mut w, [0, 0, 0], Block::Air);
    settle(&mut w, 256);
    assert!(wet(&w, [0, 0, 0]));
    for d in SIDES {
        assert!(!wet(&w, [d[0], 1, d[2]]));
    }
}

#[test]
fn unreachable_and_out_of_range_openings_are_ignored() {
    let mut w = basin();
    put(&mut w, [2, 0, 0], Block::Air);
    for d in SIDES {
        put(&mut w, [2 + d[0], 1, d[2]], Block::Stone);
    }
    put(&mut w, [0, 0, -8], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    assert_eq!(mask(&w, [0, 1, 0]), 1 | 4 | 8); // right neighbor itself is a wall
    settle(&mut w, 256);
    assert!(!wet(&w, [2, 0, 0]));
    assert!(!wet(&w, [0, 0, -8]));
    assert!(wet(&w, [-1, 1, 0]));
}

#[test]
fn walls_and_corners_count_real_horizontal_steps() {
    let mut w = basin();
    put(&mut w, [2, 0, 0], Block::Air);
    put(&mut w, [1, 1, 0], Block::Stone);
    put(&mut w, [-3, 0, 0], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    // Right is four steps around a wall, left only three.
    assert_eq!(mask(&w, [0, 1, 0]), 1);
    put(&mut w, [-3, 0, 0], Block::Stone);
    // The shortest detours start in both Z directions, never through the wall.
    assert_eq!(mask(&w, [0, 1, 0]), 4 | 8);
    settle(&mut w, 256);
    assert_eq!(w.block(BlockPos([1, 1, 0])), Block::Stone);
    assert!(wet(&w, [2, 0, 0]));
}

#[test]
fn narrow_bent_channel_respects_remaining_horizontal_strength() {
    let mut w = basin();
    for x in -4..=4 {
        for z in -4..=4 {
            put(&mut w, [x, 1, z], Block::Stone);
        }
    }
    for p in [[0, 1, 0], [1, 1, 0], [2, 1, 0], [2, 1, 1], [2, 1, 2]] {
        put(&mut w, p, Block::Air);
    }
    put(&mut w, [2, 0, 2], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 17);
    assert_eq!(w.block(BlockPos([2, 1, 2])), Block::Water4);
    assert_eq!(w.block(BlockPos([2, 0, 2])), Block::WaterFalling);
    put(&mut w, [0, 1, 0], Block::Water6);
    assert_eq!(mask(&w, [0, 1, 0]), 2); // fallback can enter only the channel
    settle(&mut w, 256);
    assert_eq!(w.water_cells(), 0);
}

#[test]
fn distant_terrain_edit_opens_and_blocks_a_preferred_route_repeatedly() {
    let mut w = basin();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    for _ in 0..3 {
        put(&mut w, [4, 0, 0], Block::Air);
        settle(&mut w, 23);
        assert_eq!(mask(&w, [0, 1, 0]), 2);
        assert!(!wet(&w, [-1, 1, 0]));
        assert!(wet(&w, [4, 0, 0]));
        put(&mut w, [4, 0, 0], Block::Stone);
        settle(&mut w, 256);
        assert!(wet(&w, [-1, 1, 0]));
        assert_eq!(mask(&w, [0, 1, 0]), 15);
    }
    put(&mut w, [0, 1, 0], Block::Air);
    settle(&mut w, 17);
    assert_eq!(w.water_cells(), 0);
}

#[test]
fn edit_without_any_adjacent_water_still_wakes_distant_feeder() {
    let mut w = basin();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    put(&mut w, [-2, 0, 0], Block::Air);
    settle(&mut w, 256);
    assert!(!wet(&w, [1, 1, 0]));
    // The new right-hand hole is completely dry, including all six neighbors.
    put(&mut w, [2, 0, 0], Block::Air);
    settle(&mut w, 256);
    assert!(wet(&w, [2, 0, 0]));
    assert_eq!(mask(&w, [0, 1, 0]), 3);
}

#[test]
fn multiple_sources_and_queue_budgets_reach_identical_fixed_point() {
    let mut a = basin();
    let mut b = basin();
    for w in [&mut a, &mut b] {
        put(w, [0, 0, 2], Block::Air);
    }
    for p in [[-2, 1, 0], [2, 1, 0]] {
        put(&mut a, p, Block::WaterSource);
    }
    for p in [[2, 1, 0], [-2, 1, 0]] {
        put(&mut b, p, Block::WaterSource);
    }
    settle(&mut a, 7);
    settle(&mut b, 256);
    assert_eq!(
        crate::persistence::encode(&a).unwrap(),
        crate::persistence::encode(&b).unwrap()
    );
    put(&mut a, [-2, 1, 0], Block::Air);
    settle(&mut a, 11);
    assert!(wet(&a, [0, 0, 2]));
    put(&mut a, [2, 1, 0], Block::Air);
    settle(&mut a, 256);
    assert_eq!(a.water_cells(), 0);
}

#[test]
fn saving_partial_and_settled_routes_restores_valid_state_and_zero_idle_work() {
    let mut w = basin();
    put(&mut w, [3, 0, 0], Block::Air);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    w.tick_fluids(FluidConfig { budget: 7 }, &mut Vec::new());
    let mut loaded = crate::persistence::decode(&crate::persistence::encode(&w).unwrap()).unwrap();
    settle(&mut w, 13);
    settle(&mut loaded, 256);
    let bytes = crate::persistence::encode(&w).unwrap();
    assert_eq!(bytes, crate::persistence::encode(&loaded).unwrap());
    let revision = loaded.revision();
    let mut changes = Vec::new();
    for _ in 0..100 {
        loaded.tick_fluids(FluidConfig::default(), &mut changes);
        assert_eq!(loaded.fluid_updates().processed, 0);
    }
    assert_eq!(loaded.revision(), revision);
}

fn column(w: &mut World, cx: i32, cz: i32, hole: bool) {
    for cy in -1..=1 {
        let mut c = crate::Chunk::default();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let lp = LocalPos::new([x, y, z]).unwrap();
                    let p = ChunkPos([cx, cy, cz]).block_position(lp).unwrap();
                    if p.0[1] <= 0 && !(hole && p.0 == [17, 0, 8]) {
                        c.set(lp, Block::Stone);
                    }
                }
            }
        }
        w.insert_chunk(ChunkPos([cx, cy, cz]), c);
    }
}
#[test]
fn chunk_install_reactivates_routes_beyond_immediate_face_water() {
    let mut w = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 2,
    });
    column(&mut w, 0, 0, false);
    put(&mut w, [14, 1, 8], Block::WaterSource);
    put(&mut w, [11, 0, 8], Block::Air);
    settle(&mut w, 256);
    assert!(!wet(&w, [15, 1, 8]));
    column(&mut w, 1, 0, true);
    // Equally short drop across the seam activates a previously dry direction.
    settle(&mut w, 256);
    assert_eq!(mask(&w, [14, 1, 8]), 3);
    assert!(wet(&w, [17, 0, 8]));
    put(&mut w, [11, 0, 8], Block::Stone);
    settle(&mut w, 31);
    assert_eq!(mask(&w, [14, 1, 8]), 2);
    assert!(wet(&w, [17, 0, 8]));
    assert_eq!(w.chunk_count(), 6);
    w.evict_column([1, 0]);
    settle(&mut w, 256);
    assert_eq!(w.chunk_count(), 3);
    column(&mut w, 1, 0, true);
    settle(&mut w, 256);
    assert!(wet(&w, [17, 0, 8]));
}

#[test]
fn unloaded_routes_are_closed_and_never_materialize_chunks() {
    let mut w = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 2,
    });
    column(&mut w, 0, 0, false);
    put(&mut w, [14, 1, 8], Block::WaterSource);
    settle(&mut w, 256);
    assert_eq!(w.chunk_count(), 3);
    assert!(!w.ready(BlockPos([16, 1, 8])));
    assert_eq!(mask(&w, [14, 1, 8]), 15); // no invented drop in missing terrain
    assert_eq!(w.fluid_updates().pending(), 0);
}

#[test]
fn search_respects_both_four_step_cap_and_remaining_levels() {
    let mut w = basin();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    put(&mut w, [5, 0, 0], Block::Air);
    assert_eq!(mask(&w, [0, 1, 0]), 15);
    put(&mut w, [4, 0, 0], Block::Air);
    assert_eq!(mask(&w, [0, 1, 0]), 2);
    put(&mut w, [0, 1, 0], Block::Water5);
    assert_eq!(mask(&w, [0, 1, 0]), 15);
    put(&mut w, [2, 0, 0], Block::Air);
    assert_eq!(mask(&w, [0, 1, 0]), 2);
    put(&mut w, [0, 1, 0], Block::Water7);
    assert_eq!(mask(&w, [0, 1, 0]), 0);
}

#[test]
fn diagonal_chunk_install_wakes_routes_through_loaded_corner_neighbors() {
    let mut w = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 2,
    });
    for [x, z] in [[0, 0], [1, 0], [0, 1]] {
        column(&mut w, x, z, false);
    }
    put(&mut w, [15, 1, 15], Block::WaterSource);
    put(&mut w, [13, 0, 15], Block::Air);
    settle(&mut w, 256);
    assert!(!wet(&w, [16, 1, 15]));
    // Install the diagonal chunk already containing a hole; no set_block event
    // may hide an insufficient lifecycle invalidation radius.
    let mut diagonal = World::default();
    column(&mut diagonal, 1, 1, false);
    put(&mut diagonal, [16, 0, 16], Block::Air);
    for (p, c) in diagonal.chunks() {
        w.insert_chunk(p, c.clone());
    }
    settle(&mut w, 256);
    assert_eq!(mask(&w, [15, 1, 15]), 1 | 2 | 8);
    assert!(wet(&w, [16, 0, 16]));
    assert_eq!(w.chunk_count(), 12);
}

#[test]
fn deterministic_obstacle_fixtures_converge_across_budgets_and_recede() {
    for seed in 0..16u64 {
        let mut w = basin();
        let mut random = seed + 1;
        for x in -5i32..=5 {
            for z in -5..=5 {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                if z == 0 && x.abs() <= 2 {
                    continue;
                }
                match random >> 61 {
                    0 => put(&mut w, [x, 0, z], Block::Air),
                    1 | 2 => put(&mut w, [x, 1, z], Block::Stone),
                    _ => (),
                }
            }
        }
        put(&mut w, [-2, 1, 0], Block::WaterSource);
        put(&mut w, [2, 1, 0], Block::WaterSource);
        let mut other = w.clone();
        settle(&mut w, 31);
        settle(&mut other, 256);
        assert_eq!(
            crate::persistence::encode(&w).unwrap(),
            crate::persistence::encode(&other).unwrap(),
            "seed {seed}"
        );
        for p in [[-2, 1, 0], [2, 1, 0]] {
            put(&mut w, p, Block::Air);
        }
        settle(&mut w, 256);
        assert_eq!(w.water_cells(), 0, "seed {seed}");
    }
}
