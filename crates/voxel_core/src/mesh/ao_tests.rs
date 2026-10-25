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
        assert_eq!(m.colors[q * 4], {
            let sky = light.level(BlockPos([8, 2, 8]).offset(face.normal).unwrap());
            [
                crate::lighting::face_color(sky, face.normal)[0],
                crate::lighting::face_color(0, face.normal)[0],
                f32::from(sky) / 15.,
                1.,
            ]
        });
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
            assert!(colors[3][0] >= base[0] * 0.579);
            if sky == 0 {
                assert!(colors[3][0] >= base[0] * 0.729);
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
fn constant_and_transverse_affine_patterns_merge_without_stretching() {
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
        assert_eq!(floor.len(), 1);
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

#[test]
fn affine_fields_match_exactly_for_every_packed_pattern_and_face_orientation() {
    for face in &FACES {
        let axis = face.normal.iter().position(|&n| n != 0).unwrap();
        let [u, v] = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let canonical = [[0, 0], [1, 0], [0, 1]].map(|[cu, cv]| {
            face.corners
                .iter()
                .position(|c| c[u] == cu && c[v] == cv)
                .unwrap()
        });
        for packed in 0..=255 {
            let corners = ao::unpack(packed);
            assert_eq!(ao::pack(corners), packed);
            let field = ao::Affine::new(corners, canonical);
            assert_eq!(
                field.is_some(),
                corners[0] + corners[2] == corners[1] + corners[3]
            );
            let Some(field) = field else { continue };
            let [lo, hi_u, hi_v] = canonical.map(|i| i32::from(corners[i]));
            let reference =
                |x: usize, y: usize| lo + (hi_u - lo) * x as i32 + (hi_v - lo) * y as i32;
            for width in 1..=4 {
                for height in 1..=4 {
                    let outer = face
                        .corners
                        .map(|c| reference(usize::from(c[u]) * width, usize::from(c[v]) * height));
                    if !outer.iter().all(|a| (0..=3).contains(a)) {
                        continue;
                    }
                    assert_eq!(
                        field.extended(face, [u, v], [width, height]),
                        outer.map(|a| a as u8)
                    );
                    for x in 0..width {
                        for y in 0..height {
                            let expected = face.corners.map(|c| {
                                reference(x + usize::from(c[u]), y + usize::from(c[v])) as u8
                            });
                            assert!(field.matches(expected, [x, y]));
                            for k in 0..4 {
                                let mut wrong = expected;
                                wrong[k] = (wrong[k] + 1) % 4;
                                assert!(!field.matches(wrong, [x, y]));
                            }
                        }
                    }
                }
            }
            // Repeating a tuple along its gradient would reset the field at
            // the shared edge. Extending perpendicular to it is valid.
            assert_eq!(field.matches(corners, [1, 0]), hi_u == lo);
            assert_eq!(field.matches(corners, [0, 1]), hi_v == lo);
        }
    }
}

#[test]
fn cached_colors_preserve_the_formula_and_documented_endpoints() {
    for (f, face) in FACES.iter().enumerate() {
        for sky in 0..=15 {
            for ao in 0..=3 {
                assert_eq!(
                    ao::colors()[f][sky][ao],
                    ao::color(sky as u8, face.normal, ao as u8)
                );
            }
        }
    }
    for (sky, expected) in [(15, [1., 0.86, 0.72, 0.58]), (0, [1., 0.91, 0.82, 0.73])] {
        let base = crate::lighting::face_color(sky, [0, 1, 0])[0];
        for (ao, factor) in expected.into_iter().enumerate() {
            assert!((ao::colors()[2][usize::from(sky)][ao][0] / base - factor).abs() < 1e-6);
        }
    }
}

#[test]
fn visibility_halo_treats_unrepresentable_coordinates_as_air() {
    for axis in 0..3 {
        for sign in [-1, 1] {
            let mut cp = [0; 3];
            cp[axis] = if sign < 0 {
                i32::MIN.div_euclid(16)
            } else {
                i32::MAX.div_euclid(16)
            };
            let mut local = [8; 3];
            local[axis] = if sign < 0 { 0 } else { 15 };
            let mut w = World::default();
            w.insert_chunk(ChunkPos(cp), Chunk::default());
            let mut outside = cp;
            outside[axis] += sign;
            let mut solid = Chunk::default();
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        solid.set(LocalPos::new([x, y, z]).unwrap(), Block::Stone);
                    }
                }
            }
            // Even trusted raw chunk insertion cannot supply valid i32 block
            // positions for this neighbor. It must not hide a boundary face.
            w.insert_chunk(ChunkPos(outside), solid);
            let mut normal = [0; 3];
            normal[axis] = sign;
            let face = FACES.iter().find(|f| f.normal == normal).unwrap();
            let halo = ao::Occlusion::new(&w, ChunkPos(cp));
            assert!(!halo.solid(ao::Occlusion::index(local), ao::normal_offset(face)));
            let p = ChunkPos(cp)
                .block_position(LocalPos::new(local).unwrap())
                .unwrap();
            assert_eq!(halo.corners(local, face), reference_ao(&w, p, face));
            assert_eq!(halo.corners(local, face), [0; 4]);
        }
    }
}

#[test]
fn split_channels_preserve_noon_ao_and_leave_blocklight_independent_at_night() {
    for face in &FACES {
        for sky in 0..=15 {
            for block in 0..=15 {
                for occlusion in 0..=3 {
                    let s = ao::color(sky, face.normal, occlusion)[0];
                    let b = ao::color(block, face.normal, occlusion)[0];
                    assert_eq!(
                        s.max(b),
                        ao::color(sky.max(block), face.normal, occlusion)[0]
                    );
                    let night = (s * 0.1).max(b);
                    assert!(night >= b && night <= s.max(b));
                    if block >= sky {
                        assert_eq!(night, b);
                    }
                }
            }
        }
    }
}
