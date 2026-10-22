use super::*;
use crate::{Block, Chunk};
fn room(origin: [i32; 3]) -> World {
    let mut w = World::default();
    for x in 0..9 {
        for y in 0..9 {
            for z in 0..9 {
                if x == 0 || x == 8 || y == 0 || y == 8 || z == 0 || z == 8 {
                    let p = BlockPos([origin[0] + x, origin[1] + y, origin[2] + z]);
                    let (cp, lp) = p.split();
                    let mut c = w.chunk(cp).cloned().unwrap_or_default();
                    c.set(lp, Block::Stone);
                    w.insert_chunk(cp, c);
                }
            }
        }
    }
    w
}
#[test]
fn direct_sky_sealed_room_and_open_close_aperture() {
    let mut w = room([0, 0, 0]);
    let targets = BTreeSet::from([ChunkPos([0; 3])]);
    let light = Skylight::for_chunks(&w, &targets);
    assert_eq!(light.level(BlockPos([4, 9, 4])), 15);
    assert_eq!(light.level(BlockPos([4, 4, 4])), 0);
    let hole = BlockPos([4, 8, 4]);
    w.set_block(hole, Block::Air).unwrap();
    let light = Skylight::for_chunks(&w, &targets);
    assert_eq!(light.level(BlockPos([4, 1, 4])), 15);
    assert_eq!(light.level(BlockPos([5, 4, 4])), 14);
    assert_eq!(light.level(BlockPos([6, 4, 4])), 13);
    w.set_block(hole, Block::Stone).unwrap();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([4, 4, 4])),
        0
    );
}
#[test]
fn pending_is_opaque_and_leaf_canopy_blocks_vertical_sun() {
    let mut w = crate::streaming::new_world(42);
    let targets = BTreeSet::from([ChunkPos([0; 3])]);
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([1, 10, 1])),
        0
    );
    let mut c = Chunk::default();
    c.set(LocalPos::new([1, 15, 1]).unwrap(), Block::Leaves);
    w.insert_chunk(ChunkPos([0, 1, 0]), Chunk::default());
    w.insert_chunk(ChunkPos([0, 0, 0]), c);
    let l = Skylight::for_chunks(&w, &targets);
    assert_eq!(l.level(BlockPos([1, 14, 1])), 14);
    assert_eq!(l.level(BlockPos([1, 15, 1])), 0);
}
#[test]
fn signed_xyz_seams_match_common_field_and_source_order() {
    for origin in [[-4, -4, -4], [12, 12, 12], [-20, -4, -20]] {
        let w = room(origin);
        let targets = w.chunks().map(|(p, _)| p).collect();
        let shared = Skylight::for_chunks(&w, &targets);
        for (p, _) in w.chunks() {
            let single = Skylight::for_chunks(&w, &BTreeSet::from([p]));
            assert_eq!(single.chunk(p), shared.chunk(p));
        }
        let mut reversed = World::default();
        for (p, c) in w.chunks().collect::<Vec<_>>().into_iter().rev() {
            reversed.insert_chunk(p, c.clone());
        }
        assert_eq!(
            Skylight::for_chunks(&reversed, &targets).levels,
            shared.levels
        );
    }
}
#[test]
fn modulation_is_monotonic_flat_neutral_and_has_readable_dark_floor() {
    for n in [[0, 1, 0], [0, -1, 0], [1, 0, 0], [0, 0, 1]] {
        let mut previous = 0.;
        for l in 0..=SKY {
            let c = face_color(l, n);
            assert!(c[0] > previous && c[0] <= 1.);
            assert_eq!(c, [c[0], c[0], c[0], 1.]);
            previous = c[0];
        }
    }
}

#[test]
fn tunnel_falloff_crosses_negative_seam_and_opening_roof_changes_below_other_y_chunk() {
    let mut w = World::default();
    for x in -25..=24 {
        for y in 0..=4 {
            for z in -2i32..=2 {
                if y == 0 || y == 4 || z.abs() == 2 || x == 24 {
                    w.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
                }
            }
        }
    }
    let targets = BTreeSet::from([ChunkPos([-1, 0, 0]), ChunkPos([0, 0, 0])]);
    let light = Skylight::for_chunks(&w, &targets);
    for x in -25..=0 {
        assert_eq!(
            light.level(BlockPos([x, 2, 0])),
            (14i32 - (x + 25)).max(0) as u8,
            "x={x}"
        );
    }
    for z in -1..=1 {
        for y in 1..4 {
            w.set_block(BlockPos([-25, y, z]), Block::Stone).unwrap();
        }
    }
    let light = Skylight::for_chunks(&w, &targets);
    assert_eq!(light.level(BlockPos([-16, 2, 0])), 0);
    w.set_block(BlockPos([0, 4, 0]), Block::Air).unwrap();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([0, 1, 0])),
        15
    );
    let mut w = room([-4, 12, -4]);
    let targets = w.chunks().map(|(p, _)| p).collect();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([0, 15, 0])),
        0
    );
    w.set_block(BlockPos([0, 20, 0]), Block::Air).unwrap();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([0, 15, 0])),
        15
    );
    w.set_block(BlockPos([0, 20, 0]), Block::Leaves).unwrap();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([0, 15, 0])),
        0
    );
}

#[test]
fn dense_leaf_canopy_is_dark_and_nearby_sun_does_not_cross_solid_cells() {
    let mut w = World::default();
    for x in -16..=16 {
        for z in -16..=16 {
            w.set_block(BlockPos([x, 20, z]), Block::Leaves).unwrap();
        }
    }
    let targets = BTreeSet::from([ChunkPos([0, 0, 0])]);
    let light = Skylight::for_chunks(&w, &targets);
    assert_eq!(light.level(BlockPos([0, 19, 0])), 0);
    assert_eq!(light.level(BlockPos([0, 20, 0])), 0);
    assert_eq!(light.level(BlockPos([0, 21, 0])), 15);
    w.set_block(BlockPos([0, 20, 0]), Block::Air).unwrap();
    assert_eq!(
        Skylight::for_chunks(&w, &targets).level(BlockPos([0, 0, 0])),
        15
    );
}

#[test]
fn optimized_roof_frontier_matches_independent_all_sky_source_flood() {
    use std::collections::BTreeMap;
    let mut seed = 7u64;
    for _ in 0..8 {
        let mut w = room([0; 3]);
        for _ in 0..40 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = ((seed >> 8) % 9) as i32;
            let z = ((seed >> 24) % 9) as i32;
            let y = ((seed >> 40) % 9) as i32;
            w.set_block(
                BlockPos([x, y, z]),
                if seed & 1 == 0 {
                    Block::Air
                } else {
                    Block::Leaves
                },
            )
            .unwrap();
        }
        let mut oracle = BTreeMap::new();
        let mut queue = VecDeque::new();
        for x in -1..=9 {
            for y in -1..=9 {
                for z in -1..=9 {
                    let p = BlockPos([x, y, z]);
                    if !w.block(p).is_solid() {
                        let sky =
                            (y + 1..MAX_Y).all(|cy| !w.block(BlockPos([x, cy, z])).is_solid());
                        oracle.insert(p, if sky { 15u8 } else { 0 });
                        if sky {
                            queue.push_back(p);
                        }
                    }
                }
            }
        }
        while let Some(p) = queue.pop_front() {
            let next = oracle[&p].saturating_sub(1);
            for offset in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                let n = p.offset(offset).unwrap();
                if let Some(value) = oracle.get_mut(&n)
                    && *value < next
                {
                    *value = next;
                    queue.push_back(n);
                }
            }
        }
        let light = Skylight::for_chunks(&w, &BTreeSet::from([ChunkPos([0; 3])]));
        for x in 1..8 {
            for y in 1..8 {
                for z in 1..8 {
                    let p = BlockPos([x, y, z]);
                    assert_eq!(
                        light.level(p),
                        oracle.get(&p).copied().unwrap_or(0),
                        "{p:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn light_batches_skip_unchanged_meshes_and_reuse_pristine_halo_generation() {
    use crate::{
        streaming::*,
        worldgen::{GenerationConfig, Generator},
    };
    let generator = Generator::new(GenerationConfig {
        seed: 42,
        version: 3,
    })
    .unwrap();
    let targets = BTreeSet::from([ChunkPos([0, 0, 0]), ChunkPos([0, 1, 0])]);
    let mut cache = TerrainCache::default();
    let first = remesh_lit(
        &generator,
        World::default(),
        Default::default(),
        &targets,
        &mut cache,
        &Default::default(),
        &targets,
    );
    assert_eq!(first.lighting.generated_columns, 9);
    let second = remesh_lit(
        &generator,
        World::default(),
        Default::default(),
        &targets,
        &mut cache,
        &first.lights,
        &Default::default(),
    );
    assert!(second.meshes.is_empty());
    assert_eq!(second.lighting.generated_columns, 0);
    assert_eq!(cache.columns(), 9);
    let other = Generator::new(GenerationConfig {
        seed: 43,
        version: 3,
    })
    .unwrap();
    let rebuilt = build_column_cached(
        &other,
        World::default(),
        Default::default(),
        [0, 0],
        0,
        &mut cache,
    );
    let reference = build_column(&other, crate::regions::EditSnapshot::default(), [0, 0], 0);
    assert_eq!(
        rebuilt.lighting.generated_columns, 9,
        "changing seed must invalidate pristine cache"
    );
    assert_eq!(rebuilt.chunks, reference.chunks);
    cache.retain(&BTreeSet::from([[1000, -1000]]));
    assert_eq!(cache.columns(), 0);
}

#[test]
fn derived_lighting_preserves_legacy_and_streamed_save_bytes_and_revisions() {
    use crate::{demo_world, persistence::encode, streaming::*};
    for mut w in [demo_world(), new_world(42), new_large_world(42)] {
        if w.identity().is_some() {
            prepare_spawn(&mut w).unwrap();
        }
        let bytes = encode(&w).unwrap();
        let revision = w.revision();
        let targets = w.chunks().map(|(p, _)| p).collect();
        let l = Skylight::for_chunks(&w, &targets);
        for (p, _) in w.chunks() {
            assert_eq!(
                crate::mesh_chunk_lit(&w, p, &l).colors.len(),
                crate::mesh_chunk(&w, p).positions.len()
            );
        }
        assert_eq!(encode(&w).unwrap(), bytes);
        assert_eq!(w.revision(), revision);
    }
}

#[test]
fn saved_world_v1_v2_v3_v4_items_and_far_lighting_reconstruct_after_restart() {
    use crate::{
        inventory::{Click, Item, edit_inventory},
        persistence::{SaveStore, encode},
        physics::{PhysicsConfig, Player},
        raycast::Ray,
        streaming::*,
    };
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn computed(w: &World) -> Vec<(ChunkPos, crate::ChunkMesh)> {
        if w.identity().is_some() {
            let id = w.identity().unwrap();
            let g = crate::worldgen::Generator::new(crate::worldgen::GenerationConfig {
                seed: id.seed,
                version: id.generator,
            })
            .unwrap();
            let c = if w.region_identity().is_some() {
                [1000, -1000]
            } else {
                [0, 0]
            };
            build_column(&g, w.overlay_snapshot(), c, w.revision())
                .chunks
                .into_iter()
                .map(|(p, _, m)| (p, m))
                .collect()
        } else {
            w.chunks()
                .map(|(p, _)| (p, crate::mesh_chunk(w, p)))
                .collect()
        }
    }
    fn prepare(store: &SaveStore, w: &mut World) {
        if w.identity().is_some() {
            let c = if w.region_identity().is_some() {
                [1000, -1000]
            } else {
                [0, 0]
            };
            let targets = (-1..2).map(|y| ChunkPos([c[0], y, c[1]])).collect();
            if let Some(id) = w.region_identity() {
                for r in lighting_regions(&targets) {
                    w.install_region(r, store.read_region(id, r).unwrap());
                }
            }
            prepare_column(w, c).unwrap();
        }
    }
    for version in 1..=4 {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let t = Temp(std::env::temp_dir().join(format!(
            "rustcraft-light-{}-{stamp}-{version}",
            std::process::id()
        )));
        std::fs::create_dir_all(&t.0).unwrap();
        if version == 1 {
            let mut b = encode(&crate::demo_world()).unwrap();
            b[8..12].copy_from_slice(&1u32.to_le_bytes());
            let mut h = crc32fast::Hasher::new();
            h.update(&b[..20]);
            h.update(&b[24..]);
            b[20..24].copy_from_slice(&h.finalize().to_le_bytes());
            std::fs::write(t.0.join("world.rcw"), b).unwrap();
        }
        let (store, mut w, _) = SaveStore::open_with(&t.0, || {
            Ok(match version {
                3 => new_world(42),
                4 => new_large_world(42),
                _ => crate::demo_world(),
            })
        })
        .unwrap();
        let mut inv = store.load_player(&mut w).unwrap();
        if version == 1 {
            assert_eq!(
                &std::fs::read(store.path()).unwrap()[8..12],
                &1u32.to_le_bytes()
            );
        }
        prepare(&store, &mut w);
        let p = if version == 4 {
            BlockPos([16008, -15, -15992])
        } else if version == 3 {
            BlockPos([8, -15, 8])
        } else {
            BlockPos([0, 0, 0])
        };
        assert!(w.block(p).is_solid());
        inv.select(8);
        inv.click(0, Click::Right);
        let ray = Ray {
            origin: p.0.map(|n| f64::from(n) + 0.5),
            direction: [0., -1., 0.],
            reach: 5.,
        };
        assert!(
            edit_inventory(
                &mut w,
                &mut inv,
                ray,
                false,
                &Player::new([100., 50., 100.]),
                &PhysicsConfig::default()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(w.block(p), Block::Air);
        assert!(
            inv.slots()
                .iter()
                .flatten()
                .any(|s| s.item() == Item::Stone)
        );
        let expected = computed(&w);
        let captured = inv.clone();
        store.save_player_session(&w, &inv, true).unwrap();
        let root = std::fs::read(store.path()).unwrap();
        assert_eq!(computed(&w), expected);
        assert_eq!(std::fs::read(store.path()).unwrap(), root);
        drop(store);
        let (store, mut w, _) =
            SaveStore::open_with(&t.0, || panic!("existing world must not invoke factory"))
                .unwrap();
        let inv = store.load_player(&mut w).unwrap();
        prepare(&store, &mut w);
        assert_eq!(inv.slots(), captured.slots());
        assert_eq!(inv.cursor(), captured.cursor());
        assert_eq!(inv.selected(), 8);
        assert_eq!(w.block(p), Block::Air);
        assert_eq!(computed(&w), expected);
    }
}
