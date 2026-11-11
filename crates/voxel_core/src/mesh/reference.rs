// SPDX-License-Identifier: MIT OR Apache-2.0
// Frozen merged-Slice-32 mesher; test-only independent traversal oracle.
use super::{ChunkMesh, FACES, Face, append_torch};
use crate::{CHUNK_SIZE, ChunkPos, LocalPos, World};
#[path = "reference_ao.rs"]
mod ao;
pub fn mesh_chunk_lit(
    world: &World,
    position: ChunkPos,
    light: &crate::lighting::Skylight,
) -> ChunkMesh {
    let mut mesh = ChunkMesh {
        water: crate::water_mesh::mesh_water(world, position, light),
        ..Default::default()
    };
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
        sky: u8,
        block: u8,
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
                if block.is_plant() {
                    let bp = origin.offset(p.map(|v| v as i32)).unwrap();
                    mesh.plants.append(block, p.map(|v| v as f32), bp, light);
                }
                if block.is_torch() {
                    let bp = origin.offset(p.map(|v| v as i32)).unwrap();
                    append_torch(
                        &mut mesh,
                        block,
                        p.map(|v| v as f32),
                        [light.level(bp), light.block_level(bp)],
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
                            sky: origin
                                .offset(std::array::from_fn(|a| p[a] as i32 + face.normal[a]))
                                .map_or(0, |p| light.level(p)),
                            block: origin
                                .offset(std::array::from_fn(|a| p[a] as i32 + face.normal[a]))
                                .map_or(0, |p| light.block_level(p)),
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
                                && candidate.sky == key.sky
                                && candidate.block == key.block
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
                    mesh.colors.extend(ao.map(|a| {
                        [
                            colors[f][usize::from(key.sky)][usize::from(a)][0],
                            colors[f][usize::from(key.block)][usize::from(a)][0],
                            f32::from(key.sky) / 15.,
                            1.,
                        ]
                    }));
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
