use super::*;
use crate::{Block, BlockPos, Chunk, StreamIdentity, lighting::Skylight};
use std::collections::BTreeSet;

// Deliberately query world positions, independently of the optimized halo.
pub(super) fn reference_ao(world: &World, p: BlockPos, face: &Face) -> [u8; 4] {
    let tangents: Vec<_> = (0..3).filter(|&a| face.normal[a] == 0).collect();
    face.corners.map(|corner| {
        let mut a = face.normal;
        let mut b = face.normal;
        a[tangents[0]] = if corner[tangents[0]] == 0 { -1 } else { 1 };
        b[tangents[1]] = if corner[tangents[1]] == 0 { -1 } else { 1 };
        let mut diagonal = a;
        diagonal[tangents[1]] = b[tangents[1]];
        let [a, b, c] =
            [a, b, diagonal].map(|d| p.offset(d).is_some_and(|p| world.block(p).is_solid()));
        if a && b {
            3
        } else {
            a as u8 + b as u8 + c as u8
        }
    })
}

fn put(world: &mut World, p: [i32; 3], b: Block) {
    let (cp, lp) = BlockPos(p).split();
    let mut chunk = world.chunk(cp).cloned().unwrap_or_default();
    chunk.set(lp, b);
    world.insert_chunk(cp, chunk);
}

#[test]
fn all_occupancy_combinations_and_orientations_at_signed_seams() {
    // All six faces, four corners, eight occupancy patterns, every combination
    // of signed chunk faces/edges/corners, and a fully interior location.
    for base in [[8; 3], [-16; 3], [-1; 3], [0; 3], [15; 3], [16; 3]] {
        for face in &FACES {
            let axes: Vec<_> = (0..3).filter(|&a| face.normal[a] == 0).collect();
            for corner in face.corners {
                for bits in 0..8 {
                    let mut w = World::default();
                    put(&mut w, base, Block::Stone);
                    let mut a = face.normal;
                    let mut b = face.normal;
                    a[axes[0]] = if corner[axes[0]] == 0 { -1 } else { 1 };
                    b[axes[1]] = if corner[axes[1]] == 0 { -1 } else { 1 };
                    let mut c = a;
                    c[axes[1]] = b[axes[1]];
                    for (i, d) in [a, b, c].into_iter().enumerate() {
                        if bits & (1 << i) != 0 {
                            put(&mut w, BlockPos(base).offset(d).unwrap().0, Block::Leaves);
                        }
                    }
                    let (cp, lp) = BlockPos(base).split();
                    let actual = ao::Occlusion::new(&w, cp).corners(lp.coordinates(), face);
                    assert_eq!(actual, reference_ao(&w, BlockPos(base), face));
                    let i = face.corners.iter().position(|&c| c == corner).unwrap();
                    let expected = if bits & 3 == 3 {
                        3
                    } else {
                        (bits as u8).count_ones() as u8
                    };
                    assert_eq!(actual[i], expected);
                }
            }
        }
    }
}

#[test]
fn isolated_and_flat_surfaces_remain_bright_and_merge() {
    let mut w = World::default();
    put(&mut w, [8, 2, 8], Block::Stone);
    let cp = ChunkPos([0; 3]);
    let m = mesh_chunk(&w, cp);
    assert_eq!(m.indices.len(), 36);
    let light = Skylight::for_chunks(&w, &BTreeSet::from([cp]));
    for (q, face) in FACES.iter().enumerate() {
        assert_eq!(reference_ao(&w, BlockPos([8, 2, 8]), face), [0; 4]);
        assert_eq!(
            m.colors[q * 4],
            crate::lighting::face_color(
                light.level(BlockPos([8, 2, 8]).offset(face.normal).unwrap()),
                face.normal
            )
        );
    }
    for x in 0..16 {
        for z in 0..16 {
            put(&mut w, [x, 2, z], Block::Stone);
        }
    }
    let m = mesh_chunk(&w, cp);
    assert_eq!(m.normals.iter().filter(|&&n| n == [0., 1., 0.]).count(), 4);
    greedy_tests::verify(&w, cp);
}

#[test]
fn opaque_enclosure_and_skylight_floor_are_preserved() {
    let mut w = World::default();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                put(&mut w, [x, y, z], Block::Stone);
            }
        }
    }
    // Center solid has no visible face; removing it creates six fully occluded
    // cavity faces with zero gameplay skylight, still readable after modulation.
    put(&mut w, [0; 3], Block::Air);
    let cp = ChunkPos([0; 3]);
    let light = Skylight::for_chunks(&w, &BTreeSet::from([cp]));
    assert_eq!(light.level(BlockPos([0; 3])), 0);
    for sky in 0..=15 {
        for face in &FACES {
            let base = crate::lighting::face_color(sky, face.normal);
            let colors = [0, 1, 2, 3].map(|a| ao::color(sky, face.normal, a));
            assert_eq!(colors[0], base);
            assert!(colors.windows(2).all(|p| p[0][0] > p[1][0]));
            assert!(colors[3][0] >= base[0] * 0.759);
            if sky == 0 {
                assert!(colors[3][0] >= base[0] * 0.915);
            }
            assert!(colors.iter().all(|c| c[3] == 1.));
        }
    }
    for (cp, _) in w.chunks() {
        greedy_tests::verify(&w, cp);
    }
}

#[test]
fn pending_streamed_neighbors_are_opaque_and_finite_outside_is_air() {
    let mut w = World::streaming(StreamIdentity {
        seed: 42,
        generator: 2,
    });
    w.insert_chunk(ChunkPos([0; 3]), Chunk::default());
    let p = BlockPos([15, 8, 15]);
    let face = &FACES[2];
    assert_eq!(
        ao::Occlusion::new(&w, p.split().0).corners(p.split().1.coordinates(), face),
        reference_ao(&w, p, face)
    );
    assert_eq!(reference_ao(&w, p, face)[1], 3);
    let p = BlockPos([63, 8, 63]);
    assert_eq!(
        ao::Occlusion::new(&w, p.split().0).corners(p.split().1.coordinates(), face)[1],
        0
    );
}

#[test]
fn edits_split_and_restore_greedy_geometry_without_touching_light_data() {
    let mut w = World::default();
    for x in 10..=18 {
        for z in 10..=18 {
            put(&mut w, [x, 0, z], Block::Stone);
        }
    }
    let targets: BTreeSet<_> = w.chunks().map(|(p, _)| p).collect();
    let initial: Vec<_> = targets.iter().map(|&cp| mesh_chunk(&w, cp)).collect();
    for _ in 0..8 {
        for b in [Block::Wood, Block::Air] {
            w.set_block(BlockPos([16, 1, 16]), b).unwrap();
            for &cp in &targets {
                greedy_tests::verify(&w, cp);
            }
        }
        for (&cp, m) in targets.iter().zip(&initial) {
            assert_eq!(&mesh_chunk(&w, cp), m);
        }
    }
}

#[test]
fn equal_nonuniform_patterns_do_not_stretch_but_uniform_occlusion_merges() {
    for walls in [1, 2] {
        let mut w = World::default();
        for x in 2..9 {
            put(&mut w, [x, 2, 2], Block::Stone);
        }
        for x in 1..10 {
            put(&mut w, [x, 3, 1], Block::Stone);
            if walls == 2 {
                put(&mut w, [x, 3, 3], Block::Stone);
            }
        }
        let m = mesh_chunk(&w, ChunkPos([0; 3]));
        let floor: Vec<_> = m
            .positions
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(q, p)| m.normals[q * 4] == [0., 1., 0.] && p.iter().all(|p| p[1] == 3.))
            .collect();
        assert_eq!(floor.len(), if walls == 1 { 7 } else { 1 });
        for (q, _) in floor {
            let c = &m.colors[q * 4..q * 4 + 4];
            assert_eq!(c.iter().all(|&v| v == c[0]), walls == 2);
        }
        greedy_tests::verify(&w, ChunkPos([0; 3]));
    }
}

#[test]
fn both_triangle_diagonals_follow_the_ao_field_with_outward_winding() {
    let mut seen = BTreeSet::new();
    for face in &FACES {
        for bits in 0..16 {
            let mut w = World::default();
            put(&mut w, [8; 3], Block::Stone);
            let axes: Vec<_> = (0..3).filter(|&a| face.normal[a] == 0).collect();
            // Vary the four diagonal occluders; side samples stay clear.
            for (i, c) in face.corners.iter().enumerate() {
                if bits & (1 << i) != 0 {
                    let mut d = face.normal;
                    for &a in &axes {
                        d[a] = if c[a] == 0 { -1 } else { 1 };
                    }
                    put(&mut w, BlockPos([8; 3]).offset(d).unwrap().0, Block::Stone);
                }
            }
            let m = mesh_chunk(&w, ChunkPos([0; 3]));
            let q = m
                .positions
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .find(|(q, p)| {
                    m.normals[q * 4] == face.normal.map(|n| n as f32)
                        && p.iter().all(|p| p.iter().all(|&a| (8. ..=9.).contains(&a)))
                })
                .unwrap()
                .0;
            let a = reference_ao(&w, BlockPos([8; 3]), face);
            let flip = a[0] + a[2] < a[1] + a[3];
            seen.insert(flip);
            let expected = if flip {
                [0, 1, 3, 1, 2, 3]
            } else {
                [0, 1, 2, 0, 2, 3]
            };
            assert_eq!(
                m.indices[q * 6..q * 6 + 6],
                expected.map(|i| i + (q * 4) as u32)
            );
            greedy_tests::verify(&w, ChunkPos([0; 3]));
        }
    }
    assert_eq!(seen, BTreeSet::from([false, true]));
}
