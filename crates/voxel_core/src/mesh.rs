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
/// Missing chunks and coordinates beyond i32 space are treated as air.
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
    // Fixed visibility planes, filled in one storage-order pass. Empty slices
    // are skipped. Extend MergeKey for future AO/block-light data.
    #[derive(Clone, Copy, PartialEq)]
    struct MergeKey {
        tile: crate::texture::Tile,
        sky: u8,
    }
    let mut active_slices = [0u16; 6];
    let mut planes = [[[None; CHUNK_SIZE * CHUNK_SIZE]; CHUNK_SIZE]; 6];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = [x, y, z];
                let local = LocalPos::new(p).unwrap();
                let block = chunk.get(local);
                if !block.is_solid() {
                    continue;
                }
                let Some(bp) = position.block_position(local) else {
                    continue;
                };
                for (f, face) in FACES.iter().enumerate() {
                    let axis = f / 2;
                    let [u, v] = match axis {
                        0 => [1, 2],
                        1 => [0, 2],
                        _ => [0, 1],
                    };
                    let mut n = p.map(|v| v as i32);
                    n[axis] += face.normal[axis];
                    let solid = if (0..CHUNK_SIZE as i32).contains(&n[axis]) {
                        chunk
                            .get(LocalPos::new(n.map(|v| v as usize)).unwrap())
                            .is_solid()
                    } else {
                        bp.offset(face.normal)
                            .is_some_and(|n| world.block(n).is_solid())
                    };
                    if !solid {
                        active_slices[f] |= 1 << p[axis];
                        planes[f][p[axis]][p[v] * CHUNK_SIZE + p[u]] = Some(MergeKey {
                            tile: crate::texture::face_tile(block, face.normal).unwrap(),
                            sky: bp.offset(face.normal).map_or(0, |p| light.level(p)),
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
                    let mut width = 1;
                    while i + width < CHUNK_SIZE && mask[j * CHUNK_SIZE + i + width] == Some(key) {
                        width += 1;
                    }
                    let mut height = 1;
                    while j + height < CHUNK_SIZE
                        && (0..width).all(|x| mask[(j + height) * CHUNK_SIZE + i + x] == Some(key))
                    {
                        height += 1;
                    }
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
                        .extend([crate::lighting::face_color(key.sky, face.normal); 4]);
                    mesh.indices
                        .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    i += width;
                }
            }
        }
    }
    mesh
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
    fn verify(world: &World, cp: ChunkPos) {
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
                                    crate::lighting::face_color(
                                        bp.offset(face.normal).map_or(0, |p| light.level(p)),
                                        face.normal,
                                    ),
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
                        assert!(
                            actual
                                .insert(([x, y, z], f), (m.atlas_uvs[q * 4], m.colors[q * 4]))
                                .is_none(),
                            "overlap"
                        );
                    }
                }
            }
            for k in 0..4 {
                assert_eq!(m.colors[q * 4 + k], m.colors[q * 4]);
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
        assert_eq!(actual, expected, "coverage/tile/light at {cp:?}");
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
