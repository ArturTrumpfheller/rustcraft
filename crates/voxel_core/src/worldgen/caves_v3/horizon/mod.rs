// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ludovic Marie and Horizon contributors
// Copyright (c) 2026 RustCraft contributors
//! Small adapted scalar subset of Horizon 0de8932: named xoroshiro seeds,
//! gradient Perlin and paired octave stacks. See LICENSE; no game resources.
//! Deliberately excludes Horizon's graph engine, renderer and platform code.
mod hash;
#[derive(Clone)]
struct Random([u64; 2]);
impl Random {
    fn state(v: [u64; 2]) -> Self {
        Self(if v == [0, 0] {
            [0x9e3779b97f4a7c15, 0x6a09e667f3bcc909]
        } else {
            v
        })
    }
    fn new(seed: u64) -> Self {
        fn mix(mut v: u64) -> u64 {
            v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
            v ^ (v >> 31)
        }
        let s = seed ^ 0x6a09e667f3bcc909;
        Self::state([mix(s), mix(s.wrapping_add(0x9e3779b97f4a7c15))])
    }
    fn next(&mut self) -> u64 {
        let [a, b] = self.0;
        let c = a ^ b;
        self.0 = [a.rotate_left(49) ^ c ^ (c << 21), c.rotate_left(28)];
        a.wrapping_add(b).rotate_left(17).wrapping_add(a)
    }
    fn fork(&mut self) -> [u64; 2] {
        [self.next(), self.next()]
    }
    fn named(factory: [u64; 2], name: &str) -> Self {
        let h = hash::seed_from_hash(name);
        Self::state([factory[0] ^ h[0], factory[1] ^ h[1]])
    }
    fn double(&mut self) -> f64 {
        (self.next() >> 11) as f64 / 9_007_199_254_740_992.0
    }
    fn bound(&mut self, n: u32) -> usize {
        let threshold = n.wrapping_neg() % n;
        loop {
            let v = u64::from(self.next() as u32) * u64::from(n);
            if v as u32 >= threshold {
                return (v >> 32) as usize;
            }
        }
    }
}
struct Perlin {
    offsets: [f64; 3],
    table: [u8; 256],
}
impl Perlin {
    fn new(mut rng: Random) -> Self {
        let offsets = std::array::from_fn(|_| rng.double() * 256.0);
        let mut table = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = i + rng.bound((256 - i) as u32);
            table.swap(i, j);
        }
        Self { offsets, table }
    }
    #[inline]
    fn get(&self, p: [f64; 3]) -> f32 {
        const G: [[f32; 3]; 16] = [
            [1., 1., 0.],
            [-1., 1., 0.],
            [1., -1., 0.],
            [-1., -1., 0.],
            [1., 0., 1.],
            [-1., 0., 1.],
            [1., 0., -1.],
            [-1., 0., -1.],
            [0., 1., 1.],
            [0., -1., 1.],
            [0., 1., -1.],
            [0., -1., -1.],
            [1., 1., 0.],
            [0., -1., 1.],
            [-1., 1., 0.],
            [0., -1., -1.],
        ];
        let p: [f64; 3] = std::array::from_fn(|i| {
            let v = p[i];
            let v = if (-16_777_215.999_999_998..16_777_215.999_999_998).contains(&v) {
                v
            } else {
                v - (v / 33_554_432.0 + 0.5).floor() * 33_554_432.0
            };
            v + self.offsets[i]
        });
        // Wrapped, finite coordinates are inside i32. Truncation plus one
        // signed adjustment is exact floor, without a host libm call.
        let cell = p.map(|v| {
            let n = v as i32;
            n - i32::from(v < f64::from(n))
        });
        let f: [f32; 3] = std::array::from_fn(|i| (p[i] - f64::from(cell[i])) as f32);
        let perm = |i: i32| i32::from(self.table[(i & 255) as usize]);
        let rows: [i32; 4] = std::array::from_fn(|i| {
            perm(perm(cell[0] + (i & 1) as i32) + cell[1] + ((i >> 1) & 1) as i32)
        });
        let values: [f32; 8] = std::array::from_fn(|i| {
            let [x, y, z] = [i & 1, (i >> 1) & 1, (i >> 2) & 1];
            let g = G[(perm(rows[i & 3] + cell[2] + z as i32) & 15) as usize];
            g[0] * (f[0] - x as f32) + g[1] * (f[1] - y as f32) + g[2] * (f[2] - z as f32)
        });
        let fade = f.map(|v| v * v * v * (v * (v * 6. - 15.) + 10.));
        let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);
        let rows: [f32; 4] =
            std::array::from_fn(|i| lerp(values[i * 2], values[i * 2 + 1], fade[0]));
        lerp(
            lerp(rows[0], rows[1], fade[1]),
            lerp(rows[2], rows[3], fade[1]),
            fade[2],
        )
    }
}
pub(super) struct Noise(Vec<(Perlin, f64, f32)>);
impl Noise {
    /// Functional octave parameters, interpreted as Horizon's default normalized
    /// NormalNoise. The paired stacks share octave names, not mutable state.
    pub(super) fn new(seed: u64, name: &str, base: i32, amplitude: f64, modifiers: &[f64]) -> Self {
        let mut root = Random::new(seed);
        let mut rng = Random::named(root.fork(), &format!("minecraft:{name}"));
        let factories = [rng.fork(), rng.fork()];
        let count = modifiers.len() as i32;
        let initial = 2f64.powi(count - 1) / (2f64.powi(count) - 1.0);
        let weights: Vec<_> = modifiers
            .iter()
            .enumerate()
            .map(|(i, v)| initial * v * 2f64.powi(-(i as i32)))
            .collect();
        let sum: f64 = weights.iter().sum();
        let variance: f64 = weights
            .iter()
            .map(|v| (0.270_224_783_124_521_1 * v).powi(2))
            .sum();
        let normalize = sum / 3.0 / (variance.sqrt() * 2f64.sqrt());
        let mut layers = vec![];
        for (i, &w) in weights.iter().enumerate() {
            if w == 0.0 {
                continue;
            }
            for (j, factory) in factories.iter().enumerate() {
                let perlin = Perlin::new(Random::named(
                    *factory,
                    &format!("octave_{}", base + i as i32),
                ));
                let frequency =
                    2f64.powi(base + i as i32) * if j == 0 { 1.0 } else { 1.018_126_888_217_522_7 };
                layers.push((perlin, frequency, (normalize * amplitude * w) as f32));
            }
        }
        Self(layers)
    }
    #[inline]
    pub(super) fn get(&self, p: [f64; 3]) -> f32 {
        let mut result = 0.;
        for (noise, frequency, amplitude) in &self.0 {
            result += amplitude * noise.get(p.map(|v| v * frequency));
        }
        result
    }
}
