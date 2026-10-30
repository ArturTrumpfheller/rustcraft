use crate::{
    Block, BlockPos, ChunkPos, World,
    interaction::{BuildBlock, EditError},
    inventory::{Inventory, Item, edit_inventory},
    physics::{PhysicsConfig, Player},
    raycast::{Ray, raycast},
};
const PLANTS: [Block; 5] = [
    Block::TallGrass,
    Block::Fern,
    Block::RedFlower,
    Block::YellowFlower,
    Block::DeadBush,
];
fn planted(b: Block) -> World {
    let mut w = World::default();
    w.set_block(
        BlockPos([0, 0, 0]),
        if b == Block::DeadBush {
            Block::Sand
        } else {
            Block::Grass
        },
    )
    .unwrap();
    w.set_block(BlockPos([0, 1, 0]), b).unwrap();
    w
}
#[test]
fn registry_ids_support_targeting_and_atomic_collection_are_consistent() {
    for id in 12..=20 {
        let i = Item::from_id(id).unwrap();
        assert_eq!(i.id(), id);
        assert!(Item::CREATIVE.contains(&i));
        let b = i.build().unwrap();
        assert_eq!(BuildBlock::from_slot(id as u8), Some(b));
        assert_eq!(Item::from_block(b.block()), Some(i));
        assert_eq!(b.slot(), id as u8);
    }
    assert_eq!(Item::from_id(21), None);
    assert_eq!(Item::WaterSource.id(), 7);
    for b in PLANTS {
        assert!(!b.is_collidable() && !b.is_opaque() && !b.is_cube() && b.is_targetable());
        assert!(b.fluid_replaceable());
        let mut w = planted(b);
        let ray = Ray {
            origin: [0.5, 1.3, 3.],
            direction: [0., 0., -1.],
            reach: 5.,
        };
        assert_eq!(raycast(&w, ray).unwrap().position, BlockPos([0, 1, 0]));
        assert_eq!(
            w.set_block(BlockPos([0, 0, 0]), Block::Air),
            Err(EditError::SupportsPlant)
        );
        let cfg = PhysicsConfig::default();
        let body = Player::new([8., 1., 8.]);
        let mut inv = Inventory::empty();
        inv.insert(Item::Stone, 36 * 64);
        let before = inv.clone();
        let revision = w.revision();
        assert!(edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg).is_err());
        assert_eq!(inv, before);
        assert_eq!(w.revision(), revision);
        inv = Inventory::empty();
        edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(
            inv.selected_stack().unwrap().item(),
            Item::from_block(b).unwrap()
        );
        assert_eq!(w.block(BlockPos([0, 1, 0])), Block::Air);
        assert!(
            edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg)
                .unwrap()
                .is_none()
        );
        // Placement from above consumes one and validates the supporting material.
        let down = Ray {
            origin: [0.5, 3., 0.5],
            direction: [0., -1., 0.],
            reach: 5.,
        };
        edit_inventory(&mut w, &mut inv, down, true, &body, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(w.block(BlockPos([0, 1, 0])), b);
        assert!(inv.selected_stack().is_none());
        let support = Ray {
            origin: [0.5, 0.5, 3.],
            direction: [0., 0., -1.],
            reach: 5.,
        };
        let batch = edit_inventory(&mut w, &mut inv, support, false, &body, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(batch.changes().len(), 2);
        assert_eq!(w.block(BlockPos([0, 1, 0])), Block::Air);
        assert_eq!(
            w.set_block(BlockPos([0, 2, 0]), b),
            Err(EditError::Unsupported)
        );
    }
}
#[test]
fn water_displaces_plants_once_and_never_makes_them_barriers() {
    for b in PLANTS {
        let mut w = planted(b);
        let p = BlockPos([0, 1, 0]);
        w.set_block(p, Block::WaterSource).unwrap();
        assert_eq!(w.dropped_items().len(), 1);
        w.set_block(p, Block::WaterSource).unwrap();
        assert_eq!(w.dropped_items().len(), 1);
        let (_, stack) = w.dropped_items().iter().next().unwrap();
        assert_eq!(stack.item(), Item::from_block(b).unwrap());
        assert_eq!(stack.count(), 1);
        assert_eq!(w.block(p), Block::WaterSource);
    }
    let mut w = planted(Block::Fern);
    for x in -6..=6 {
        for z in -6..=6 {
            w.set_block(BlockPos([x, 0, z]), Block::Stone).ok();
        }
    }
    w.set_block(BlockPos([1, 0, 0]), Block::Stone).unwrap();
    w.set_block(BlockPos([1, 1, 0]), Block::WaterSource)
        .unwrap();
    let mut changes = Vec::new();
    for _ in 0..12 {
        w.tick_fluids(crate::fluid::FluidConfig::default(), &mut changes);
    }
    assert!(w.block(BlockPos([0, 1, 0])).fluid().is_some());
    assert_eq!(w.dropped_items().len(), 1);
}
#[test]
fn crossed_quads_transmit_light_and_do_not_change_opaque_geometry_or_water() {
    use crate::{lighting::Skylight, mesh_chunk_lit};
    use std::collections::BTreeSet;
    let cp = ChunkPos([0, 0, 0]);
    for b in PLANTS {
        let w = planted(b);
        let lights = Skylight::for_chunks(&w, &BTreeSet::from([cp]));
        let m = mesh_chunk_lit(&w, cp, &lights);
        let mut empty = w.clone();
        empty.set_block(BlockPos([0, 1, 0]), Block::Air).unwrap();
        let bare = mesh_chunk_lit(
            &empty,
            cp,
            &Skylight::for_chunks(&empty, &BTreeSet::from([cp])),
        );
        assert_eq!(m.positions, bare.positions);
        assert_eq!(m.indices, bare.indices);
        assert_eq!(m.colors, bare.colors);
        assert_eq!(m.water, bare.water);
        assert_eq!(m.plants.positions.len(), 8);
        assert_eq!(m.plants.indices.len(), 12);
        assert!(bare.plants.positions.capacity() == 0);
        assert_eq!(lights.level(BlockPos([0, 1, 0])), 15);
        let tile = crate::texture::face_tile(b, [0, 0, 1]).unwrap().uv([0, 0]);
        assert!(m.plants.atlas_uvs.iter().all(|uv| *uv == tile));
        assert!(m.plants.colors.iter().all(|c| c[0] == 1. && c[2] == 1.));
        assert!(
            m.plants
                .positions
                .iter()
                .all(|p| p[1] <= 1. + b.plant_height())
        );
    }
}
