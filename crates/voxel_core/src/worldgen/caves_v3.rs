// SPDX-License-Identifier: MIT OR Apache-2.0
//! V14 dry cavities. Bounded ownership graph + warped implicit rock density.
//! All geometry uses absolute coordinates and integer classification. No neighbor
//! chunks, mutable RNG, biome labels or hydrological source admission.
use super::{Generator, caves, hash, hydrology, noise};
use crate::{Block, BlockPos, Chunk, ChunkPos, LocalPos};
const CELL: i32 = 128;
const MIN_Y: i32 = -60;
const STEP: i32 = 4;

/// The historical implementation is intentionally kept behind an explicit gate.
pub(super) enum Versioned {
    Historical(caves::Plan),
    V14(Plan),
}
impl Versioned {
    pub(super) fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        if g.config.version == 14 {
            Self::V14(Plan::new(g, min, max))
        } else {
            Self::Historical(caves::Plan::new(g.config.seed, min, max))
        }
    }
    pub(super) fn contains(&self, p: BlockPos, height: i32) -> bool {
        match self {
            Self::Historical(c) => c.contains(p, height),
            Self::V14(c) => c.contains(p, height),
        }
    }
    pub(super) fn carve(&self, c: &mut Chunk, p: ChunkPos, height: impl Fn(i32, i32) -> i32) {
        match self {
            Self::Historical(plan) => plan.carve(c, p, height),
            Self::V14(plan) => plan.carve_if(c, p, height, |_, _| true),
        }
    }
    pub(super) fn carve_if(
        &self,
        c: &mut Chunk,
        p: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        match self {
            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),
            Self::V14(plan) => plan.carve_if(c, p, height, permitted),
        }
    }
}

fn mix(a: i32, b: i32, t: i32, q: i32) -> i32 {
    a + (b - a) * t / q
}
/// Q10 smooth value field. Salted Y participates in the same signed hash as X/Z.
fn noise3(seed: u64, p: [i32; 3], scale: [i32; 3], salt: u64) -> i32 {
    let c: [i32; 3] = std::array::from_fn(|i| p[i].div_euclid(scale[i]));
    let f: [i32; 3] = std::array::from_fn(|i| {
        let t = p[i].rem_euclid(scale[i]) * 1024 / scale[i];
        t * t / 1024 * (3072 - 2 * t) / 1024
    });
    let v: [i32; 8] = std::array::from_fn(|i| {
        (hash(
            seed ^ ((c[1] + ((i >> 1) & 1) as i32) as i64 as u64).wrapping_mul(0x632be59bd9b4e019),
            c[0] + (i & 1) as i32,
            c[2] + ((i >> 2) & 1) as i32,
            salt,
        ) % 2049) as i32
            - 1024
    });
    mix(
        mix(
            mix(v[0], v[1], f[0], 1024),
            mix(v[2], v[3], f[0], 1024),
            f[1],
            1024,
        ),
        mix(
            mix(v[4], v[5], f[0], 1024),
            mix(v[6], v[7], f[0], 1024),
            f[1],
            1024,
        ),
        f[2],
        1024,
    )
}
/// Shared globally aligned 4³ lattice, only spanning intersecting room bounds.
/// Tubes/empty columns require no density lattice at all.
struct Field {
    min: [i32; 3],
    size: [usize; 3],
    values: Vec<[i32; 2]>,
}
impl Field {
    fn new(seed: u64, min: [i32; 3], max: [i32; 3]) -> Self {
        let min = min.map(|v| v.div_euclid(STEP));
        let size = std::array::from_fn(|i| (max[i].div_euclid(STEP) + 2 - min[i]) as usize);
        let mut values = Vec::with_capacity(size.iter().product());
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let p = [
                        (min[0] + x as i32) * STEP,
                        (min[1] + y as i32) * STEP,
                        (min[2] + z as i32) * STEP,
                    ];
                    values.push([
                        noise3(seed, p, [32, 20, 28], 0xe301),
                        noise3(seed, p, [14, 12, 16], 0xe302),
                    ]);
                }
            }
        }
        Self { min, size, values }
    }
    fn sample(&self, p: [i32; 3]) -> [i32; 2] {
        let c: [usize; 3] = std::array::from_fn(|i| (p[i].div_euclid(STEP) - self.min[i]) as usize);
        let f = p.map(|v| v.rem_euclid(STEP));
        std::array::from_fn(|k| {
            let v: [i32; 8] = std::array::from_fn(|i| {
                self.values[c[0]
                    + (i & 1)
                    + (c[1] + ((i >> 1) & 1) + (c[2] + ((i >> 2) & 1)) * self.size[1])
                        * self.size[0]][k]
            });
            mix(
                mix(
                    mix(v[0], v[1], f[0], STEP),
                    mix(v[2], v[3], f[0], STEP),
                    f[1],
                    STEP,
                ),
                mix(
                    mix(v[4], v[5], f[0], STEP),
                    mix(v[6], v[7], f[0], STEP),
                    f[1],
                    STEP,
                ),
                f[2],
                STEP,
            )
        })
    }
}
#[derive(Clone, Debug)]
struct Shape {
    a: [i32; 3], // Q4 voxel centers, absolute
    b: [i32; 3],
    radius: [i32; 3], // Q4
    min: [i32; 3],
    max: [i32; 3],
    room: bool,
    entrance: bool,
    salt: u64,
}
impl Shape {
    fn new(
        a: [i32; 3],
        b: [i32; 3],
        radius: [i32; 3],
        room: bool,
        entrance: bool,
        salt: u64,
    ) -> Self {
        // Room deformation stays inside its expanded envelope; tubes unwarped.
        let margin = if room { 20 * 16 } else { 0 };
        Self {
            a,
            b,
            radius,
            room,
            entrance,
            salt,
            min: std::array::from_fn(|i| (a[i].min(b[i]) - radius[i] - margin).div_euclid(16)),
            max: std::array::from_fn(|i| (a[i].max(b[i]) + radius[i] + margin).div_euclid(16)),
        }
    }
    fn bounds(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| (self.min[i]..=self.max[i]).contains(&p[i]))
    }
    fn contains(&self, p: [i32; 3], field: Option<&Field>) -> bool {
        if !self.bounds(p) {
            return false;
        }
        let mut d: [i64; 3] = std::array::from_fn(|i| i64::from(p[i] * 16 + 8 - self.a[i]));
        if self.room {
            let n = field.expect("rooms have a density lattice").sample(p);
            d[0] += i64::from(n[0] * 64 / 1024);
            d[2] += i64::from(n[1] * 64 / 1024);
            d[1] += i64::from((n[0] + n[1]) * 24 / 1024);
            // Rounded non-ellipsoidal footprint, sculpted by two coherent scales.
            let q: [i64; 3] = std::array::from_fn(|i| d[i] * 1024 / i64::from(self.radius[i]));
            let metric = q[0] * q[0] / 1024 + q[2] * q[2] / 1024 + q[1] * q[1] / 1024;
            if metric > i64::from(900 + n[0] / 2 + n[1] / 3) {
                return false;
            }
            // Residual tapered pillars. Paths are composed afterwards and may
            // pierce them, retaining the graph's connections through rooms.
            if self.radius[0] >= 20 * 16 {
                for i in 0..2 {
                    let h = self.salt.rotate_right(i * 23);
                    let px = ((h % 17) as i64 - 8) * i64::from(self.radius[0]) / 16;
                    let pz = (((h >> 8) % 17) as i64 - 8) * i64::from(self.radius[2]) / 16;
                    let r = 26 + ((h >> 16) % 25) as i64 + d[1].abs() / 12;
                    if (d[0] - px).pow(2) + (d[2] - pz).pow(2) < r * r {
                        return false;
                    }
                }
                // A partial shelf grows from one wall; other sides stay open.
                if (-16..=24).contains(&d[1]) && q[0] > 260 && n[1] > -180 {
                    return false;
                }
            }
            true
        } else {
            let v: [i64; 3] = std::array::from_fn(|i| i64::from(self.b[i] - self.a[i]));
            let len: i64 = v.iter().map(|v| v * v).sum();
            let t = if len == 0 {
                0
            } else {
                ((0..3).map(|i| d[i] * v[i]).sum::<i64>() * 4096 / len).clamp(0, 4096)
            };
            let metric: i64 = (0..3)
                .map(|i| {
                    let q = (d[i] - v[i] * t / 4096) * 1024 / i64::from(self.radius[i]);
                    q * q
                })
                .sum();
            metric <= 1024 * 1024
        }
    }
}
#[derive(Clone, Copy)]
struct Node {
    deep: [i32; 3],
    upper: [i32; 3],
    h: u64,
}
fn node(g: &Generator, cell: [i32; 2]) -> Option<Node> {
    let h = hash(g.config.seed, cell[0], cell[1], 0xe001);
    let x = cell[0] * CELL + 24 + (h % 80) as i32;
    let z = cell[1] * CELL + 24 + ((h >> 8) % 80) as i32;
    let richness = 570 + noise(g.config.seed, x, z, 512, 0xe002) * 480 / 1024;
    if ((h >> 24) % 1000) as i32 >= richness {
        return None;
    }
    // Surface-relative upper systems independently of absolute deep layers.
    let surface = hydrology::surface_for(g, x, z).bed;
    Some(Node {
        deep: [x, -43 + ((h >> 32) % 20) as i32, z],
        upper: [
            x + ((h >> 44) % 33) as i32 - 16,
            (surface - 22 - ((h >> 52) % 18) as i32).clamp(-16, 136),
            z + ((h >> 36) % 33) as i32 - 16,
        ],
        h,
    })
}
pub(super) struct Plan {
    shapes: Vec<Shape>,
    field: Option<Field>,
}
struct Builder {
    min: [i32; 2],
    max: [i32; 2],
    shapes: Vec<Shape>,
}
impl Builder {
    fn add(&mut self, s: Shape) {
        if s.min[0] <= self.max[0]
            && s.max[0] >= self.min[0]
            && s.min[2] <= self.max[1]
            && s.max[2] >= self.min[1]
        {
            self.shapes.push(s);
        }
    }
    fn room(&mut self, p: [i32; 3], h: u64, large: bool) {
        let radius = if large {
            [
                24 + (h % 19) as i32,
                12 + ((h >> 8) % 13) as i32,
                20 + ((h >> 16) % 19) as i32,
            ]
        } else {
            [
                8 + (h % 9) as i32,
                5 + ((h >> 8) % 6) as i32,
                8 + ((h >> 16) % 9) as i32,
            ]
        };
        let a = p.map(|v| v * 16 + 8);
        self.add(Shape::new(a, a, radius.map(|v| v * 16), true, false, h));
    }
    fn route(&mut self, a: [i32; 3], b: [i32; 3], h: u64, narrow: bool, entrance: bool) {
        let a = a.map(|v| v * 16 + 8);
        let b = b.map(|v| v * 16 + 8);
        // Cubic Bezier: shared endpoints guarantee joins, independent lateral
        // controls create continuous bends; short overlapping chords stay smooth.
        let dx = b[0] - a[0];
        let dz = b[2] - a[2];
        let bend = if h & 1 == 0 { 1 } else { -1 };
        let controls = [
            a,
            [
                a[0] + dx / 3 - dz * bend / 3,
                a[1] + (b[1] - a[1]) / 4 + ((h >> 8) % 17) as i32 * 16 - 128,
                a[2] + dz / 3 + dx * bend / 3,
            ],
            [
                a[0] + dx * 2 / 3 + dz * bend / 4,
                a[1] + (b[1] - a[1]) * 3 / 4,
                a[2] + dz * 2 / 3 - dx * bend / 4,
            ],
            b,
        ];
        let mut prev = a;
        for t in 1i32..=24 {
            let u = 24 - t;
            let next = std::array::from_fn(|i| {
                ((i64::from(u * u * u) * i64::from(controls[0][i])
                    + i64::from(3 * u * u * t) * i64::from(controls[1][i])
                    + i64::from(3 * u * t * t) * i64::from(controls[2][i])
                    + i64::from(t * t * t) * i64::from(controls[3][i]))
                    / (24 * 24 * 24)) as i32
            });
            // Slowly varying anisotropic width. Min noodle diameter >2 blocks.
            let wave = (t - 12).abs();
            let r = if narrow {
                23 + ((h >> 16) % 7) as i32 + wave / 4
            } else {
                42 + ((h >> 16) % 24) as i32 + wave / 2
            };
            self.add(Shape::new(
                prev,
                next,
                [
                    r,
                    r + if narrow { 8 } else { 16 },
                    r + ((h >> 22) % 9) as i32,
                ],
                false,
                entrance,
                h,
            ));
            prev = next;
        }
    }
    fn system(&mut self, g: &Generator, n: Node, neighbors: [Option<Node>; 2]) {
        let h = n.h;
        self.room(n.deep, h, (h >> 40).is_multiple_of(4));
        let upper = n.upper[1] - n.deep[1] > 17;
        if upper {
            self.room(n.upper, h.rotate_left(17), (h >> 46).is_multiple_of(7));
            // Offset path gives a traversable alternative to the optional shaft.
            let side = [
                n.deep[0] + 32,
                n.deep[1] + (n.upper[1] - n.deep[1]) / 2,
                n.deep[2] - 27,
            ];
            self.route(n.deep, side, h, false, false);
            self.route(side, n.upper, h.rotate_left(7), false, false);
            if (h >> 54).is_multiple_of(3) {
                self.route(n.deep, n.upper, h.rotate_left(31), true, false);
            }
        }
        for (i, other) in neighbors.into_iter().enumerate() {
            if let Some(other) = other
                && (h >> (i * 9 + 5)) % 100 < 68
            {
                self.route(
                    n.deep,
                    other.deep,
                    h.rotate_left(i as u32 * 13),
                    false,
                    false,
                );
                if upper && other.upper[1] - other.deep[1] > 17 && (h >> 48) % 100 < 45 {
                    self.route(n.upper, other.upper, h.rotate_left(27), true, false);
                }
            }
        }
        // A local returning branch forms a loop with the major connection.
        if (h >> 28).is_multiple_of(3) {
            let end = [n.deep[0] - 28, n.deep[1] + 13, n.deep[2] + 32];
            self.route(n.deep, end, h.rotate_left(11), true, false);
            self.route(
                end,
                if upper {
                    n.upper
                } else {
                    [n.deep[0] + 12, n.deep[1], n.deep[2]]
                },
                h.rotate_left(43),
                true,
                false,
            );
        }
        if !(h >> 57).is_multiple_of(4) {
            return;
        }
        // Choose a dry, genuinely occupied slope surface, never a legacy height.
        // Bounded probes favor slopes without forcing every system to the surface.
        let mut mouth = None;
        for [dx, dz] in [[38, 0], [-38, 0], [0, 38], [0, -38]] {
            let x = n.upper[0] + dx;
            let z = n.upper[2] + dz;
            if x.abs() < 32 && (z - 10).abs() < 32 {
                continue;
            }
            let water = hydrology::surface_for(g, x, z);
            if water.depth() > 0 || water.bed <= hydrology::SEA_LEVEL + 6 {
                continue;
            }
            let profile = super::terrain_v5::profile(g.config.seed, x, z);
            if !profile.solid(water.bed) {
                continue;
            }
            if mouth.is_none_or(|p: [i32; 3]| water.bed < p[1]) {
                mouth = Some([x, water.bed, z]);
            }
        }
        if let Some(m) = mouth {
            let start = if upper { n.upper } else { n.deep };
            let inward = [
                (start[0] + m[0]) / 2,
                (m[1] - 12).min(start[1] + 18),
                (start[2] + m[2]) / 2,
            ];
            self.route(start, inward, h.rotate_left(9), false, false);
            self.route(
                inward,
                [m[0], m[1] + 2, m[2]],
                h.rotate_left(19),
                false,
                true,
            );
        }
    }
}
impl Plan {
    pub(super) fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        let mut builder = Builder {
            min,
            max,
            shapes: Vec::new(),
        };
        // An owner has only +X/+Z edges. Controls and radius fit inside the
        // owner's [-1,+2] cell envelope. Two rings prove complete discovery.
        let lo = min.map(|v| v.div_euclid(CELL) - 2);
        let hi = max.map(|v| v.div_euclid(CELL) + 2);
        let nx = (hi[0] - lo[0] + 2) as usize;
        let nz = (hi[1] - lo[1] + 2) as usize;
        let nodes: Vec<_> = (0..nx * nz)
            .map(|i| node(g, [lo[0] + (i % nx) as i32, lo[1] + (i / nx) as i32]))
            .collect();
        for z in 0..nz - 1 {
            for x in 0..nx - 1 {
                let i = z * nx + x;
                if let Some(n) = nodes[i] {
                    builder.system(g, n, [nodes[i + 1], nodes[i + nx]]);
                }
            }
        }
        let rooms: Vec<_> = builder.shapes.iter().filter(|s| s.room).collect();
        let field = if rooms.is_empty() {
            None
        } else {
            let bottom = rooms.iter().map(|s| s.min[1]).min().unwrap().max(MIN_Y);
            let top = rooms.iter().map(|s| s.max[1]).max().unwrap().min(191);
            Some(Field::new(
                g.config.seed,
                [min[0], bottom, min[1]],
                [max[0], top, max[1]],
            ))
        };
        Self {
            shapes: builder.shapes,
            field,
        }
    }
    fn allowed(p: [i32; 3], h: i32, entrance: bool) -> bool {
        p[1] >= MIN_Y
            && p[1] <= h
            && p[1] < 192
            && !(p[0].abs() <= 12 && (p[2] - 10).abs() <= 12 && p[1] > h - 20)
            && (entrance || p[1] <= h - 8)
    }
    pub(super) fn contains(&self, p: BlockPos, h: i32) -> bool {
        self.shapes
            .iter()
            .any(|s| Self::allowed(p.0, h, s.entrance) && s.contains(p.0, self.field.as_ref()))
    }
    fn carve_if(
        &self,
        c: &mut Chunk,
        cp: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        let o = cp.0.map(|v| v * 16);
        for s in &self.shapes {
            let min: [i32; 3] = std::array::from_fn(|i| s.min[i].max(o[i]));
            let max: [i32; 3] = std::array::from_fn(|i| s.max[i].min(o[i] + 15));
            if (0..3).any(|i| min[i] > max[i]) {
                continue;
            }
            for z in min[2]..=max[2] {
                for x in min[0]..=max[0] {
                    let h = height(x, z);
                    for y in min[1].max(MIN_Y)..=max[1].min(h) {
                        let p = BlockPos([x, y, z]);
                        let l = LocalPos::new([
                            (x - o[0]) as usize,
                            (y - o[1]) as usize,
                            (z - o[2]) as usize,
                        ])
                        .unwrap();
                        if c.get(l) != Block::Air
                            && Self::allowed(p.0, h, s.entrance)
                            && permitted(p, h)
                            && s.contains(p.0, self.field.as_ref())
                        {
                            c.set(l, Block::Air);
                        }
                    }
                }
            }
        }
        c.compact();
    }
}

/// Read-only development landmarks, clipped to the requested ownership window.
/// They describe proposed features, not an assertion of final player clearance.
/// Callers must inspect final terrain/water occupancy before positioning a camera.
pub fn landmarks(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Vec<(&'static str, [i32; 3])> {
    if g.config.version != 14
        || min
            .iter()
            .chain(max.iter())
            .any(|v| !(-32000..32000).contains(v))
        || (0..2).any(|i| max[i] < min[i] || max[i] - min[i] > 512)
    {
        return vec![];
    }
    let plan = Plan::new(g, min, max);
    let mut result = std::collections::BTreeSet::new();
    for s in plan.shapes {
        let p = std::array::from_fn(|i| ((s.a[i] + s.b[i]) / 2).div_euclid(16));
        if !(min[0]..=max[0]).contains(&p[0]) || !(min[1]..=max[1]).contains(&p[2]) {
            continue;
        }
        let kind = if s.room {
            if s.radius[0] >= 20 * 16 {
                "cavern"
            } else {
                "chamber"
            }
        } else if s.entrance {
            "entrance-route"
        } else if (s.a[1] - s.b[1]).abs() > (s.a[0] - s.b[0]).abs().max((s.a[2] - s.b[2]).abs()) {
            "shaft"
        } else if s.radius[0] < 38 {
            "narrow"
        } else {
            "tunnel"
        };
        result.insert((kind, p));
    }
    result.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_signed_noise_and_global_lattice_fixture() {
        for line in include_str!("../../tests/fixtures/caves-v14-fields.csv")
            .lines()
            .skip(1)
        {
            let v: Vec<_> = line.split(',').collect();
            let seed = v[0].parse().unwrap();
            let p: [i32; 3] = std::array::from_fn(|i| v[i + 1].parse().unwrap());
            let field = Field::new(seed, p, p);
            let expected: Vec<i32> = v[4..].iter().map(|v| v.parse().unwrap()).collect();
            assert_eq!(
                [
                    noise3(seed, p, [32, 20, 28], 0xe301),
                    noise3(seed, p, [14, 12, 16], 0xe302),
                    field.sample(p)[0],
                    field.sample(p)[1]
                ],
                expected.as_slice(),
                "{line}"
            );
        }
    }
    #[test]
    fn owner_discovery_envelope_and_density_bounds_are_proven() {
        for seed in [0, 42, 2026, u64::MAX] {
            let g = Generator::new(super::super::GenerationConfig { seed, version: 14 }).unwrap();
            for z in -3..=3 {
                for x in -3..=3 {
                    if let Some(n) = node(&g, [x, z]) {
                        let mut b = Builder {
                            min: [-32768; 2],
                            max: [32767; 2],
                            shapes: vec![],
                        };
                        b.system(&g, n, [node(&g, [x + 1, z]), node(&g, [x, z + 1])]);
                        for s in b.shapes {
                            assert!(
                                s.min[0] >= (x - 1) * CELL && s.max[0] < (x + 3) * CELL,
                                "{s:?}"
                            );
                            assert!(
                                s.min[2] >= (z - 1) * CELL && s.max[2] < (z + 3) * CELL,
                                "{s:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn overlapping_lattices_and_tubes_agree_on_signed_vertical_seams() {
        for seed in [0, 42, 2026, u64::MAX] {
            let a = Field::new(seed, [-130, -60, -70], [20, 100, 80]);
            let b = Field::new(seed, [-17, -33, -17], [16, 81, 16]);
            for x in -17..=16 {
                for y in [-33, -32, -17, -16, -1, 0, 15, 16, 63, 64, 80] {
                    let p = [x, y, -1];
                    assert_eq!(a.sample(p), b.sample(p));
                }
            }
            let mut b = Builder {
                min: [-200; 2],
                max: [200; 2],
                shapes: vec![],
            };
            b.route([-120, -45, 10], [120, 72, -40], seed, false, false);
            for s in &b.shapes {
                assert!(s.contains(s.a.map(|v| v.div_euclid(16)), None));
                assert!(s.contains(s.b.map(|v| v.div_euclid(16)), None));
            }
        }
    }
}
