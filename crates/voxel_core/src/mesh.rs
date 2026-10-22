use crate::{BlockPos, CHUNK_SIZE, ChunkPos, LocalPos, World};

/// Engine-neutral mesh buffers. Vertices are local to the chunk origin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
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
    let mut mesh = ChunkMesh::default();
    let Some(chunk) = world.chunk(position) else {
        return mesh;
    };
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = LocalPos::new([x, y, z]).unwrap();
                let block = chunk.get(local);
                if !block.is_solid() {
                    continue;
                }
                let Some(block_position) = position.block_position(local) else {
                    continue;
                };
                for face in &FACES {
                    let n = [
                        x as i32 + face.normal[0],
                        y as i32 + face.normal[1],
                        z as i32 + face.normal[2],
                    ];
                    let solid = if n.iter().all(|v| (0..16).contains(v)) {
                        chunk
                            .get(LocalPos::new(n.map(|v| v as usize)).unwrap())
                            .is_solid()
                    } else {
                        block_position
                            .offset(face.normal)
                            .is_some_and(|neighbor: BlockPos| world.block(neighbor).is_solid())
                    };
                    if solid {
                        continue;
                    }
                    let start = mesh.positions.len() as u32;
                    let tile = crate::texture::face_tile(block, face.normal).unwrap();
                    mesh.uvs
                        .extend_from_slice(crate::texture::face_uvs(tile, face.normal[1] != 0));
                    for corner in face.corners {
                        mesh.positions.push([
                            x as f32 + f32::from(corner[0]),
                            y as f32 + f32::from(corner[1]),
                            z as f32 + f32::from(corner[2]),
                        ]);
                        mesh.normals.push(face.normal.map(|value| value as f32));
                    }
                    mesh.indices
                        .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                }
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, Chunk};

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
            .map(|(position, _)| mesh_chunk(world, position).indices.len() / 6)
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
        Block, Chunk,
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
                let tile = face_tile(block, face.normal).unwrap();
                let uv = if face.normal[1] == 0 {
                    wall
                } else {
                    horizontal
                };
                for (j, &expected_uv) in uv.iter().enumerate() {
                    assert_eq!(m.positions[4 * i + j], face.corners[j].map(f32::from));
                    assert_eq!(m.normals[4 * i + j], face.normal.map(|v| v as f32));
                    assert_eq!(m.uvs[4 * i + j], tile.uv(expected_uv));
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
                        let min = tile.uv([0, 0]);
                        let max = tile.uv([1, 1]);
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
