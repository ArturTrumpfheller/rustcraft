mod ao;

use crate::{CHUNK_SIZE, ChunkPos, LocalPos, World};

/// Engine-neutral mesh buffers. Vertices are local to the chunk origin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Constant atlas tile origin per quad; uvs are now block-repeat coordinates.
    pub atlas_uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

struct Face {
    normal: [i32; 3],
    corners: [[u8; 3]; 4],
}

// Counter-clockwise when viewed from outside the solid block.
const FACES: [Face; 6] = [
    Face {
        normal: [1, 0, 0],
        corners: [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
    },
    Face {
        normal: [-1, 0, 0],
        corners: [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]],
    },
    Face {
        normal: [0, 1, 0],
        corners: [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]],
    },
    Face {
        normal: [0, -1, 0],
        corners: [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
    },
    Face {
        normal: [0, 0, 1],
        corners: [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]],
    },
    Face {
        normal: [0, 0, -1],
        corners: [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]],
    },
];

/// Emits only faces adjacent to air, consulting neighboring chunks too.
/// Missing legacy chunks are Air; pending streamed chunks are opaque barriers.
/// Coordinates beyond i32 space are Air.
pub fn mesh_chunk(world: &World, position: ChunkPos) -> ChunkMesh {
    if world.chunk(position).is_none() {
        return ChunkMesh::default();
    }
    let light =
        crate::lighting::Skylight::for_chunks(world, &std::collections::BTreeSet::from([position]));
    mesh_chunk_lit(world, position, &light)
}
/// Mesh using a shared, authoritative light computation for an entire batch.
pub fn mesh_chunk_lit(
    world: &World,
    position: ChunkPos,
    light: &crate::lighting::Skylight,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    let Some(chunk) = world.chunk(position) else {
        return mesh;
    };
    let Some(origin) = position.block_position(LocalPos::new([0; 3]).unwrap()) else {
        return mesh;
    };
    // Fixed visibility planes, filled in one storage-order pass. Empty slices
    // are skipped. Only exact affine AO fields may grow into larger rectangles.
    #[derive(Clone, Copy, PartialEq)]
    struct MergeKey {
        tile: crate::texture::Tile,
        level: u8,
        ao: u8,
    }
    let colors = ao::colors();
    let occlusion = ao::Occlusion::new(world, position);
    let ao_offsets = FACES.each_ref().map(ao::offsets);
    let mut active_slices = [0u16; 6];
    let mut planes = [[[None; CHUNK_SIZE * CHUNK_SIZE]; CHUNK_SIZE]; 6];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = [x, y, z];
                let local = LocalPos::new(p).unwrap();
                let block = chunk.get(local);
                if block.is_torch() {
                    let bp = origin.offset(p.map(|v| v as i32)).unwrap();
                    append_torch(
                        &mut mesh,
                        block,
                        p.map(|v| v as f32),
                        light.combined_level(bp),
                    );
                }
                if !block.is_cube() {
                    continue;
                }
                let center = ao::Occlusion::index(p);
                for (f, face) in FACES.iter().enumerate() {
                    let axis = f / 2;
                    let [u, v] = match axis {
                        0 => [1, 2],
                        1 => [0, 2],
                        _ => [0, 1],
                    };
                    let solid = occlusion.solid(center, ao::normal_offset(face));
                    if !solid {
                        active_slices[f] |= 1 << p[axis];
                        planes[f][p[axis]][p[v] * CHUNK_SIZE + p[u]] = Some(MergeKey {
                            tile: crate::texture::face_tile(block, face.normal).unwrap(),
                            level: origin
                                .offset(std::array::from_fn(|a| p[a] as i32 + face.normal[a]))
                                .map_or(0, |p| light.combined_level(p)),
                            ao: ao::pack(occlusion.sample_index(center, ao_offsets[f])),
                        });
                    }
                }
            }
        }
    }
    for (f, face) in FACES.iter().enumerate() {
        let axis = f / 2;
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
        for (slice, mask) in planes[f].iter_mut().enumerate() {
            if active_slices[f] & (1 << slice) == 0 {
                continue;
            }
            for j in 0..CHUNK_SIZE {
                let mut i = 0;
                while i < CHUNK_SIZE {
                    let Some(key) = mask[j * CHUNK_SIZE + i] else {
                        i += 1;
                        continue;
                    };
                    // Identical tuples alone do not preserve a repeated gradient.
                    // Match each unit face against one common integer affine field.
                    let corners = ao::unpack(key.ao);
                    let uniform = corners.iter().all(|&a| a == corners[0]);
                    let field = if uniform {
                        None
                    } else {
                        ao::Affine::new(corners, canonical)
                    };
                    let mergeable = uniform || field.is_some();
                    let compatible = |cell: Option<MergeKey>, offset| {
                        if uniform {
                            return cell == Some(key);
                        }
                        cell.is_some_and(|candidate| {
                            candidate.tile == key.tile
                                && candidate.level == key.level
                                && field
                                    .as_ref()
                                    .is_some_and(|a| a.matches(ao::unpack(candidate.ao), offset))
                        })
                    };
                    let mut width = 1;
                    while mergeable
                        && i + width < CHUNK_SIZE
                        && compatible(mask[j * CHUNK_SIZE + i + width], [width, 0])
                    {
                        width += 1;
                    }
                    let mut height = 1;
                    while mergeable
                        && j + height < CHUNK_SIZE
                        && (0..width).all(|x| {
                            compatible(mask[(j + height) * CHUNK_SIZE + i + x], [x, height])
                        })
                    {
                        height += 1;
                    }
                    let ao = if width == 1 && height == 1 {
                        corners
                    } else {
                        field
                            .as_ref()
                            .map_or(corners, |a| a.extended(face, [u, v], [width, height]))
                    };
                    for y in j..j + height {
                        mask[y * CHUNK_SIZE + i..y * CHUNK_SIZE + i + width].fill(None);
                    }
                    let mut p = [0.; 3];
                    p[axis] = slice as f32;
                    p[u] = i as f32;
                    p[v] = j as f32;
                    let mut extent = [1.; 3];
                    extent[u] = width as f32;
                    extent[v] = height as f32;
                    let start = mesh.positions.len() as u32;
                    for corner in face.corners {
                        mesh.positions.push(std::array::from_fn(|a| {
                            p[a] + f32::from(corner[a]) * extent[a]
                        }));
                        mesh.normals.push(face.normal.map(|n| n as f32));
                    }
                    let (w, h) = if axis == 1 {
                        (extent[0], extent[2])
                    } else {
                        (extent[if axis == 0 { 2 } else { 0 }], extent[1])
                    };
                    mesh.uvs.extend(if axis == 1 {
                        [[0., h], [w, h], [w, 0.], [0., 0.]]
                    } else {
                        [[w, h], [w, 0.], [0., 0.], [0., h]]
                    });
                    mesh.atlas_uvs.extend([key.tile.uv([0, 0]); 4]);
                    mesh.colors
                        .extend(ao.map(|a| colors[f][usize::from(key.level)][usize::from(a)]));
                    // Connect the darker opposite pair to avoid a bright crease.
                    let indices = if ao[0] + ao[2] < ao[1] + ao[3] {
                        [0, 1, 3, 1, 2, 3]
                    } else {
                        [0, 1, 2, 0, 2, 3]
                    };
                    mesh.indices.extend(indices.map(|i| start + i));
                    i += width;
                }
            }
        }
    }
    mesh
}

/// Original twelve-quad model, centered in its cell; also reused by the hand.
/// Texture V 0..1/4 is the flame, V 1/4..1 is the wooden shaft.
pub fn torch_mesh() -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    append_torch(&mut mesh, crate::Block::Torch, [0.; 3], 15);
    mesh
}
fn append_torch(mesh: &mut ChunkMesh, block: crate::Block, origin: [f32; 3], level: u8) {
    let transform = block.shape_transform();
    for (bounds, v0, v1, glowing) in [
        (crate::shape::TORCH_BOXES[0], 0.25, 1., false),
        (crate::shape::TORCH_BOXES[1], 0., 0.25, true),
    ] {
        let lo = bounds.min;
        let hi = bounds.max;
        for face in &FACES {
            let start = mesh.positions.len() as u32;
            for corner in face.corners {
                let local = std::array::from_fn(|a| lo[a] + f64::from(corner[a]) * (hi[a] - lo[a]));
                let p = transform.point(local);
                mesh.positions
                    .push(std::array::from_fn(|a| origin[a] + p[a] as f32));
                mesh.normals.push(
                    transform
                        .vector(face.normal.map(f64::from))
                        .map(|n| n as f32),
                );
            }
            mesh.uvs.extend([[0., v1], [1., v1], [1., v0], [0., v0]]);
            mesh.atlas_uvs
                .extend([crate::texture::Tile::Torch.uv([0, 0]); 4]);
            mesh.colors.extend(
                [if glowing {
                    [1.; 4]
                } else {
                    crate::lighting::face_color(level, face.normal)
                }; 4],
            );
            mesh.indices.extend([0, 1, 2, 0, 2, 3].map(|i| start + i));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, BlockPos, Chunk};

    fn world_with_blocks(positions: &[[i32; 3]]) -> World {
        let mut chunks = std::collections::BTreeMap::<ChunkPos, Chunk>::new();
        for &position in positions {
            let (chunk, local) = BlockPos(position).split();
            chunks.entry(chunk).or_default().set(local, Block::Grass);
        }
        let mut world = World::default();
        for (position, chunk) in chunks {
            world.insert_chunk(position, chunk);
        }
        world
    }

    fn face_count(world: &World) -> usize {
        world
            .chunks()
            .map(|(position, _)| super::greedy_tests::unit_area(&mesh_chunk(world, position)))
            .sum()
    }

    #[test]
    fn empty_world_and_air_chunk_have_no_geometry() {
        let mut world = World::default();
        assert_eq!(mesh_chunk(&world, ChunkPos([0; 3])), ChunkMesh::default());
        world.insert_chunk(ChunkPos([0; 3]), Chunk::default());
        assert_eq!(face_count(&world), 0);
    }

    #[test]
    fn adjacent_blocks_hide_the_shared_face() {
        assert_eq!(face_count(&world_with_blocks(&[[0, 0, 0]])), 6);
        assert_eq!(face_count(&world_with_blocks(&[[0, 0, 0], [1, 0, 0]])), 10);
    }

    #[test]
    fn adjacency_across_chunk_boundaries_hides_faces_in_every_axis() {
        for axis in 0..3 {
            for boundary in [0, 16, -16] {
                let mut a = [0; 3];
                let mut b = [0; 3];
                a[axis] = boundary - 1;
                b[axis] = boundary;
                assert_eq!(face_count(&world_with_blocks(&[a, b])), 10);
            }
        }
    }

    #[test]
    fn full_chunk_only_emits_surface_and_cavity_faces() {
        let blocks: Vec<_> = (0..16)
            .flat_map(|x| (0..16).flat_map(move |y| (0..16).map(move |z| [x, y, z])))
            .collect();
        let mut world = world_with_blocks(&blocks);
        assert_eq!(face_count(&world), 6 * 16 * 16);
        let mut chunk = world.chunk(ChunkPos([0; 3])).unwrap().clone();
        chunk.set(LocalPos::new([8; 3]).unwrap(), Block::Air);
        world.insert_chunk(ChunkPos([0; 3]), chunk);
        assert_eq!(face_count(&world), 6 * 16 * 16 + 6);
    }

    #[test]
    fn triangles_have_outward_winding_and_consistent_vertex_attributes() {
        let world = world_with_blocks(&[[0, 0, 0]]);
        let mesh = mesh_chunk(&world, ChunkPos([0; 3]));
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.uvs.len(), 24);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = mesh.positions[triangle[0] as usize];
            let b = mesh.positions[triangle[1] as usize];
            let c = mesh.positions[triangle[2] as usize];
            let ab: [f32; 3] = std::array::from_fn(|axis| b[axis] - a[axis]);
            let ac: [f32; 3] = std::array::from_fn(|axis| c[axis] - a[axis]);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            assert_eq!(cross, mesh.normals[triangle[0] as usize]);
        }
    }
}

#[cfg(test)]
mod texture_tests {
    use super::*;
    use crate::{
        Block, BlockPos, Chunk,
        texture::{Tile, face_tile},
    };

    #[test]
    fn every_material_has_upright_uvs_and_unchanged_face_geometry_and_indices() {
        // Expected UV corner order in the existing CCW face order: walls start
        // at image bottom-right; horizontal planes start at image bottom-left.
        let wall = [[1, 1], [1, 0], [0, 0], [0, 1]];
        let horizontal = [[0, 1], [1, 1], [1, 0], [0, 0]];
        for block in [
            Block::Grass,
            Block::Dirt,
            Block::Stone,
            Block::Wood,
            Block::Leaves,
        ] {
            let mut chunk = Chunk::default();
            chunk.set(LocalPos::new([0; 3]).unwrap(), block);
            let mut world = World::default();
            world.insert_chunk(ChunkPos([0; 3]), chunk);
            let m = mesh_chunk(&world, ChunkPos([0; 3]));
            assert_eq!(m.uvs.len(), m.positions.len());
            for (i, face) in FACES.iter().enumerate() {
                assert_eq!(
                    m.atlas_uvs[4 * i],
                    face_tile(block, face.normal).unwrap().uv([0, 0])
                );
                let uv = if face.normal[1] == 0 {
                    wall
                } else {
                    horizontal
                };
                for (j, &expected_uv) in uv.iter().enumerate() {
                    assert_eq!(m.positions[4 * i + j], face.corners[j].map(f32::from));
                    assert_eq!(m.normals[4 * i + j], face.normal.map(|v| v as f32));
                    assert_eq!(m.uvs[4 * i + j], expected_uv.map(|v| v as f32));
                }
                let s = (4 * i) as u32;
                assert_eq!(
                    m.indices[6 * i..6 * i + 6],
                    [s, s + 1, s + 2, s, s + 2, s + 3]
                );
            }
        }
    }

    #[test]
    fn mixed_textures_cull_shared_faces_and_keep_uvs_across_all_signed_chunk_seams() {
        for axis in 0..3 {
            for edge in [-16, 0, 16] {
                let mut a = [2; 3];
                a[axis] = edge - 1;
                let mut b = a;
                b[axis] = edge;
                let mut world = World::default();
                for (p, block) in [(BlockPos(a), Block::Grass), (BlockPos(b), Block::Wood)] {
                    let (cp, lp) = p.split();
                    let mut chunk = Chunk::default();
                    chunk.set(lp, block);
                    world.insert_chunk(cp, chunk);
                }
                for (p, block) in [(BlockPos(a), Block::Grass), (BlockPos(b), Block::Wood)] {
                    let cp = p.split().0;
                    let m = mesh_chunk(&world, cp);
                    assert_eq!(m.indices.len(), 30);
                    assert_eq!(m.uvs.len(), 20);
                    for i in 0..5 {
                        let n = m.normals[4 * i].map(|v| v as i32);
                        let tile = face_tile(block, n).unwrap();
                        assert_eq!(m.atlas_uvs[4 * i], tile.uv([0, 0]));
                        let min = [0., 0.];
                        let max = [1., 1.];
                        for uv in &m.uvs[4 * i..4 * i + 4] {
                            assert!(
                                uv.iter()
                                    .enumerate()
                                    .all(|(a, &v)| v == min[a] || v == max[a])
                            );
                        }
                    }
                }
                // No culling/texture policy change for opaque leaves.
                assert_eq!(face_tile(Block::Leaves, [0, 1, 0]), Some(Tile::Leaves));
            }
        }
    }
}

#[cfg(test)]
mod greedy_tests {
    use super::*;
    use crate::{Block, Chunk, lighting::Skylight};
    use std::collections::{BTreeMap, BTreeSet};
    pub(super) fn unit_area(m: &ChunkMesh) -> usize {
        m.positions
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let a: f32 = (0..3).map(|i| (p[1][i] - p[0][i]).abs()).sum();
                let b: f32 = (0..3).map(|i| (p[3][i] - p[0][i]).abs()).sum();
                (a * b) as usize
            })
            .sum()
    }
    pub(super) fn verify(world: &World, cp: ChunkPos) {
        let light = Skylight::for_chunks(world, &BTreeSet::from([cp]));
        let m = mesh_chunk_lit(world, cp, &light);
        assert_eq!(m, mesh_chunk_lit(world, cp, &light));
        let mut expected = BTreeMap::new();
        if let Some(c) = world.chunk(cp) {
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let lp = LocalPos::new([x, y, z]).unwrap();
                        let block = c.get(lp);
                        let Some(bp) = cp.block_position(lp) else {
                            continue;
                        };
                        if !block.is_solid() {
                            continue;
                        }
                        for (f, face) in FACES.iter().enumerate() {
                            if bp
                                .offset(face.normal)
                                .is_some_and(|n| world.block(n).is_solid())
                            {
                                continue;
                            }
                            expected.insert(
                                ([x, y, z], f),
                                (
                                    crate::texture::face_tile(block, face.normal)
                                        .unwrap()
                                        .uv([0, 0]),
                                    super::ao_tests::reference_ao(world, bp, face).map(|a| {
                                        ao::color(
                                            bp.offset(face.normal).map_or(0, |p| light.level(p)),
                                            face.normal,
                                            a,
                                        )
                                    }),
                                ),
                            );
                        }
                    }
                }
            }
        }
        let mut actual = BTreeMap::new();
        for q in 0..m.indices.len() / 6 {
            let ps = &m.positions[q * 4..q * 4 + 4];
            let n = m.normals[q * 4].map(|x| x as i32);
            let f = FACES.iter().position(|face| face.normal == n).unwrap();
            let axis = n.iter().position(|&x| x != 0).unwrap();
            let mut lo: [usize; 3] =
                std::array::from_fn(|a| ps.iter().map(|p| p[a] as usize).min().unwrap());
            let mut hi: [usize; 3] =
                std::array::from_fn(|a| ps.iter().map(|p| p[a] as usize).max().unwrap());
            if n[axis] > 0 {
                lo[axis] -= 1;
            }
            hi[axis] = lo[axis] + 1;
            for x in lo[0]..hi[0] {
                for y in lo[1]..hi[1] {
                    for z in lo[2]..hi[2] {
                        let colors = face_colors_at_unit(&m, q, [x, y, z], &FACES[f]);
                        verify_unit_interiors(
                            &m,
                            q,
                            [x, y, z],
                            &FACES[f],
                            expected[&([x, y, z], f)].1,
                        );
                        assert!(
                            actual
                                .insert(([x, y, z], f), (m.atlas_uvs[q * 4], colors))
                                .is_none(),
                            "overlap"
                        );
                    }
                }
            }
            for k in 0..4 {
                assert_eq!(m.atlas_uvs[q * 4 + k], m.atlas_uvs[q * 4]);
            }
            let a: [f32; 3] = std::array::from_fn(|i| ps[1][i] - ps[0][i]);
            let b: [f32; 3] = std::array::from_fn(|i| ps[2][i] - ps[0][i]);
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            assert!(cross.iter().zip(n).map(|(a, b)| a * b as f32).sum::<f32>() > 0.);
            // Repetition measures geometry in block units, for walls and planes.
            let uv = &m.uvs[q * 4..q * 4 + 4];
            let uv_area = (uv[1][0] - uv[0][0]).abs() + (uv[1][1] - uv[0][1]).abs();
            assert_eq!(uv_area, a.iter().map(|v| v.abs()).sum::<f32>());
        }
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "coverage at {cp:?}"
        );
        for (key, (tile, colors)) in &actual {
            let (expected_tile, expected_colors) = &expected[key];
            assert_eq!(tile, expected_tile);
            for (a, b) in colors
                .iter()
                .flatten()
                .zip(expected_colors.iter().flatten())
            {
                assert!(
                    (a - b).abs() < 1e-6,
                    "AO/light at {cp:?} {key:?}: {colors:?} != {expected_colors:?}"
                );
            }
            // A merged rectangle must have the same piecewise-linear field as
            // every unit face, including internal boundaries and both diagonals.
        }
    }
    fn face_colors_at_unit(m: &ChunkMesh, q: usize, p: [usize; 3], face: &Face) -> [[f32; 4]; 4] {
        let axis = face.normal.iter().position(|&n| n != 0).unwrap();
        let [u, v] = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        // A merged quad is affine. Matching every unit corner against the
        // independent oracle proves both triangles and all internal boundaries.
        let quad_colors = &m.colors[q * 4..q * 4 + 4];
        let ps = &m.positions[q * 4..q * 4 + 4];
        let area: f32 = (0..3).map(|a| (ps[1][a] - ps[0][a]).abs()).sum::<f32>()
            * (0..3).map(|a| (ps[3][a] - ps[0][a]).abs()).sum::<f32>();
        if area > 1. {
            let diagonal_02 = quad_colors[0]
                .iter()
                .zip(&quad_colors[2])
                .map(|(a, b)| a + b);
            let diagonal_13 = quad_colors[1]
                .iter()
                .zip(&quad_colors[3])
                .map(|(a, b)| a + b);
            assert!(
                diagonal_02
                    .zip(diagonal_13)
                    .all(|(a, b)| (a - b).abs() < 1e-6)
            );
        }
        for tri in m.indices[q * 6..q * 6 + 6].as_chunks::<3>().0 {
            let a = m.positions[tri[0] as usize];
            let b = m.positions[tri[1] as usize];
            let c = m.positions[tri[2] as usize];
            let determinant = (b[u] - a[u]) * (c[v] - a[v]) - (b[v] - a[v]) * (c[u] - a[u]);
            let expected_sign = match axis {
                1 => -face.normal[axis],
                _ => face.normal[axis],
            };
            assert!(determinant * expected_sign as f32 > 0.);
        }
        let indices = m.indices[q * 6..q * 6 + 6].try_into().unwrap();
        face.corners.map(|corner| {
            let point = std::array::from_fn(|a| p[a] as f32 + f32::from(corner[a]));
            interpolate(&m.positions, &m.colors, indices, point, [u, v])
        })
    }

    fn interpolate(
        positions: &[[f32; 3]],
        colors: &[[f32; 4]],
        indices: [u32; 6],
        point: [f32; 3],
        [u, v]: [usize; 2],
    ) -> [f32; 4] {
        let [x, y] = [point[u], point[v]];
        for tri in indices.as_chunks::<3>().0 {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| positions[i as usize]);
            let d = (b[v] - c[v]) * (a[u] - c[u]) + (c[u] - b[u]) * (a[v] - c[v]);
            let w0 = ((b[v] - c[v]) * (x - c[u]) + (c[u] - b[u]) * (y - c[v])) / d;
            let w1 = ((c[v] - a[v]) * (x - c[u]) + (a[u] - c[u]) * (y - c[v])) / d;
            let w2 = 1. - w0 - w1;
            if [w0, w1, w2].iter().all(|&w| w >= -1e-6) {
                return std::array::from_fn(|i| {
                    w0 * colors[tri[0] as usize][i]
                        + w1 * colors[tri[1] as usize][i]
                        + w2 * colors[tri[2] as usize][i]
                });
            }
        }
        panic!("uncovered point {point:?}")
    }

    fn verify_unit_interiors(
        m: &ChunkMesh,
        q: usize,
        p: [usize; 3],
        face: &Face,
        reference_colors: [[f32; 4]; 4],
    ) {
        let axis = face.normal.iter().position(|&n| n != 0).unwrap();
        let [u, v] = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let reference_positions = face
            .corners
            .map(|c| std::array::from_fn(|a| p[a] as f32 + f32::from(c[a])));
        let reference_indices = if reference_colors[0][0] + reference_colors[2][0]
            > reference_colors[1][0] + reference_colors[3][0]
        {
            [0, 1, 3, 1, 2, 3]
        } else {
            [0, 1, 2, 0, 2, 3]
        };
        for [x, y] in [
            [0.25, 0.25],
            [0.75, 0.25],
            [0.25, 0.75],
            [0.75, 0.75],
            [0.5, 0.5],
        ] {
            let mut point = reference_positions[0];
            point[u] = p[u] as f32 + x;
            point[v] = p[v] as f32 + y;
            let expected = interpolate(
                &reference_positions,
                &reference_colors,
                reference_indices,
                point,
                [u, v],
            );
            let actual = interpolate(
                &m.positions,
                &m.colors,
                m.indices[q * 6..q * 6 + 6].try_into().unwrap(),
                point,
                [u, v],
            );
            for (a, b) in actual.into_iter().zip(expected) {
                assert!(
                    (a - b).abs() < 1e-6,
                    "interior field at {point:?}: {actual:?} != {expected:?}"
                );
            }
        }
    }

    #[test]
    fn surfaces_match_unit_visibility_and_lighting_for_adversarial_shapes() {
        for cp in [
            ChunkPos([0, 0, 0]),
            ChunkPos([-1, -1, -1]),
            ChunkPos([2047, 2, -2048]),
        ] {
            for kind in 0..7 {
                let mut w = World::default();
                let mut c = Chunk::default();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let solid = match kind {
                                0 => false,
                                1 => true,
                                2 => y < 2,
                                3 => {
                                    (2..12).contains(&x)
                                        && (3..9).contains(&y)
                                        && (4..15).contains(&z)
                                }
                                4 => x == 0 || x == 15 || y == 0 || y == 15 || z == 0 || z == 15,
                                5 => (x * 17 + y * 23 + z * 31) % 5 < 2,
                                _ => !(y > 2 && y < 10 && (x % 8 < 4 || z % 8 < 4)),
                            };
                            let b = if !solid {
                                Block::Air
                            } else if kind == 5 {
                                [
                                    Block::Grass,
                                    Block::Dirt,
                                    Block::Stone,
                                    Block::Wood,
                                    Block::Leaves,
                                ][(x + y + z) % 5]
                            } else {
                                Block::Stone
                            };
                            c.set(LocalPos::new([x, y, z]).unwrap(), b);
                        }
                    }
                }
                w.insert_chunk(cp, c);
                verify(&w, cp);
                if kind == 1 && cp.0[1] >= 2 {
                    assert_eq!(mesh_chunk(&w, cp).indices.len() / 6, 6);
                }
                // All six neighboring chunk seams, light shadows and subsequent edits.
                for axis in 0..3 {
                    for sign in [-1, 1] {
                        let mut p = cp.0;
                        p[axis] += sign;
                        w.insert_chunk(ChunkPos(p), {
                            let mut c = Chunk::default();
                            for y in 0..16 {
                                for z in 0..16 {
                                    for x in 0..16 {
                                        c.set(LocalPos::new([x, y, z]).unwrap(), Block::Stone);
                                    }
                                }
                            }
                            c
                        });
                    }
                }
                verify(&w, cp);
                let bp = cp
                    .block_position(LocalPos::new([0, 0, 0]).unwrap())
                    .unwrap();
                // Direct chunk replacement also works at the finite world bounds.
                let mut c = w.chunk(cp).unwrap().clone();
                c.set(bp.split().1, Block::Air);
                w.insert_chunk(cp, c);
                verify(&w, cp);
            }
        }
    }
}

#[cfg(test)]
mod ao_tests;

#[cfg(test)]
mod torch_tests {
    use super::*;
    use crate::{Block, BlockPos};
    #[test]
    fn torch_is_small_centered_wound_and_has_a_bright_tip_in_darkness() {
        let m = torch_mesh();
        assert_eq!(m.indices.len(), 12 * 6);
        assert_eq!(m.positions.len(), 48);
        for p in &m.positions {
            assert!((0.40625..=0.59375).contains(&p[0]));
            assert!((0.40625..=0.59375).contains(&p[2]));
            assert!((0.0..=0.6875).contains(&p[1]));
        }
        assert!(m.colors[24..].iter().all(|c| *c == [1.; 4]));
        for t in m.indices.as_chunks::<3>().0 {
            let a = m.positions[t[0] as usize];
            let b = m.positions[t[1] as usize];
            let c = m.positions[t[2] as usize];
            let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            assert!(
                cross
                    .iter()
                    .zip(m.normals[t[0] as usize])
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
                    > 0.
            );
        }
    }
    #[test]
    fn torch_does_not_cull_cube_faces_or_occlude_ao_and_greedy_keeps_light_bands() {
        let mut w = World::default();
        for x in -8..8 {
            for z in -8..8 {
                w.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
            }
        }
        let cp = ChunkPos([0; 3]);
        let before = mesh_chunk(&w, cp);
        w.set_block(BlockPos([0, 1, 0]), Block::Torch).unwrap();
        // In open sky the terrain lighting and AO stay unchanged. Only the
        // twelve-quadded non-cube model is added; its supporting face stays.
        let after = mesh_chunk(&w, cp);
        assert_eq!(after.indices.len(), before.indices.len() + 72);
        for x in -8..8 {
            for z in -8..8 {
                w.set_block(BlockPos([x, 5, z]), Block::Stone).unwrap();
            }
        }
        // Check merged floor colors against their independent combined-light sample.
        let light =
            crate::lighting::Skylight::for_chunks(&w, &std::collections::BTreeSet::from([cp]));
        let m = mesh_chunk_lit(&w, cp, &light);
        let floor: Vec<_> = m
            .positions
            .as_chunks::<4>()
            .0
            .iter()
            .zip(m.colors.as_chunks::<4>().0)
            .enumerate()
            .filter(|(q, (p, _))| m.normals[q * 4] == [0., 1., 0.] && p.iter().all(|v| v[1] == 1.))
            .map(|(_, face)| face)
            .collect();
        assert!(floor.len() > 1, "torch gradient must split the floor");
        for (p, c) in floor {
            let x = p.iter().map(|p| p[0] as i32).min().unwrap();
            let z = p.iter().map(|p| p[2] as i32).min().unwrap();
            let expected =
                crate::lighting::face_color(light.combined_level(BlockPos([x, 1, z])), [0, 1, 0]);
            assert_eq!(c, &[expected; 4]);
        }
    }
}

#[cfg(test)]
mod wall_torch_tests {
    use super::*;
    #[test]
    fn wall_models_use_selection_geometry_and_keep_original_quad_count_and_winding() {
        for block in [
            crate::Block::TorchEast,
            crate::Block::TorchWest,
            crate::Block::TorchSouth,
            crate::Block::TorchNorth,
        ] {
            let mut m = ChunkMesh::default();
            append_torch(&mut m, block, [0.; 3], 14);
            assert_eq!(m.positions.len(), 48);
            assert_eq!(m.indices.len(), 72);
            assert!(m.colors[24..].iter().all(|c| *c == [1.; 4]));
            assert!(
                m.positions
                    .iter()
                    .flatten()
                    .all(|v| (-1e-6..=1.000001).contains(v))
            );
            for (box_index, bounds) in block.selection_boxes().iter().enumerate() {
                let corners = bounds
                    .corners()
                    .map(|p| block.shape_transform().point(p).map(|v| v as f32));
                for p in &m.positions[box_index * 24..box_index * 24 + 24] {
                    assert!(
                        corners
                            .iter()
                            .any(|c| c.iter().zip(p).all(|(a, b)| (a - b).abs() < 1e-6))
                    );
                }
            }
            for t in m.indices.as_chunks::<3>().0 {
                let [a, b, c] = t.map(|i| m.positions[i as usize]);
                let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
                let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
                let cross = [
                    ab[1] * ac[2] - ab[2] * ac[1],
                    ab[2] * ac[0] - ab[0] * ac[2],
                    ab[0] * ac[1] - ab[1] * ac[0],
                ];
                assert!(
                    cross
                        .into_iter()
                        .zip(m.normals[t[0] as usize])
                        .map(|(a, b)| a * b)
                        .sum::<f32>()
                        > 0.
                );
            }
        }
    }
}
