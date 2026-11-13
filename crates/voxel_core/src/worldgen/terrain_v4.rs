// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generator 11: signed gradient fields, nested terrain splines and a sparse
//! surface density band. Independent implementation; no Minecraft source/data.
//! All decisions use Q12 integers and absolute coordinates, including the 4³
//! interpolation lattice. Historical terrain modules are deliberately separate.
use super::{biome, hash};
use crate::Block;
const Q: i32 = 4096;
pub(super) fn mul(a: i32, b: i32) -> i32 {
    (i64::from(a) * i64::from(b) / i64::from(Q)) as i32
}
pub(super) fn lerp(a: i32, b: i32, t: i32) -> i32 {
    a + mul(b - a, t)
}
fn fade(t: i32) -> i32 {
    // Quintic interpolation, with zero first/second derivatives at lattice edges.
    mul(mul(mul(t, t), t), mul(t, 6 * t - 15 * Q) + 10 * Q)
}
pub(super) fn ramp(x: i32, lo: i32, hi: i32) -> i32 {
    let t = ((x - lo) * Q / (hi - lo)).clamp(0, Q);
    mul(mul(t, t), 3 * Q - 2 * t)
}
/// Shape-preserving cubic Hermite curve. Shared harmonic tangents preserve
/// monotonicity without flattening every knot into a contour shelf.
pub(super) fn spline(x: i32, points: &[(i32, i32)]) -> i32 {
    let i = points.partition_point(|p| p.0 < x);
    if i == 0 {
        return points[0].1 * Q;
    }
    if i == points.len() {
        return points[i - 1].1 * Q;
    }
    let slope =
        |i: usize| (points[i + 1].1 - points[i].1) * Q * Q / (points[i + 1].0 - points[i].0);
    let tangent = |i: usize| {
        if i == 0 {
            return slope(0);
        }
        if i == points.len() - 1 {
            return slope(i - 1);
        }
        let a = i64::from(slope(i - 1));
        let b = i64::from(slope(i));
        if a * b <= 0 {
            0
        } else {
            (2 * a * b / (a + b)) as i32
        }
    };
    let (x0, y0) = points[i - 1];
    let (x1, y1) = points[i];
    let t = (x - x0) * Q / (x1 - x0);
    let t2 = mul(t, t);
    let t3 = mul(t2, t);
    (2 * t3 - 3 * t2 + Q) * y0
        + (-2 * t3 + 3 * t2) * y1
        + mul(t3 - 2 * t2 + t, mul(tangent(i - 1), x1 - x0))
        + mul(t3 - t2, mul(tangent(i), x1 - x0))
}
pub(super) fn gradient2(seed: u64, x: i32, z: i32, sx: i32, sz: i32, salt: u64) -> i32 {
    // Seeded phases avoid making every field vanish at the world origin.
    let phase = hash(seed, 0, 0, salt ^ 0xb000);
    let px = x + (phase % (sx as u64 * Q as u64)) as i32;
    let pz = z + ((phase >> 32) % (sz as u64 * Q as u64)) as i32;
    let cx = px.div_euclid(sx * Q);
    let cz = pz.div_euclid(sz * Q);
    let fx = px.rem_euclid(sx * Q) / sx;
    let fz = pz.rem_euclid(sz * Q) / sz;
    let dot = |dx: i32, dz: i32| {
        let a = fx - dx * Q;
        let b = fz - dz * Q;
        match hash(seed, cx + dx, cz + dz, salt) & 7 {
            0 => a,
            1 => -a,
            2 => b,
            3 => -b,
            4 => (a + b) * 2896 / Q,
            5 => (a - b) * 2896 / Q,
            6 => (-a + b) * 2896 / Q,
            _ => (-a - b) * 2896 / Q,
        }
    };
    lerp(
        lerp(dot(0, 0), dot(1, 0), fade(fx)),
        lerp(dot(0, 1), dot(1, 1), fade(fx)),
        fade(fz),
    ) * 2
}
fn gradient3(seed: u64, p: [i32; 3], scale: [i32; 3], salt: u64) -> i32 {
    let phase = hash(seed, 0, 0, salt ^ 0xb100);
    let p: [i32; 3] = std::array::from_fn(|i| {
        p[i] * Q + ((phase >> (i * 17)) % (scale[i] as u64 * Q as u64)) as i32
    });
    let c: [i32; 3] = std::array::from_fn(|i| p[i].div_euclid(scale[i] * Q));
    let f: [i32; 3] = std::array::from_fn(|i| p[i].rem_euclid(scale[i] * Q) / scale[i]);
    let mut v = [0; 8];
    for (i, value) in v.iter_mut().enumerate() {
        let d: [i32; 3] = std::array::from_fn(|k| ((i >> k) & 1) as i32);
        let a: [i32; 3] = std::array::from_fn(|k| f[k] - d[k] * Q);
        let h = hash(
            seed ^ ((c[1] + d[1]) as i64 as u64).wrapping_mul(0x632be59bd9b4e019),
            c[0] + d[0],
            c[2] + d[2],
            salt,
        );
        let axis = (h % 3) as usize;
        *value = ((if h & 4 == 0 { a[axis] } else { -a[axis] })
            + (if h & 8 == 0 {
                a[(axis + 1) % 3]
            } else {
                -a[(axis + 1) % 3]
            }))
            * 2896
            / Q;
    }
    let f = f.map(fade);
    let a = lerp(lerp(v[0], v[1], f[0]), lerp(v[2], v[3], f[0]), f[1]);
    let b = lerp(lerp(v[4], v[5], f[0]), lerp(v[6], v[7], f[0]), f[1]);
    lerp(a, b, f[2]) * 2
}
#[derive(Clone, Copy, Debug)]
pub struct Parameters {
    pub continentalness: i32,
    pub erosion: i32,
    pub weirdness: i32,
    /// Q12 height offset and maximum density displacement, in blocks.
    pub offset: i32,
    pub amplitude: i32,
}
pub fn parameters(seed: u64, x: i32, z: i32) -> Parameters {
    // Rotation is linear, not domain warping. Nonmatching octave wavelengths
    // and signed fields avoid absolute-value zero contours and radial massifs.
    let px = (i64::from(x) * 4 + i64::from(z) * 3) * i64::from(Q) / 5;
    let pz = (i64::from(z) * 4 - i64::from(x) * 3) * i64::from(Q) / 5;
    let (px, pz) = (px as i32, pz as i32);
    let n = |sx, sz, salt| gradient2(seed, px, pz, sx, sz, salt);
    let c = (n(1536, 1536, 0xb001) * 4 + n(640, 640, 0xb002) * 2 + n(256, 256, 0xb003)) / 7;
    let e = (n(512, 512, 0xb004) * 4 + n(224, 224, 0xb005) * 2 + n(96, 96, 0xb006)) / 7;
    let range_a = (n(112, 448, 0xb007) * 3 + n(48, 144, 0xb008) * 2 + n(24, 56, 0xb009)) / 6;
    let range_b = (n(448, 112, 0xb017) * 3 + n(144, 48, 0xb018) * 2 + n(56, 24, 0xb019)) / 6;
    // Slowly changing structural grain avoids a preferred world-wide ridge axis.
    // Blend scalar fields continuously; no rotated region tiles or warp seams.
    let grain = ramp(n(896, 896, 0xb00f), -900, 900);
    let w = lerp(range_a, range_b, grain);
    let inland = ramp(c, -1300, -250);
    let datum = spline(
        c,
        &[
            (-4096, -32),
            (-1800, -22),
            (-1150, -9),
            (-650, 5),
            (0, 14),
            (1600, 23),
            (4096, 30),
        ],
    );
    let relief = spline(
        e,
        &[
            (-4096, 138),
            (-2000, 128),
            (-900, 105),
            (-100, 54),
            (650, 12),
            (1600, 3),
            (4096, 2),
        ],
    );
    let peaks = spline(
        w,
        &[
            (-4096, 0),
            (-2300, 1),
            (-1300, 4),
            (-450, 17),
            (450, 40),
            (1300, 75),
            (2300, 120),
            (4096, 180),
        ],
    ) / 100;
    let jagged = mul(ramp(-e, 200, 2000), ramp(w, 200, 1600));
    let rolling_scale = Q / 3 + mul(Q - ramp(e, -300, 1200), 3 * Q);
    let rolling = mul(n(72, 72, 0xb00a), rolling_scale);
    let detail = mul(n(18, 18, 0xb00b), mul(jagged, 5 * Q));
    let offset = datum + mul(inland, mul(relief, peaks) + rolling + detail);
    // Asymptotic summit compression leaves at least eighteen blocks of sky even at
    // the theoretical field extremes; no hard clipping plane in generated rock.
    let excess = (offset - 130 * Q).max(0);
    let offset = offset - excess
        + (i64::from(excess) * i64::from(32 * Q) / i64::from(excess + 32 * Q)) as i32;
    // Density remains zero-cost in the coasts/quiet lowlands. A smooth bound
    // creates occasional broken slopes only in hills and mountain systems.
    let amplitude = mul(inland, mul(ramp(relief / Q, 16, 100), 12 * Q));
    let distance = x.abs().max((z - 10).abs());
    let spawn = ramp(distance, 12, 76);
    Parameters {
        continentalness: c,
        erosion: e,
        weirdness: w,
        offset: lerp(offset.max(5 * Q), offset, spawn),
        amplitude: mul(amplitude, spawn),
    }
}
/// Surface occupancy needs at most 26 bits per horizontal cell. Below `floor`
/// is solid; above `top` is Air. Caves/water/materials compose afterward.
#[derive(Clone, Copy, Debug)]
pub(super) struct Profile {
    pub top: i32,
    pub(super) floor: i32,
    bits: u64,
}
impl Profile {
    #[cfg(test)]
    pub(super) fn bits_for_test(self) -> u64 {
        self.bits
    }
    pub(super) fn solid(self, y: i32) -> bool {
        y < self.floor || (y <= self.top && self.bits & (1u64 << (y - self.floor)) != 0)
    }
    pub(super) fn build(
        p: Parameters,
        x: i32,
        z: i32,
        mut sample: impl FnMut([i32; 3]) -> i32,
    ) -> Self {
        if p.amplitude == 0 {
            return Self {
                top: p.offset.div_euclid(Q),
                floor: p.offset.div_euclid(Q) + 1,
                bits: 0,
            };
        }
        let a = (p.amplitude + Q - 1) / Q;
        let floor = p.offset.div_euclid(Q) - a;
        let end = p.offset.div_euclid(Q) + a + 1;
        let mut bits = 0u64;
        let cx = x.div_euclid(4) * 4;
        let cz = z.div_euclid(4) * 4;
        let fx = x.rem_euclid(4) * Q / 4;
        let fz = z.rem_euclid(4) * Q / 4;
        let mut previous_y = i32::MIN;
        let mut lo = 0;
        let mut hi = 0;
        for y in floor..=end {
            let cy = y.div_euclid(4) * 4;
            if cy != previous_y {
                let mut plane = |py| {
                    lerp(
                        lerp(sample([cx, py, cz]), sample([cx + 4, py, cz]), fx),
                        lerp(sample([cx, py, cz + 4]), sample([cx + 4, py, cz + 4]), fx),
                        fz,
                    )
                };
                lo = plane(cy);
                hi = plane(cy + 4);
                previous_y = cy;
            }
            let n = lerp(lo, hi, y.rem_euclid(4) * Q / 4).clamp(-Q, Q);
            if p.offset - y * Q + mul(p.amplitude, n) >= 0 {
                bits |= 1u64 << (y - floor);
            }
        }
        let top = if bits == 0 {
            floor - 1
        } else {
            floor + 63 - bits.leading_zeros() as i32
        };
        Self { top, floor, bits }
    }
}
pub(super) fn lattice(seed: u64, p: [i32; 3]) -> i32 {
    (gradient3(seed, p, [32, 28, 32], 0xb020) * 2 + gradient3(seed, p, [12, 8, 12], 0xb021)) / 3
}
pub fn height(seed: u64, x: i32, z: i32) -> i32 {
    profile(seed, x, z).top
}
pub(super) fn profile(seed: u64, x: i32, z: i32) -> Profile {
    Profile::build(parameters(seed, x, z), x, z, |p| lattice(seed, p))
}
/// One column's bounded shared noise vertices; discarded after planning. Never
/// a global/world-sized density map or a full-height per-voxel noise loop.
pub(super) fn profiles(seed: u64, min: [i32; 2], size: i32) -> Vec<Profile> {
    profiles_with_fields(seed, min, size).0
}
pub(super) fn profiles_with_fields(
    seed: u64,
    min: [i32; 2],
    size: i32,
) -> (Vec<Profile>, Vec<Parameters>) {
    profiles_with(seed, min, size, parameters)
}
pub(super) fn profiles_with(
    seed: u64,
    min: [i32; 2],
    size: i32,
    parameters: impl Fn(u64, i32, i32) -> Parameters,
) -> (Vec<Profile>, Vec<Parameters>) {
    let fields: Vec<_> = (0..size * size)
        .map(|i| parameters(seed, min[0] + i % size, min[1] + i / size))
        .collect();
    let bound = |p: &Parameters| {
        let a = (p.amplitude + Q - 1) / Q;
        (p.offset.div_euclid(Q) - a, p.offset.div_euclid(Q) + a + 1)
    };
    let low = fields
        .iter()
        .map(|p| bound(p).0)
        .min()
        .unwrap()
        .div_euclid(4)
        * 4;
    let high = fields
        .iter()
        .map(|p| bound(p).1)
        .max()
        .unwrap()
        .div_euclid(4)
        * 4
        + 4;
    let origin = [min[0].div_euclid(4) * 4, low, min[1].div_euclid(4) * 4];
    let nx = (min[0] + size - 1 - origin[0]) / 4 + 2;
    let nz = (min[1] + size - 1 - origin[2]) / 4 + 2;
    let ny = (high - low) / 4 + 1;
    // Dense bounded memo: <12 KiB even at the theoretical full altitude span.
    // A sentinel avoids Option padding and tree allocations per lattice vertex.
    let mut cache = vec![i32::MIN; (nx * ny * nz) as usize];
    let profiles = fields
        .iter()
        .copied()
        .enumerate()
        .map(|(i, p)| {
            Profile::build(p, min[0] + i as i32 % size, min[1] + i as i32 / size, |v| {
                let [x, y, z] = std::array::from_fn(|k| (v[k] - origin[k]) / 4);
                let i = ((y * nz + z) * nx + x) as usize;
                if cache[i] == i32::MIN {
                    cache[i] = lattice(seed, v);
                }
                cache[i]
            })
        })
        .collect();
    (profiles, fields)
}

pub(super) fn material(seed: u64, x: i32, z: i32, h: i32, slope: i32) -> biome::Column {
    let climate =
        biome::sample_climate(seed, x.clamp(-32768, 32767), z.clamp(-32768, 32767)).unwrap();
    resurface(seed, x, z, h, slope, climate)
}
pub(super) fn resurface(
    seed: u64,
    x: i32,
    z: i32,
    h: i32,
    slope: i32,
    climate: biome::Climate,
) -> biome::Column {
    let edge = super::noise(seed, x, z, 48, 0xb030) * 55 / 1024;
    let cold = climate.temperature - (h - 32).max(0) * 7;
    let top = if slope >= 3 {
        Block::Stone
    } else if cold < -300 + edge {
        Block::SnowyGround
    } else if h > 100 || (h > 52 && slope >= 2) {
        Block::Stone
    } else if biome::classify(climate) == biome::Biome::Desert {
        Block::Sand
    } else {
        Block::Grass
    };
    biome::Column {
        height: h,
        climate,
        top,
        soil: match top {
            Block::Sand => Block::Sand,
            Block::Stone => Block::Stone,
            _ => Block::Dirt,
        },
    }
}
pub(super) fn columns(seed: u64, min: [i32; 2]) -> (Vec<Profile>, Vec<biome::Column>, Vec<i32>) {
    let grid = profiles(seed, min.map(|v| v - 1), 20);
    let mut profiles = Vec::with_capacity(324);
    let mut materials = Vec::with_capacity(324);
    for z in 1..19 {
        for x in 1..19 {
            let j = z * 20 + x;
            let h = grid[j].top;
            let slope = [j - 1, j + 1, j - 20, j + 20]
                .into_iter()
                .map(|i| (grid[i].top - h).abs())
                .max()
                .unwrap();
            profiles.push(grid[j]);
            materials.push(material(
                seed,
                min[0] + x as i32 - 1,
                min[1] + z as i32 - 1,
                h,
                slope,
            ));
        }
    }
    (
        profiles,
        materials,
        grid.into_iter().map(|p| p.top).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_fixed_point_fields_and_density_profiles() {
        for line in include_str!("../../tests/fixtures/terrain-v11.csv")
            .lines()
            .skip(1)
        {
            let v: Vec<_> = line.split(',').collect();
            let seed = v[0].parse().unwrap();
            let x = v[1].parse().unwrap();
            let z = v[2].parse().unwrap();
            let p = parameters(seed, x, z);
            let expected: Vec<i32> = v[3..10].iter().map(|s| s.parse().unwrap()).collect();
            let profile = profile(seed, x, z);
            assert_eq!(
                [
                    p.continentalness,
                    p.erosion,
                    p.weirdness,
                    p.offset,
                    p.amplitude,
                    profile.top,
                    profile.floor
                ],
                expected.as_slice(),
                "{line}"
            );
            assert_eq!(profile.bits, v[10].parse::<u64>().unwrap(), "{line}");
        }
    }
    #[test]
    fn shared_absolute_lattice_matches_point_profiles_at_signed_and_distant_seams() {
        for seed in [0, 42, 2026, u64::MAX] {
            for min in [
                [-17, -17],
                [15, 63],
                [-24001, 20001],
                [27999, -30001],
                [-520, -3208],
            ] {
                for (i, p) in profiles(seed, min, 20).into_iter().enumerate() {
                    let x = min[0] + (i % 20) as i32;
                    let z = min[1] + (i / 20) as i32;
                    let expected = profile(seed, x, z);
                    assert_eq!(
                        (p.top, p.floor, p.bits),
                        (expected.top, expected.floor, expected.bits)
                    );
                    for y in p.floor - 1..=p.top + 1 {
                        assert_eq!(p.solid(y), expected.solid(y));
                    }
                }
            }
        }
    }
    #[test]
    fn spline_tangents_are_continuous_monotone_and_do_not_flatten_each_knot() {
        let knots = [
            (-4096, 0),
            (-1300, 4),
            (-450, 17),
            (450, 40),
            (1300, 75),
            (2300, 120),
            (4096, 180),
        ];
        let mut previous = 0;
        for x in -4096..=4096 {
            let h = spline(x, &knots);
            assert!(h >= previous - 2);
            previous = h;
        }
        for (x, _) in &knots[1..knots.len() - 1] {
            let left = spline(*x, &knots) - spline(*x - 4, &knots);
            let right = spline(*x + 4, &knots) - spline(*x, &knots);
            assert!(left > 0 && right > 0 && (left - right).abs() < 128);
        }
    }
    #[test]
    fn bounded_density_has_real_broken_slopes_and_foundation_headroom() {
        let mut breaks = 0;
        for seed in [0, 42, 2026, u64::MAX] {
            for z in (-4096..4096).step_by(32) {
                for x in (-4096..4096).step_by(32) {
                    let p = profile(seed, x, z);
                    assert!(p.top < 174 && p.floor >= -54, "{seed} {x} {z} {p:?}");
                    assert!(p.solid(p.floor - 1) && p.solid(p.top) && !p.solid(p.top + 1));
                    breaks += usize::from((p.floor..p.top).any(|y| !p.solid(y)));
                }
            }
        }
        assert!(breaks > 10, "no genuine 3D surface recesses: {breaks}");
    }
}
