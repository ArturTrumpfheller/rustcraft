use super::*;
fn flat() -> World {
    let mut w = World::default();
    for x in -12..=12 {
        for z in -12..=12 {
            w.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
        }
    }
    w
}
fn put(w: &mut World, p: [i32; 3], b: Block) {
    w.set_block(BlockPos(p), b).unwrap();
}
fn settle(w: &mut World, budget: usize) -> u64 {
    let initial = w.fluid_updates().total_processed;
    let mut changes = Vec::new();
    for _ in 0..10000 {
        w.tick_fluids(FluidConfig { budget }, &mut changes);
        assert!(w.fluid_updates().processed <= budget);
        if w.fluid_updates().pending() == 0 {
            return w.fluid_updates().total_processed - initial;
        }
    }
    panic!(
        "fluid did not settle: {} pending",
        w.fluid_updates().pending()
    );
}
#[test]
fn source_stable_seven_steps_then_zero_idle_work() {
    let mut w = flat();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::WaterSource);
    for x in 1..=7 {
        assert_eq!(
            w.block(BlockPos([x, 1, 0])).fluid().unwrap().state_id(),
            x as u8
        );
    }
    assert_eq!(w.block(BlockPos([8, 1, 0])), Block::Air);
    let rev = w.revision();
    let mut changes = Vec::new();
    for _ in 0..100 {
        w.tick_fluids(FluidConfig::default(), &mut changes);
        assert_eq!(w.fluid_updates().processed, 0);
    }
    assert_eq!(w.revision(), rev);
    assert_eq!(std::mem::size_of::<Block>(), 1);
}
#[test]
fn waterfall_obstacles_channel_source_removal() {
    let mut w = flat();
    put(&mut w, [0, 6, 0], Block::WaterSource);
    settle(&mut w, 256);
    for y in 1..6 {
        assert_eq!(w.block(BlockPos([0, y, 0])), Block::WaterFalling);
    }
    assert_eq!(w.block(BlockPos([1, 5, 0])), Block::Air);
    // Excavate beneath the pool; water enters and falls to the lower boundary.
    put(&mut w, [2, 0, 0], Block::Air);
    settle(&mut w, 256);
    assert_eq!(w.block(BlockPos([2, -16, 0])), Block::WaterFalling);
    put(&mut w, [0, 6, 0], Block::Air);
    settle(&mut w, 256);
    assert_eq!(w.water_cells(), 0);
    let mut w = flat();
    put(&mut w, [1, 1, 0], Block::Stone);
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    assert_eq!(w.block(BlockPos([2, 1, 0])), Block::Water4); // shortest detour around obstacle
    put(&mut w, [0, 0, 0], Block::Air);
    settle(&mut w, 256);
    assert_eq!(w.block(BlockPos([0, -1, 0])), Block::WaterFalling);
    put(&mut w, [0, 1, 0], Block::Stone);
    settle(&mut w, 256);
    assert_eq!(w.water_cells(), 0);
}
#[test]
fn multiple_sources_budget_and_event_order_reach_same_fixed_point() {
    let mut a = flat();
    let mut b = flat();
    for p in [[-2, 1, 0], [2, 1, 0]] {
        put(&mut a, p, Block::WaterSource);
    }
    for p in [[2, 1, 0], [-2, 1, 0]] {
        put(&mut b, p, Block::WaterSource);
    }
    settle(&mut a, 7);
    settle(&mut b, 256);
    for x in -12..=12 {
        for z in -12..=12 {
            assert_eq!(a.block(BlockPos([x, 1, z])), b.block(BlockPos([x, 1, z])));
        }
    }
    put(&mut a, [-2, 1, 0], Block::Air);
    settle(&mut a, 256);
    assert!(a.water_cells() > 0);
    put(&mut a, [2, 1, 0], Block::Air);
    settle(&mut a, 256);
    assert_eq!(a.water_cells(), 0);
}
fn column(w: &mut World, x: i32) {
    for y in -1..2 {
        let mut c = crate::Chunk::default();
        if y == 0 {
            for lx in 0..16 {
                for z in 0..16 {
                    c.set(LocalPos::new([lx, 0, z]).unwrap(), Block::Stone);
                }
            }
        }
        w.insert_chunk(ChunkPos([x, y, 0]), c);
    }
}
#[test]
fn streaming_boundary_load_order_eviction_and_return() {
    let mut a = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 2,
    });
    column(&mut a, 0);
    put(&mut a, [15, 1, 8], Block::WaterSource);
    settle(&mut a, 256);
    assert_eq!(a.chunk_count(), 3);
    assert_eq!(a.fluid_updates().pending(), 0);
    column(&mut a, 1);
    settle(&mut a, 256);
    assert_eq!(a.block(BlockPos([16, 1, 8])), Block::Water1);
    let mut b = World::streaming(crate::StreamIdentity {
        seed: 42,
        generator: 2,
    });
    column(&mut b, 1);
    column(&mut b, 0);
    put(&mut b, [15, 1, 8], Block::WaterSource);
    settle(&mut b, 37);
    for x in 8..24 {
        for z in 1..15 {
            assert_eq!(a.block(BlockPos([x, 1, z])), b.block(BlockPos([x, 1, z])));
        }
    }
    let saved: Vec<_> = a
        .chunks()
        .filter(|(p, _)| p.0[0] == 1)
        .map(|(p, c)| (p, c.clone()))
        .collect();
    a.evict_column([1, 0]);
    put(&mut a, [15, 1, 8], Block::Air);
    settle(&mut a, 256);
    for (p, mut c) in saved {
        let edits = a.edits();
        for (&bp, &block) in edits.iter().filter(|(bp, _)| bp.split().0 == p) {
            c.set(bp.split().1, block);
        }
        a.insert_chunk(p, c);
    }
    settle(&mut a, 256);
    assert_eq!(a.water_cells(), 0);
}
#[test]
fn snapshot_roundtrip_and_historical_ids() {
    let mut w = flat();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    settle(&mut w, 256);
    let bytes = crate::persistence::encode(&w).unwrap();
    let mut loaded = crate::persistence::decode(&bytes).unwrap();
    settle(&mut loaded, 256);
    assert_eq!(crate::persistence::encode(&loaded).unwrap(), bytes);
    put(&mut loaded, [0, 1, 0], Block::Air);
    settle(&mut loaded, 256);
    assert_eq!(
        crate::persistence::decode(&crate::persistence::encode(&loaded).unwrap())
            .unwrap()
            .water_cells(),
        0
    );
    for id in 1..=6 {
        assert_eq!(crate::inventory::Item::from_id(id).unwrap().id(), id);
    }
    assert_eq!(crate::inventory::Item::WaterSource.id(), 7);
}
#[test]
fn water_movement_swimming_and_exit() {
    use crate::physics::*;
    let config = PhysicsConfig::default();
    let mut w = flat();
    for x in -4..=4 {
        for z in -4..=4 {
            for y in 1..=4 {
                put(&mut w, [x, y, z], Block::WaterSource);
            }
        }
    }
    let mut swimmer = Player::new([0.5, 1., 0.5]);
    let mut walker = swimmer;
    let land = flat();
    for _ in 0..60 {
        let input = MovementInput {
            direction: [1., 0.],
            ..Default::default()
        };
        swimmer.tick(&w, input, &config);
        walker.tick(&land, input, &config);
    }
    assert!(swimmer.position[0] < walker.position[0]);
    swimmer = Player::new([0.5, 1., 0.5]);
    for _ in 0..60 {
        swimmer.tick(
            &w,
            MovementInput {
                jump: true,
                ..Default::default()
            },
            &config,
        );
    }
    assert!(swimmer.position[1] > 2.);
    assert!(swimmer.velocity[1] > 0.);
    swimmer.position = [10., 10., 10.];
    swimmer.velocity = [0.; 3];
    assert!(!swimmer.in_water(&w, &config));
    swimmer.tick(&w, MovementInput::default(), &config);
    assert_eq!(swimmer.velocity[1], -config.gravity * PHYSICS_DT);
}
#[test]
fn mesh_height_culling_light_and_solid_buffers() {
    let mut w = flat();
    let cp = ChunkPos([0, 0, 0]);
    put(&mut w, [2, 1, 2], Block::WaterSource);
    put(&mut w, [3, 1, 2], Block::Water7);
    let targets = std::collections::BTreeSet::from([cp]);
    let light = crate::lighting::Skylight::for_chunks(&w, &targets);
    assert_eq!(light.level(BlockPos([2, 1, 2])), 15);
    let m = crate::mesh_chunk_lit(&w, cp, &light);
    assert!(!m.water.indices.is_empty());
    assert_eq!(m.water.indices.len(), 8 * 6); // bottom hidden by floor, shared side removed
    assert!(m.water.positions.iter().any(|p| p[1] > 1. && p[1] < 1.5));
    assert!(m.water.positions.iter().all(|p| p[1] <= 1. + 8. / 9.));
    let terrain = m.indices;
    put(&mut w, [2, 1, 2], Block::Air);
    put(&mut w, [3, 1, 2], Block::Air);
    assert_eq!(crate::mesh_chunk_lit(&w, cp, &light).indices, terrain);
}

#[test]
fn inventory_sources_displace_water_and_flowing_water_does_not_obstruct_rays() {
    use crate::{inventory::*, physics::*, raycast::*};
    let mut w = flat();
    let mut i = Inventory::starter();
    i.select(6);
    let p = Player::new([0.5, 1., 3.5]);
    let ray = Ray {
        origin: [0.5, 3., 0.5],
        direction: [0., -1., 0.],
        reach: 5.,
    };
    edit_inventory(&mut w, &mut i, ray, true, &p, &PhysicsConfig::default())
        .unwrap()
        .unwrap();
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::WaterSource);
    assert_eq!(i.selected_stack().unwrap().count(), 31);
    edit_inventory(&mut w, &mut i, ray, false, &p, &PhysicsConfig::default())
        .unwrap()
        .unwrap();
    assert_eq!(i.selected_stack().unwrap().count(), 32);
    put(&mut w, [0, 1, 0], Block::Water3);
    assert_eq!(raycast(&w, ray).unwrap().position, BlockPos([0, 0, 0]));
    i.select(2);
    edit_inventory(&mut w, &mut i, ray, true, &p, &PhysicsConfig::default())
        .unwrap()
        .unwrap();
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::Stone);
}
#[test]
fn cached_water_geometry_matches_light_channels_and_shared_chunk_edges() {
    let mut w = flat();
    put(&mut w, [0, 1, 0], Block::WaterSource);
    put(&mut w, [-1, 1, 0], Block::Water1);
    put(&mut w, [1, 1, 1], Block::Torch);
    let targets = std::collections::BTreeSet::from([ChunkPos([0; 3]), ChunkPos([-1, 0, 0])]);
    let light = crate::lighting::Skylight::for_chunks(&w, &targets);
    assert!(light.block_level(BlockPos([0, 1, 0])) > 0);
    assert_eq!(light.level(BlockPos([0, 1, 0])), 15);
    for cp in targets {
        assert_eq!(
            crate::water_mesh::mesh_water(&w, cp, &light),
            crate::water_mesh::mesh_water_cached(&w, cp, &light.chunk(cp))
        );
    }
    assert_eq!(
        crate::water_mesh::corner_height(&w, BlockPos([-1, 1, 0]), 1, 0),
        crate::water_mesh::corner_height(&w, BlockPos([0, 1, 0]), 0, 0)
    );
}
