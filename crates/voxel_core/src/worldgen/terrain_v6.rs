// SPDX-License-Identifier: MIT OR Apache-2.0
//! V17 geography: shifted climate fields, nested C/E/PV shape and density factor.
//! Inspired by executed Horizon's composition, independently authored Q12 curves
//! scaled to RustCraft's bounds. No upstream registry tables/source are copied.
//! Surface density is factor*(offset-y)+noise: dividing by positive factor gives
//! the bounded offset/amplitude representation used by the shared profile cache.
use super::terrain_v4::{self, Parameters, Profile, gradient2, lerp, mul, ramp};
const Q: i32 = 4096;

/// Inputs and shape outputs are useful independently of caves/water/ecology.
#[derive(Clone, Copy, Debug)]
pub struct Router {
    pub terrain: Parameters,
    pub peaks_valleys: i32,
    pub factor: i32,
    pub jaggedness: i32,
}

/// Q12 Hermite curve with value-valued knots (allowing nested geographic curves).
/// Harmonic tangents keep each interval within its endpoints, including extrema.
fn curve(x: i32, points: &[(i32, i32)]) -> i32 {
    let i = points.partition_point(|p| p.0 < x);
    if i == 0 {
        return points[0].1;
    }
    if i == points.len() {
        return points[i - 1].1;
    }
    let slope = |i: usize| {
        (i64::from(points[i + 1].1 - points[i].1) * i64::from(Q)
            / i64::from(points[i + 1].0 - points[i].0)) as i32
    };
    let tangent = |i: usize| {
        if i == 0 {
            return slope(0);
        }
        if i == points.len() - 1 {
            return slope(i - 1);
        }
        let (a, b) = (i64::from(slope(i - 1)), i64::from(slope(i)));
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
    mul(2 * t3 - 3 * t2 + Q, y0)
        + mul(-2 * t3 + 3 * t2, y1)
        + mul(t3 - 2 * t2 + t, mul(tangent(i - 1), x1 - x0))
        + mul(t3 - t2, mul(tangent(i), x1 - x0))
}

pub fn router(seed: u64, x: i32, z: i32) -> Router {
    let px = ((i64::from(x) * 4 + i64::from(z) * 3) * i64::from(Q) / 5) as i32;
    let pz = ((i64::from(z) * 4 - i64::from(x) * 3) * i64::from(Q) / 5) as i32;
    // A shared smooth domain shift curves broad structural lines. It is global,
    // independent of biome/chunk and small relative to the macro wavelengths.
    let sx = px + gradient2(seed, px, pz, 896, 896, 0x4301) * 56;
    let sz = pz + gradient2(seed, px, pz, 896, 896, 0x4302) * 56;
    let n = |scale, salt| gradient2(seed, sx, sz, scale, scale, salt);
    let c = (n(1536, 0x4311) * 4 + n(640, 0x4312) * 2 + n(256, 0x4313)) / 7;
    let e = (n(1024, 0x4321) * 5 + n(384, 0x4322) * 2 + n(160, 0x4323)) / 8;
    // Broad signed weirdness supplies long ridges and passes. Fine surface
    // variation is a separate field and never receives the full mountain budget.
    let w = (n(384, 0x4331) * 4 + n(160, 0x4332) * 2 + n(72, 0x4333)) / 5;
    let pv = (Q - 3 * (w.abs() - 2 * Q / 3).abs()).max(-Q);
    let hills = curve(
        pv,
        &[
            (-Q, 0),
            (-1800, 4 * Q),
            (0, 15 * Q),
            (2200, 27 * Q),
            (Q, 34 * Q),
        ],
    );
    let smooth = curve(
        pv,
        &[
            (-Q, 8 * Q),
            (-2400, 14 * Q),
            (-600, 34 * Q),
            (1600, 78 * Q),
            (Q, 122 * Q),
        ],
    );
    let rugged = curve(
        pv,
        &[
            (-Q, 10 * Q),
            (-2400, 18 * Q),
            (-600, 40 * Q),
            (1600, 92 * Q),
            (Q, 148 * Q),
        ],
    );
    let mountain = lerp(smooth, rugged, ramp(w, -1800, 1800));
    let rolling = mul(n(96, 0x4341), Q + mul(ramp(-e, -400, 1200), 2 * Q));
    let plain = rolling + mul(n(224, 0x4342), 3 * Q);
    // Erosion selects different PV responses, rather than scaling one repeated
    // ridge template. Broad basin floors survive even in the mountain branch.
    let eroded = curve(
        e,
        &[
            (-Q, mountain),
            (-2300, mountain),
            (-1650, lerp(hills, mountain, 2800)),
            (-1100, hills),
            (-500, hills / 3 + plain),
            (200, plain),
            (Q, plain),
        ],
    );
    let datum = curve(
        c,
        &[
            (-Q, -32 * Q),
            (-1800, -22 * Q),
            (-1150, -9 * Q),
            (-650, 5 * Q),
            (0, 12 * Q),
            (1600, 23 * Q),
            (Q, 30 * Q),
        ],
    );
    // Coastal shoulders have a different nested response to erosion/PV than
    // inland ranges. There is no circular mountain-placement mask.
    let coast = hills / 4 + plain;
    let shape = curve(
        c,
        &[
            (-Q, 0),
            (-1200, 0),
            (-650, coast),
            (-200, eroded * 2 / 3),
            (600, eroded),
            (Q, eroded),
        ],
    );
    let inland = ramp(c, -1300, -250);
    let range = mul(ramp(-e, 650, 1800), ramp(pv, -1600, 2400));
    let jaggedness = mul(range, ramp(w, -600, 1800));
    let offset = datum + shape + mul(inland, mul(n(24, 0x4343), mul(jaggedness, 2 * Q)));
    // Positive density factor: lowlands resist 3D displacement, rugged upper
    // slopes permit recesses. factor is distinct from the nested height offset.
    let factor = lerp(14 * Q, 3 * Q, range);
    let amplitude = mul(
        inland,
        mul(
            range,
            (i64::from(36 * Q) * i64::from(Q) / i64::from(factor)) as i32,
        ),
    );
    // Soft summit cap leaves headroom for snow/vegetation without a flat ceiling.
    let excess = (offset - 138 * Q).max(0);
    let offset = offset - excess
        + (i64::from(excess) * i64::from(28 * Q) / i64::from(excess + 28 * Q)) as i32;
    let spawn = ramp(x.abs().max((z - 10).abs()), 12, 76);
    Router {
        terrain: Parameters {
            continentalness: c,
            erosion: e,
            weirdness: w,
            offset: lerp(offset.max(5 * Q), offset, spawn),
            amplitude: mul(amplitude, spawn),
        },
        peaks_valleys: pv,
        factor,
        jaggedness,
    }
}
pub fn parameters(seed: u64, x: i32, z: i32) -> Parameters {
    router(seed, x, z).terrain
}
pub(super) fn profile(seed: u64, x: i32, z: i32) -> Profile {
    Profile::build(parameters(seed, x, z), x, z, |p| {
        terrain_v4::lattice(seed, p)
    })
}
pub fn height(seed: u64, x: i32, z: i32) -> i32 {
    profile(seed, x, z).top
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_integer_router_and_every_surface_density_bit() {
        for line in include_str!("../../tests/fixtures/terrain-v17.csv")
            .lines()
            .skip(1)
        {
            let v: Vec<_> = line.split(',').collect();
            let (seed, x, z) = (
                v[0].parse().unwrap(),
                v[1].parse().unwrap(),
                v[2].parse().unwrap(),
            );
            let r = router(seed, x, z);
            let p = r.terrain;
            let s = profile(seed, x, z);
            let expected: Vec<i32> = v[3..13].iter().map(|s| s.parse().unwrap()).collect();
            assert_eq!(
                [
                    p.continentalness,
                    p.erosion,
                    p.weirdness,
                    p.offset,
                    p.amplitude,
                    r.peaks_valleys,
                    r.factor,
                    r.jaggedness,
                    s.top,
                    s.floor
                ],
                expected.as_slice(),
                "{line}"
            );
            assert_eq!(s.bits_for_test(), v[13].parse::<u64>().unwrap(), "{line}");
        }
    }
    #[test]
    fn shared_surface_cache_matches_points_on_negative_distant_and_limit_halos() {
        for seed in [0, 42, 2026, u64::MAX, 123, 777] {
            for min in [
                [-17, -17],
                [15, 63],
                [-24001, 20001],
                [27999, -30001],
                [-32768, 32748],
                [32748, -32768],
            ] {
                let (tile, _) = terrain_v4::profiles_with(seed, min, 20, parameters);
                for (i, p) in tile.into_iter().enumerate() {
                    let s = profile(seed, min[0] + i as i32 % 20, min[1] + i as i32 / 20);
                    assert_eq!(
                        (p.top, p.floor, p.bits_for_test()),
                        (s.top, s.floor, s.bits_for_test())
                    );
                }
            }
        }
    }
    #[test]
    fn bounded_surface_detail_and_quiet_lowlands_leave_world_headroom() {
        let (mut high, mut quiet, mut recesses) = (0, 0, 0);
        for seed in [0, 42, 2026, u64::MAX] {
            for z in (-4096..4096).step_by(64) {
                for x in (-4096..4096).step_by(64) {
                    let r = router(seed, x, z);
                    let p = r.terrain;
                    let s = profile(seed, x, z);
                    assert!((3 * Q..=14 * Q).contains(&r.factor));
                    assert!((0..=12 * Q).contains(&p.amplitude));
                    assert!((-48..182).contains(&s.top));
                    assert!(s.solid(s.floor - 1) && s.solid(s.top) && !s.solid(s.top + 1));
                    high += usize::from(s.top >= 115);
                    quiet += usize::from(p.amplitude == 0);
                    recesses += usize::from((s.floor..s.top).any(|y| !s.solid(y)));
                    for [dx, dz] in [[1, 0], [0, 1]] {
                        let n = parameters(seed, x + dx, z + dz);
                        if p.erosion >= 0 && n.erosion >= 0 {
                            assert!((p.offset - n.offset).abs() < 2 * Q);
                        }
                    }
                }
            }
        }
        assert!(
            high > 0 && quiet > 0 && recesses > 0,
            "{high} {quiet} {recesses}"
        );
    }
}
