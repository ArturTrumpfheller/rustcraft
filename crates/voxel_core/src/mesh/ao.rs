//! Visual-only voxel AO. A fixed one-cell halo includes edge/corner chunks.
use super::Face;
use crate::{ChunkPos, LocalPos, World};

const SIZE: usize = 18;
const STRIDE: [i32; 3] = [1, (SIZE * SIZE) as i32, SIZE as i32];

pub(super) struct Occlusion([bool; SIZE * SIZE * SIZE]);

impl Occlusion {
    pub fn new(world: &World, cp: ChunkPos) -> Self {
        let mut out = Self([false; SIZE * SIZE * SIZE]);
        // Query each chunk once, then copy only its part of the halo. This
        // avoids a world-map lookup for every vertex's three occupancy samples.
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let delta = [dx, dy, dz];
                    let [Some(cx), Some(cy), Some(cz)] =
                        std::array::from_fn(|a| cp.0[a].checked_add(delta[a]))
                    else {
                        continue;
                    };
                    let p = ChunkPos([cx, cy, cz]);
                    let chunk = world.chunk(p);
                    let pending = chunk.is_none()
                        && p.block_position(LocalPos::new([0; 3]).unwrap())
                            .is_some_and(|bp| !world.ready(bp));
                    let range = |d| match d {
                        -1 => 0..1,
                        0 => 1..17,
                        _ => 17..18,
                    };
                    for y in range(dy) {
                        for z in range(dz) {
                            for x in range(dx) {
                                let local = [x, y, z].map(|v: usize| (v + 15) % 16);
                                out.0[y * SIZE * SIZE + z * SIZE + x] = pending
                                    || chunk.is_some_and(|c| {
                                        c.get(LocalPos::new(local).unwrap()).is_solid()
                                    });
                            }
                        }
                    }
                }
            }
        }
        out
    }

    pub fn sample(&self, p: [usize; 3], offsets: [[i32; 3]; 4]) -> [u8; 4] {
        let base = ((p[1] + 1) * SIZE * SIZE + (p[2] + 1) * SIZE + p[0] + 1) as i32;
        offsets.map(|[a, b, c]| {
            level(
                self.0[(base + a) as usize],
                self.0[(base + b) as usize],
                self.0[(base + c) as usize],
            )
        })
    }

    #[cfg(test)]
    pub fn corners(&self, p: [usize; 3], face: &Face) -> [u8; 4] {
        self.sample(p, offsets(face))
    }
}

pub(super) fn offsets(face: &Face) -> [[i32; 3]; 4] {
    let axis = face.normal.iter().position(|&n| n != 0).unwrap();
    let [u, v] = match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    let normal = face.normal[axis] * STRIDE[axis];
    face.corners.map(|c| {
        let a = if c[u] == 0 { -STRIDE[u] } else { STRIDE[u] };
        let b = if c[v] == 0 { -STRIDE[v] } else { STRIDE[v] };
        [normal + a, normal + b, normal + a + b]
    })
}

/// Occlusion count: zero is bright, three is maximum corner occlusion.
pub(super) fn level(side_a: bool, side_b: bool, diagonal: bool) -> u8 {
    if side_a && side_b {
        3
    } else {
        u8::from(side_a) + u8::from(side_b) + u8::from(diagonal)
    }
}

#[cfg(test)]
pub(super) fn color(sky: u8, normal: [i32; 3], ao: u8) -> [f32; 4] {
    modulate(crate::lighting::face_color(sky, normal), sky, ao)
}

pub(super) fn modulate(mut color: [f32; 4], sky: u8, ao: u8) -> [f32; 4] {
    // At sky 15: 1.00, .92, .84, .76. Fade to 35% of that strength in
    // darkness to preserve the existing cave floor and texture legibility.
    let factor = 1. - f32::from(ao) * 0.08 * (0.35 + 0.65 * f32::from(sky.min(15)) / 15.);
    for c in &mut color[..3] {
        *c *= factor;
    }
    color
}
