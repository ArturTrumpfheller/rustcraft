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
