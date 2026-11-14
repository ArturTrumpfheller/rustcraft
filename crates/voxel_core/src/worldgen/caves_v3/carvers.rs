// SPDX-License-Identifier: MIT OR Apache-2.0
//! Independently authored branching random-walk carvers, informed by Horizon's
//! cave/canyon stages. Not a byte-equivalent port of the reference RNG/path.
//! At most97 root-to-tip advances (one repeated split), maximum radius12:
//! every feature is within125 blocks
//! of its source chunk, so eight source-chunk rings are sufficient.
use super::hash;
#[derive(Debug)]
pub(super) struct Ellipsoid {
    pub(super) center: [i32; 3],
    radius: [i32; 3],
    floor: i32,
    pub(super) min: [i32; 3],
    pub(super) max: [i32; 3],
}
impl Ellipsoid {
    fn new(center: [i32; 3], radius: [i32; 3], floor: i32) -> Self {
        Self {
            center,
            radius,
            floor,
            min: std::array::from_fn(|i| (center[i] - radius[i]).div_euclid(256)),
            max: std::array::from_fn(|i| (center[i] + radius[i]).div_euclid(256)),
        }
    }
    pub(super) fn contains(&self, p: [i32; 3]) -> bool {
        if (0..3).any(|i| p[i] < self.min[i] || p[i] > self.max[i]) {
            return false;
        }
        let d: [i64; 3] = std::array::from_fn(|i| {
            i64::from(p[i] * 256 + 128 - self.center[i]) * 4096 / i64::from(self.radius[i])
        });
        d[1] >= i64::from(self.floor) && d.iter().map(|v| v * v).sum::<i64>() < 4096 * 4096
    }
}
struct Random(u64);
impl Random {
    fn next(&mut self, n: u32) -> i32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        (hash(self.0, 0, 0, 0) % u64::from(n)) as i32
    }
}
// Integer periodic sine, Bhaskara's rational approximation. No host libm/trig
// or target-dependent rounding influences carver topology. One turn = 65536.
fn sin(angle: i32) -> i32 {
    let a = angle.rem_euclid(65536);
    let (x, sign) = if a < 32768 {
        (i64::from(a), 1)
    } else {
        (i64::from(a - 32768), -1)
    };
    let v = x * (32768 - x);
    (sign * 16 * v * 65536 / (5 * 32768i64 * 32768 - 4 * v)) as i32
}
fn reference_y(y: i32) -> i32 {
    if y <= 63 {
        -64 + (y + 64) * 66 / 127
    } else {
        y - 61
    }
}
struct Builder {
    min: [i32; 2],
    max: [i32; 2],
    shapes: Vec<Ellipsoid>,
}
impl Builder {
    fn add(&mut self, shape: Ellipsoid) {
        if shape.min[0] <= self.max[0]
            && shape.max[0] >= self.min[0]
            && shape.min[2] <= self.max[1]
            && shape.max[2] >= self.min[1]
        {
            self.shapes.push(shape);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        rng: &mut Random,
        mut p: [i32; 3],
        mut yaw: i32,
        mut pitch: i32,
        width: i32,
        start: i32,
        end: i32,
        branch: bool,
        canyon: bool,
    ) {
        let mut dyaw = 0;
        let mut dpitch = 0;
        let split = start + (end - start) / 2;
        for step in start..end {
            p[0] += (i64::from(sin(yaw + 16384)) * i64::from(sin(pitch + 16384)) * 256
                / (65536 * 65536i64)) as i32;
            p[2] += (i64::from(sin(yaw)) * i64::from(sin(pitch + 16384)) * 256 / (65536 * 65536i64))
                as i32;
            p[1] += sin(pitch) * 256 / 65536;
            pitch = pitch * 7 / 10 + dpitch / 10;
            yaw += dyaw / 10;
            dpitch = dpitch * 9 / 10 + (rng.next(2049) - 1024) * rng.next(3);
            dyaw = dyaw * 3 / 4 + (rng.next(4097) - 2048) * rng.next(3);
            if branch && step == split && width > 384 {
                self.walk(
                    rng,
                    p,
                    yaw - 16384,
                    pitch / 3,
                    width * 2 / 3,
                    step,
                    end,
                    false,
                    canyon,
                );
                self.walk(
                    rng,
                    p,
                    yaw + 16384,
                    pitch / 3,
                    width * 2 / 3,
                    step,
                    end,
                    false,
                    canyon,
                );
                return;
            }
            let radius = 384 + sin(step * 32768 / end) * width / 65536;
            let ry = if canyon {
                (radius * 3).min(12 * 256)
            } else {
                (radius * 3 / 4).max(256)
            };
            self.add(Ellipsoid::new(
                p,
                [radius, ry, radius],
                if canyon { -4096 } else { -2867 },
            ));
        }
    }
}
pub(super) fn build(seed: u64, min: [i32; 2], max: [i32; 2]) -> Vec<Ellipsoid> {
    let mut b = Builder {
        min,
        max,
        shapes: vec![],
    };
    for z in min[1].div_euclid(16) - 8..=max[1].div_euclid(16) + 8 {
        for x in min[0].div_euclid(16) - 8..=max[0].div_euclid(16) + 8 {
            for (kind, chance, top) in [(0, 150, 180), (1, 70, 47), (2, 10, 67)] {
                let mut r = Random(hash(seed, x, z, 0x390000 + kind));
                if r.next(1000) >= chance {
                    continue;
                }
                let n = if kind == 2 {
                    1
                } else {
                    let a = r.next(15) + 1;
                    let a = r.next(a as u32) + 1;
                    r.next(a as u32)
                };
                for _ in 0..n {
                    let py = if kind == 2 {
                        10 + r.next(58)
                    } else {
                        -56 + r.next((top + 57) as u32)
                    };
                    let p = [
                        (x * 16 + r.next(16)) * 256,
                        reference_y(py) * 256,
                        (z * 16 + r.next(16)) * 256,
                    ];
                    let yaw = r.next(65536);
                    let pitch = r.next(4097) - 2048;
                    let width = if kind == 2 {
                        256 + r.next(512)
                    } else {
                        128 + r.next(257) + r.next(257)
                    };
                    let length = 72 + r.next(25);
                    if kind != 2 && r.next(4) == 0 {
                        b.add(Ellipsoid::new(p, [width * 2, width, width * 2], -2867));
                    }
                    b.walk(&mut r, p, yaw, pitch, width, 0, length, true, kind == 2);
                }
            }
        }
    }
    b.shapes
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_discovery_and_signed_seams() {
        for seed in [0, 42, 2026, u64::MAX] {
            let a = build(seed, [-32, -32], [31, 31]);
            let b = build(seed, [-1, -1], [16, 16]);
            for x in -1..=16 {
                for z in -1..=16 {
                    for y in [-60, -48, -32, -16, 0, 16, 32, 64] {
                        let p = [x, y, z];
                        assert_eq!(
                            a.iter().any(|s| s.contains(p)),
                            b.iter().any(|s| s.contains(p)),
                            "{seed} {p:?}"
                        );
                    }
                }
            }
        }
    }
}
