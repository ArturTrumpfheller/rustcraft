//! Visual-only voxel AO. A fixed one-cell halo includes edge/corner chunks.
use super::Face;
use crate::{ChunkPos, LocalPos, World};

const SIZE: usize = 18;
const STRIDE: [i32; 3] = [1, (SIZE * SIZE) as i32, SIZE as i32];

// Visual-only strength per occlusion step, interpolated by gameplay skylight.
// Keep both here so gallery tuning never changes propagation or user settings.
const SKY_STEP: f32 = 0.14;
const DARK_STEP: f32 = 0.09;

type ColorTable = [[[[f32; 4]; 4]; 16]; 6];
static COLORS: std::sync::LazyLock<ColorTable> = std::sync::LazyLock::new(|| {
    super::FACES.each_ref().map(|face| {
        std::array::from_fn(|sky| {
            let base = crate::lighting::face_color(sky as u8, face.normal);
            std::array::from_fn(|ao| modulate(base, sky as u8, ao as u8))
        })
    })
});

pub(super) fn colors() -> &'static ColorTable {
    &COLORS
}

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
                    let Some(origin) = p.block_position(LocalPos::new([0; 3]).unwrap()) else {
                        continue;
                    };
                    let chunk = world.chunk(p);
                    let pending = chunk.is_none() && !world.ready(origin);
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
                                        c.get(LocalPos::new(local).unwrap()).is_opaque()
                                    });
                            }
                        }
                    }
                }
            }
        }
        out
    }

    pub fn index(p: [usize; 3]) -> i32 {
        ((p[1] + 1) * SIZE * SIZE + (p[2] + 1) * SIZE + p[0] + 1) as i32
    }

    pub fn solid(&self, center: i32, offset: i32) -> bool {
        self.0[(center + offset) as usize]
    }

    pub fn sample_index(&self, base: i32, offsets: [[i32; 3]; 4]) -> [u8; 4] {
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
        self.sample_index(Self::index(p), offsets(face))
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

/// An integer affine AO field in the plane's positive u/v directions.
/// Translating a unit face adds the same amount to all four corner values.
pub(super) struct Affine {
    corners: [u8; 4],
    slope: [i16; 2],
}

impl Affine {
    pub fn new(corners: [u8; 4], [lo, u, v]: [usize; 3]) -> Option<Self> {
        if corners[0] + corners[2] != corners[1] + corners[3] {
            return None;
        }
        Some(Self {
            corners,
            slope: [
                i16::from(corners[u]) - i16::from(corners[lo]),
                i16::from(corners[v]) - i16::from(corners[lo]),
            ],
        })
    }

    pub fn matches(&self, corners: [u8; 4], [u, v]: [usize; 2]) -> bool {
        let shift = self.slope[0] * u as i16 + self.slope[1] * v as i16;
        if shift == 0 {
            corners == self.corners
        } else {
            corners
                .into_iter()
                .zip(self.corners)
                .all(|(a, b)| i16::from(a) == i16::from(b) + shift)
        }
    }

    pub fn extended(
        &self,
        face: &Face,
        [u, v]: [usize; 2],
        [width, height]: [usize; 2],
    ) -> [u8; 4] {
        std::array::from_fn(|k| {
            let value = i16::from(self.corners[k])
                + self.slope[0] * i16::from(face.corners[k][u]) * (width - 1) as i16
                + self.slope[1] * i16::from(face.corners[k][v]) * (height - 1) as i16;
            // Every unit face has already matched this field, including the
            // rectangle's outer corners, so all emitted values remain in 0..=3.
            debug_assert!((0..=3).contains(&value));
            value as u8
        })
    }
}

#[cfg(test)]
pub(super) fn color(sky: u8, normal: [i32; 3], ao: u8) -> [f32; 4] {
    modulate(crate::lighting::face_color(sky, normal), sky, ao)
}

pub(super) fn modulate(mut color: [f32; 4], sky: u8, ao: u8) -> [f32; 4] {
    let step = DARK_STEP + (SKY_STEP - DARK_STEP) * f32::from(sky.min(15)) / 15.;
    let factor = 1. - f32::from(ao) * step;
    for c in &mut color[..3] {
        *c *= factor;
    }
    color
}

pub(super) fn normal_offset(face: &Face) -> i32 {
    (0..3).map(|a| face.normal[a] * STRIDE[a]).sum()
}

// Four two-bit occupancy counts keep each visibility-mask entry compact.
pub(super) fn pack(c: [u8; 4]) -> u8 {
    c[0] | c[1] << 2 | c[2] << 4 | c[3] << 6
}
pub(super) fn unpack(p: u8) -> [u8; 4] {
    [p & 3, (p >> 2) & 3, (p >> 4) & 3, p >> 6]
}
