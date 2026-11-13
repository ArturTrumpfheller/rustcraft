// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generator13: selective regional relief, broad foothills and raised valley floors.
//! Reuses v11's signed fields and bounded density evaluator. Original Q12 curves;
//! no reference-game source/data. V1–v12 dispatch stays on historical parameters.
use super::terrain_v4::{self, Parameters, Profile, gradient2, lerp, mul, ramp, spline};
const Q: i32 = 4096;

/// Versioned geographical input shared by terrain, hydrology and ecology.
pub fn parameters_for(version: u32, seed: u64, x: i32, z: i32) -> Parameters {
    if version == 13 {
        parameters(seed, x, z)
    } else {
        terrain_v4::parameters(seed, x, z)
    }
}
pub fn height_for(version: u32, seed: u64, x: i32, z: i32) -> i32 {
    if version == 13 {
        height(seed, x, z)
    } else {
        terrain_v4::height(seed, x, z)
    }
}
pub fn parameters(seed: u64, x: i32, z: i32) -> Parameters {
    let px = ((i64::from(x) * 4 + i64::from(z) * 3) * i64::from(Q) / 5) as i32;
    let pz = ((i64::from(z) * 4 - i64::from(x) * 3) * i64::from(Q) / 5) as i32;
    let n = |sx, sz, salt| gradient2(seed, px, pz, sx, sz, salt);
    let c = (n(1536, 1536, 0xb001) * 4 + n(640, 640, 0xb002) * 2 + n(256, 256, 0xb003)) / 7;
    // Larger erosion regions and a weaker short octave widen quiet interiors and
    // foothills. The signed ridge wavelengths/grain remain those accepted in v11.
    let e = (n(768, 768, 0xb004) * 5 + n(320, 320, 0xb005) * 2 + n(128, 128, 0xb006)) / 8;
    let a = (n(112, 448, 0xb007) * 3 + n(48, 144, 0xb008) * 2 + n(24, 56, 0xb009)) / 6;
    let b = (n(448, 112, 0xb017) * 3 + n(144, 48, 0xb018) * 2 + n(56, 24, 0xb019)) / 6;
    let w = lerp(a, b, ramp(n(896, 896, 0xb00f), -900, 900));
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
    // Full-size ranges survive in the low-erosion tail, with continuous shoulder
    // and hill bands. Ordinary median erosion no longer receives a 50-block budget.
    let relief = spline(
        e,
        &[
            (-4096, 138),
            (-2400, 128),
            (-1600, 108),
            (-1000, 58),
            (-400, 18),
            (200, 7),
            (900, 3),
            (4096, 2),
        ],
    );
    // Lift negative-weirdness basins within ranges; preserve positive summit
    // silhouettes and saddles. This reduces peak-to-valley contrast selectively.
    let peaks = spline(
        w,
        &[
            (-4096, 12),
            (-2300, 14),
            (-1300, 20),
            (-450, 29),
            (450, 49),
            (1300, 82),
            (2300, 124),
            (4096, 180),
        ],
    ) / 100;
    let rugged = ramp(-e, 600, 1900);
    let jagged = mul(rugged, ramp(w, 200, 1600));
    let rolling = mul(
        n(72, 72, 0xb00a),
        Q / 3 + mul(Q - ramp(e, -300, 1200), 3 * Q),
    );
    let detail = mul(n(18, 18, 0xb00b), mul(jagged, 5 * Q));
    let offset = datum + mul(inland, mul(relief, peaks) + rolling + detail);
    let excess = (offset - 130 * Q).max(0);
    let offset = offset - excess
        + (i64::from(excess) * i64::from(32 * Q) / i64::from(excess + 32 * Q)) as i32;
    // Quiet hills/basins do not inherit rugged upper-slope density displacement.
    // Exceptional high ridges keep the full original 12-block/26-cell envelope.
    let amplitude = mul(inland, mul(mul(rugged, ramp(w, -900, 1100)), 12 * Q));
    let spawn = ramp(x.abs().max((z - 10).abs()), 12, 76);
    Parameters {
        continentalness: c,
        erosion: e,
        weirdness: w,
        offset: lerp(offset.max(5 * Q), offset, spawn),
        amplitude: mul(amplitude, spawn),
    }
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
    fn independent_fixed_point_fields_and_every_density_bit() {
        for line in include_str!("../../tests/fixtures/terrain-v13.csv")
            .lines()
            .skip(1)
        {
            let v: Vec<_> = line.split(',').collect();
            let (seed, x, z) = (
                v[0].parse().unwrap(),
                v[1].parse().unwrap(),
                v[2].parse().unwrap(),
            );
            let p = parameters(seed, x, z);
            let profile = profile(seed, x, z);
            let expected: Vec<i32> = v[3..10].iter().map(|s| s.parse().unwrap()).collect();
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
            assert_eq!(
                profile.bits_for_test(),
                v[10].parse::<u64>().unwrap(),
                "{line}"
            );
        }
    }
    #[test]
    fn shared_profiles_match_points_and_neighboring_lattices() {
        for seed in [0, 42, 2026, u64::MAX] {
            for min in [
                [-17, -17],
                [15, 63],
                [-24001, 20001],
                [27999, -30001],
                [-520, -3208],
            ] {
                let (tile, _) = terrain_v4::profiles_with(seed, min, 20, parameters);
                for (i, p) in tile.into_iter().enumerate() {
                    let x = min[0] + i as i32 % 20;
                    let z = min[1] + i as i32 / 20;
                    let point = profile(seed, x, z);
                    assert_eq!(
                        (p.top, p.floor, p.bits_for_test()),
                        (point.top, point.floor, point.bits_for_test())
                    );
                    for y in p.floor - 1..=p.top + 1 {
                        assert_eq!(p.solid(y), point.solid(y));
                    }
                }
            }
        }
    }
    #[test]
    fn bounded_density_preserves_rare_recesses_and_extreme_summits() {
        let (mut breaks, mut high, mut quiet) = (0, 0, 0);
        for seed in [0, 42, 2026, u64::MAX] {
            for z in (-4096..4096).step_by(64) {
                for x in (-4096..4096).step_by(64) {
                    let p = parameters(seed, x, z);
                    let s = profile(seed, x, z);
                    assert!((0..=12 * Q).contains(&p.amplitude));
                    assert!(s.top < 174 && s.floor >= -54, "{seed} {x} {z}");
                    assert!(s.solid(s.floor - 1) && s.solid(s.top) && !s.solid(s.top + 1));
                    breaks += usize::from((s.floor..s.top).any(|y| !s.solid(y)));
                    high += usize::from(s.top >= 130);
                    quiet += usize::from(p.amplitude == 0);
                    for [dx, dz] in [[1, 0], [0, 1]] {
                        let n = parameters(seed, x + dx, z + dz);
                        // Quiet ordinary terrain must stay gentle. Genuine upper slopes
                        // retain cliffs, including seed0 (-960,2688)'s 6.1-block face.
                        let limit = if p.erosion >= 0 && n.erosion >= 0 {
                            2
                        } else {
                            12
                        };
                        assert!(
                            (n.offset - p.offset).abs() < limit * Q,
                            "{seed} {x} {z} {dx} {dz} {p:?} {n:?}"
                        );
                    }
                }
            }
        }
        assert!(
            breaks > 0 && high > 0 && quiet > 0,
            "{breaks} {high} {quiet}"
        );
    }
}
