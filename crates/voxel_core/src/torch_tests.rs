use crate::{
    Block, BlockPos, Chunk, ChunkPos, World,
    interaction::{BuildBlock, EditAction, EditError, edit},
    inventory::{Inventory, Item, edit_inventory},
    physics::{PhysicsConfig, Player},
    raycast::{Ray, raycast},
};
const TORCHES: [Block; 5] = [
    Block::Torch,
    Block::TorchEast,
    Block::TorchWest,
    Block::TorchSouth,
    Block::TorchNorth,
];
fn isolated(p: BlockPos, b: Block) -> World {
    let mut w = World::default();
    let (cp, lp) = p.split();
    let mut c = Chunk::default();
    c.set(lp, b);
    w.insert_chunk(cp, c);
    w
}
#[test]
fn exact_boxes_hit_each_orientation_from_multiple_faces_with_real_distances() {
    for b in TORCHES {
        for p in [
            BlockPos([0, 1, 0]),
            BlockPos([-1, 1, -16]),
            BlockPos([15, 1, 15]),
        ] {
            let w = isolated(p, b);
            let frame = b.shape_transform();
            for (start, dir, normal, expected) in [
                ([0.5, 0.25, 3.], [0., 0., -1.], [0., 0., 1.], 2.4375),
                ([-2., 0.25, 0.5], [1., 0., 0.], [-1., 0., 0.], 2.4375),
                ([3., 0.25, 0.5], [-1., 0., 0.], [1., 0., 0.], 2.4375),
                ([0.5, 2., 0.5], [0., -1., 0.], [0., 1., 0.], 1.3125),
            ] {
                let local = frame.point(start);
                let origin = std::array::from_fn(|a| f64::from(p.0[a]) + local[a]);
                let ray = Ray {
                    origin,
                    direction: frame.vector(dir),
                    reach: 5.,
                };
                let hit = raycast(&w, ray).unwrap();
                assert_eq!(hit.position, p, "{b:?}");
                assert!((hit.distance - expected).abs() < 1e-10, "{b:?} {hit:?}");
                assert_eq!(hit.adjacent, p.offset(hit.normal));
                for (a, b) in hit.surface_normal.into_iter().zip(frame.vector(normal)) {
                    assert!((a - b).abs() < 1e-10);
                }
                assert!(
                    raycast(
                        &w,
                        Ray {
                            reach: expected - 1e-5,
                            ..ray
                        }
                    )
                    .is_none()
                );
                assert!(
                    raycast(
                        &w,
                        Ray {
                            reach: expected,
                            ..ray
                        }
                    )
                    .is_some()
                );
            }
        }
    }
}
#[test]
fn empty_torch_cell_space_continues_to_the_cube_behind_it() {
    for x in [0.1, 0.41, 0.8] {
        let mut w = isolated(BlockPos([0, 1, 0]), Block::Torch);
        w.set_block(BlockPos([0, 1, -2]), Block::Stone).unwrap();
        let hit = raycast(
            &w,
            Ray {
                origin: [x, 1.25, 3.],
                direction: [0., 0., -1.],
                reach: 5.,
            },
        )
        .unwrap();
        assert_eq!(hit.position, BlockPos([0, 1, -2]));
        assert_eq!(hit.normal, [0, 0, 1]);
        assert_eq!(hit.distance, 4.);
    }
    // Empty corner of the tilted model's enclosing AABB must also miss.
    let w = isolated(BlockPos([0, 1, 0]), Block::TorchEast);
    assert!(
        raycast(
            &w,
            Ray {
                origin: [0.35, 1.2, 3.],
                direction: [0., 0., -1.],
                reach: 5.
            }
        )
        .is_none()
    );
    let w = isolated(BlockPos([0, 1, 0]), Block::Torch);
    let h = raycast(
        &w,
        Ray {
            origin: [0.5, 1.25, 0.5],
            direction: [0., 1., 0.],
            reach: 0.,
        },
    )
    .unwrap();
    assert_eq!(h.distance, 0.);
    assert_eq!(h.adjacent, None);
}
#[test]
fn shape_boxes_are_orthonormal_inside_the_cell_and_match_noncolliding_emitters() {
    for b in TORCHES {
        assert_eq!(b.emission(), 14);
        assert!(!b.is_collidable());
        assert!(!b.is_opaque());
        assert!(!b.is_cube());
        assert_eq!(Item::from_block(b), Some(Item::Torch));
        assert_eq!(Item::Torch.id(), 6);
        let frame = b.shape_transform();
        for bounds in b.selection_boxes() {
            for p in bounds.corners() {
                let world = frame.point(p);
                assert!(
                    world
                        .into_iter()
                        .all(|v| (-1e-12..=1. + 1e-12).contains(&v)),
                    "{b:?} {world:?}"
                );
                for (a, b) in p.into_iter().zip(frame.inverse_point(world)) {
                    assert!((a - b).abs() < 1e-12);
                }
            }
        }
    }
}
#[test]
fn placement_uses_clicked_face_and_every_torch_collects_the_same_item() {
    let support = BlockPos([0, 1, 0]);
    let cfg = PhysicsConfig::default();
    let body = Player::new([8., 2., 8.]);
    for normal in [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
        let mut w = World::default();
        w.set_block(support, Block::Stone).unwrap();
        let ray = Ray {
            origin: std::array::from_fn(|a| {
                f64::from(support.0[a]) + 0.5 + f64::from(normal[a]) * 3.
            }),
            direction: normal.map(|v| -f64::from(v)),
            reach: 5.,
        };
        let mut inv = Inventory::starter();
        inv.select(5);
        let change = edit_inventory(&mut w, &mut inv, ray, true, &body, &cfg)
            .unwrap()
            .unwrap();
        let change = change.changes()[0];
        assert_eq!(change.position, support.offset(normal).unwrap());
        assert_eq!(change.after, Block::torch_on_face(normal).unwrap());
        assert_eq!(inv.selected_stack().unwrap().count(), 31);
        let revision = w.revision();
        assert_eq!(
            w.set_block(support, Block::Air),
            Err(EditError::SupportsTorch)
        );
        assert_eq!(w.revision(), revision);
        edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(inv.selected_stack().unwrap().count(), 32);
        assert_eq!(w.block(change.position), Block::Air);
        w.set_block(support, Block::Air).unwrap();
    }
    let mut w = World::default();
    w.set_block(support, Block::Stone).unwrap();
    let ray = Ray {
        origin: [0.5, -1., 0.5],
        direction: [0., 1., 0.],
        reach: 5.,
    };
    let mut inv = Inventory::starter();
    inv.select(5);
    let before = inv.clone();
    let revision = w.revision();
    assert!(edit_inventory(&mut w, &mut inv, ray, true, &body, &cfg).is_err());
    assert_eq!(inv, before);
    assert_eq!(w.revision(), revision);
    assert_eq!(
        edit(
            &mut w,
            ray,
            EditAction::Place(BuildBlock::Torch),
            &body,
            &cfg
        ),
        Err(EditError::Unsupported)
    );
}
#[test]
fn unloaded_or_missing_wall_support_fails_without_mutation() {
    let mut w = World::default();
    assert_eq!(
        w.set_block(BlockPos([0, 1, 0]), Block::TorchEast),
        Err(EditError::Unsupported)
    );
    let mut w = crate::streaming::new_world(42);
    w.insert_chunk(ChunkPos([1, 1, 0]), Chunk::default());
    let revision = w.revision();
    assert_eq!(
        w.set_block(BlockPos([16, 25, 8]), Block::TorchEast),
        Err(EditError::NotLoaded)
    );
    assert_eq!(w.revision(), revision);
}

#[test]
fn mount_orientation_does_not_change_light_channels_or_block_storage_size() {
    use crate::lighting::Skylight;
    use std::collections::BTreeSet;
    assert_eq!(std::mem::size_of::<Block>(), 1);
    let p = BlockPos([-1, 16, -1]);
    let targets = BTreeSet::from([p.split().0, ChunkPos([0, 1, 0])]);
    let standing = Skylight::for_chunks(&isolated(p, Block::Torch), &targets);
    for b in TORCHES {
        let light = Skylight::for_chunks(&isolated(p, b), &targets);
        assert_eq!(light.block_level(p), 14);
        assert_eq!(light.level(p), 15);
        for &cp in &targets {
            assert_eq!(light.chunk(cp), standing.chunk(cp));
        }
        assert_eq!(light.bytes(), standing.bytes());
    }
}
