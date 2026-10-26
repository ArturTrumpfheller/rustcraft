//! Independent translucent surface buffers. Solid greedy vertices are unchanged.
use crate::{BlockPos, ChunkPos, LocalPos, World, lighting::Skylight};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
/// Exact shared world-corner sampling, including diagonals across chunk seams.
pub fn corner_height(world: &World, p: BlockPos, cx: i32, cz: i32) -> f32 {
    let mut total = 0.;
    let mut count = 0.;
    for dx in [cx - 1, cx] {
        for dz in [cz - 1, cz] {
            let q = p.offset([dx, 0, dz]).unwrap();
            if let Some(f) = world.block(q).fluid() {
                if world.block(q.offset([0, 1, 0]).unwrap()).fluid().is_some() || f.is_falling() {
                    return 1.;
                }
                let weight = if f.is_source() { 8. } else { 1. };
                total += f.height() * weight;
                count += weight;
            } else if world.block(q) == crate::Block::Air {
                // Dry edges taper gently; shared world corners stay identical.
                count += 1.;
            }
        }
    }
    if count == 0. { 0. } else { total / count }
}
pub fn mesh_water(world: &World, cp: ChunkPos, light: &Skylight) -> WaterMesh {
    mesh_water_sampled(world, cp, |p| [light.level(p), light.block_level(p)])
}
/// Reuse exact applied light for a water/Air-only change; both transmit light equally.
pub fn mesh_water_cached(
    world: &World,
    cp: ChunkPos,
    light: &crate::lighting::ChunkLight,
) -> WaterMesh {
    mesh_water_sampled(world, cp, |p| {
        let [x, y, z] = std::array::from_fn(|a| (p.0[a] - cp.0[a] * 16 + 1) as usize);
        let v = light.0[x + 18 * (z + 18 * y)];
        [v & 15, v >> 4]
    })
}
fn mesh_water_sampled(
    world: &World,
    cp: ChunkPos,
    sample: impl Fn(BlockPos) -> [u8; 2],
) -> WaterMesh {
    let mut out = WaterMesh::default();
    let Some(chunk) = world.chunk(cp).filter(|c| c.water_cells() != 0) else {
        return out;
    };
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let local = LocalPos::new([x, y, z]).unwrap();
                if chunk.get(local).fluid().is_none() {
                    continue;
                }
                let p = cp.block_position(local).unwrap();
                let h = [[0, 0], [1, 0], [0, 1], [1, 1]]
                    .map(|[cx, cz]| corner_height(world, p, cx, cz));
                for face in &crate::mesh::FACES {
                    let neighbor = world.block(p.offset(face.normal).unwrap());
                    if neighbor.fluid().is_some() || neighbor.is_opaque() {
                        continue;
                    }
                    let start = out.positions.len() as u32;
                    let shade = match face.normal {
                        [0, 1, 0] => 1.,
                        [0, -1, 0] => 0.55,
                        [0, 0, _] => 0.8,
                        _ => 0.7,
                    };
                    let sky = 0.06 + 0.94 * (f32::from(sample(p)[0]) / 15.).powf(1.6);
                    let block = if sample(p)[1] == 0 {
                        0.
                    } else {
                        0.06 + 0.94 * (f32::from(sample(p)[1]) / 15.).powf(1.6)
                    };
                    for c in face.corners {
                        let height = if c[1] == 0 {
                            0.
                        } else {
                            h[usize::from(c[0] + 2 * c[2])]
                        };
                        out.positions.push([
                            x as f32 + f32::from(c[0]),
                            y as f32 + height,
                            z as f32 + f32::from(c[2]),
                        ]);
                        out.normals.push(face.normal.map(|v| v as f32));
                        out.uvs.push(if face.normal[1] != 0 {
                            [f32::from(c[0]), f32::from(c[2])]
                        } else {
                            [
                                f32::from(if face.normal[0] != 0 { c[2] } else { c[0] }),
                                1. - height,
                            ]
                        });
                        out.colors.push([
                            sky * shade,
                            block * shade,
                            f32::from(sample(p)[0]) / 15.,
                            0.65,
                        ]);
                    }
                    out.indices
                        .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                }
            }
        }
    }
    out
}
