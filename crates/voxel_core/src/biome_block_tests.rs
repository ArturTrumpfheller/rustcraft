use crate::{
    Block, BlockPos, Chunk, ChunkPos, LocalPos, World,
    interaction::BuildBlock,
    inventory::{Inventory, Item, edit_inventory},
    physics::{PhysicsConfig, Player},
    raycast::Ray,
};
#[test]
fn new_materials_keep_old_item_ids_and_are_ordinary_solid_building_blocks() {
    let old = [
        Item::Grass,
        Item::Dirt,
        Item::Stone,
        Item::Wood,
        Item::Leaves,
        Item::Torch,
        Item::WaterSource,
        Item::EmptyBucket,
        Item::WaterBucket,
    ];
    for (i, item) in old.into_iter().enumerate() {
        assert_eq!(item.id(), i as u16 + 1);
        assert_eq!(Item::from_id(i as u16 + 1), Some(item));
    }
    for (block, item, id, build) in [
        (Block::Sand, Item::Sand, 10, BuildBlock::Sand),
        (
            Block::SnowyGround,
            Item::SnowyGround,
            11,
            BuildBlock::SnowyGround,
        ),
    ] {
        assert_eq!(item.id(), id);
        assert_eq!(Item::from_id(id), Some(item));
        assert_eq!(Item::from_block(block), Some(item));
        assert_eq!(item.build(), Some(build));
        assert!(Item::CREATIVE.contains(&item));
        assert_eq!(item.max_stack(), 64);
        assert!(
            block.is_collidable() && block.is_opaque() && block.is_cube() && block.is_targetable()
        );
        assert_eq!(block.emission(), 0);
        assert_eq!(block.fluid(), None);
        let mut world = World::default();
        let mut chunk = Chunk::default();
        chunk.set(LocalPos::new([0, 0, 0]).unwrap(), Block::Stone);
        world.insert_chunk(ChunkPos([0, 0, 0]), chunk);
        let mut inv = Inventory::empty();
        inv.insert(item, 5);
        let ray = Ray {
            origin: [0.5, 4., 0.5],
            direction: [0., -1., 0.],
            reach: 5.,
        };
        let player = Player::new([8., 8., 8.]);
        let cfg = PhysicsConfig::default();
        edit_inventory(&mut world, &mut inv, ray, true, &player, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(world.block(BlockPos([0, 1, 0])), block);
        assert_eq!(inv.selected_stack().unwrap().count(), 4);
        // Solid supports admit standing and wall torches using the same rules.
        world.set_block(BlockPos([0, 2, 0]), Block::Torch).unwrap();
        edit_inventory(&mut world, &mut inv, ray, false, &player, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(world.block(BlockPos([0, 2, 0])), Block::Air);
        edit_inventory(&mut world, &mut inv, ray, false, &player, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(world.block(BlockPos([0, 1, 0])), Block::Air);
        assert_eq!(inv.selected_stack().unwrap().count(), 5);
        world
            .set_block(BlockPos([0, 1, 0]), Block::WaterSource)
            .unwrap();
        edit_inventory(&mut world, &mut inv, ray, true, &player, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(world.block(BlockPos([0, 1, 0])), block);
        assert_eq!(world.water_cells(), 0);
        assert_eq!(Item::EmptyBucket.id(), 8);
        assert_eq!(Item::WaterBucket.max_stack(), 1);
    }
}

#[test]
fn material_lighting_ao_and_greedy_geometry_follow_existing_solid_rules() {
    use crate::{lighting::Skylight, mesh_chunk_lit};
    use std::collections::BTreeSet;
    let p = ChunkPos([0, 0, 0]);
    let targets = BTreeSet::from([p]);
    let mut reference = None;
    for material in [Block::Stone, Block::Sand, Block::SnowyGround] {
        let mut world = World::default();
        let mut chunk = Chunk::default();
        for z in 4..12 {
            for x in 4..12 {
                chunk.set(LocalPos::new([x, 4, z]).unwrap(), material);
            }
        }
        world.insert_chunk(p, chunk);
        let light = Skylight::for_chunks(&world, &targets);
        let mesh = mesh_chunk_lit(&world, p, &light);
        assert!(!mesh.indices.is_empty());
        let data = (
            mesh.positions,
            mesh.indices,
            mesh.colors,
            light.level(BlockPos([8, 3, 8])),
        );
        if let Some(ref expected) = reference {
            assert_eq!(&data, expected);
        } else {
            reference = Some(data);
        }
    }
}

#[test]
fn new_faces_use_their_own_tiles_and_snow_retains_a_dirt_underside() {
    use crate::texture::{Tile, face_tile};
    for n in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
        assert_eq!(face_tile(Block::Sand, n), Some(Tile::Sand));
        assert_eq!(face_tile(Block::SnowyGround, n), Some(Tile::SnowSide));
    }
    assert_eq!(
        face_tile(Block::SnowyGround, [0, 1, 0]),
        Some(Tile::SnowTop)
    );
    assert_eq!(face_tile(Block::SnowyGround, [0, -1, 0]), Some(Tile::Dirt));
    assert_eq!(Tile::Sand as u8, 8);
    assert_eq!(Tile::SnowTop as u8, 9);
    assert_eq!(Tile::SnowSide as u8, 10);
}
