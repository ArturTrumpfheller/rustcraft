//! Generator v5 climate and terrain. Integer Q10, bounded world-coordinate queries.
use super::{hash, noise};
use crate::Block;
const Q: i32 = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    SnowyPlains,
    RockyHighlands,
}
impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Forest => "Forest",
            Self::Desert => "Desert",
            Self::SnowyPlains => "Snowy Plains",
            Self::RockyHighlands => "Rocky Highlands",
        }
    }
}
/// Signed Q10 climate. Geography is the broad Slice24 terrain-character field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Climate {
    pub temperature: i32,
    pub moisture: i32,
    pub geography: i32,
}
fn smooth(v: i32, low: i32, high: i32) -> i32 {
    let t = ((v - low) * Q / (high - low)).clamp(0, Q);
    t * t / Q * (3 * Q - 2 * t) / Q
}
fn blend(a: i32, b: i32, t: i32) -> i32 {
    a + (b - a) * t / Q
}
fn warped(seed: u64, x: i32, z: i32) -> [i32; 2] {
    [
        x + noise(seed, x, z, 384, 101) * 36 / Q,
        z + noise(seed, x, z, 384, 103) * 36 / Q,
    ]
}
fn climate_at(seed: u64, [x, z]: [i32; 2]) -> Climate {
    Climate {
        temperature: (noise(seed, x, z, 768, 211) * 5 + noise(seed, x, z, 384, 223)) / 6,
        moisture: (noise(seed, x, z, 640, 227) * 4 + noise(seed, x, z, 320, 229)) / 5,
        geography: noise(seed, x, z, 512, 107),
    }
}
/// Independent of chunks and request order. Outside supported X/Z returns None.
pub fn sample_climate(seed: u64, x: i32, z: i32) -> Option<Climate> {
    in_bounds(x, z).then(|| climate_at(seed, warped(seed, x, z)))
}
fn in_bounds(x: i32, z: i32) -> bool {
    (-32768..32768).contains(&x) && (-32768..32768).contains(&z)
}
pub fn classify(c: Climate) -> Biome {
    if c.geography > 240 {
        Biome::RockyHighlands
    } else if c.temperature < -260 {
        Biome::SnowyPlains
    } else if c.temperature > 180 && c.moisture < -100 {
        Biome::Desert
    } else if c.moisture > 80 {
        Biome::Forest
    } else {
        Biome::Plains
    }
}
pub fn sample_biome(seed: u64, x: i32, z: i32) -> Option<Biome> {
    sample_climate(seed, x, z).map(classify)
}
fn shape(seed: u64, x: i32, z: i32) -> (i32, Climate) {
    let [wx, wz] = warped(seed, x, z);
    let c = climate_at(seed, [wx, wz]);
    let dry = smooth(c.temperature, 80, 300) * (Q - smooth(c.moisture, -260, -20)) / Q;
    let cold = Q - smooth(c.temperature, -380, -140);
    let hills = smooth(c.geography, -450, 100) * (Q - dry * 3 / 4) / Q * (Q - cold / 2) / Q;
    let mountains = smooth(c.geography, 50, 650);
    // Slice24's broad plains/rolling/ridge model, with continuous biome weights.
    let plain = 4 * Q + noise(seed, wx, wz, 384, 109) * 2 + noise(seed, wx, wz, 64, 113) / 2;
    let rolling = 7 * Q + noise(seed, wx, wz, 96, 127) * 5 + noise(seed, wx, wz, 40, 131) * 2;
    let dune = 4 * Q + noise(seed, wx, wz, 80, 233) * 2 + noise(seed, wx, wz, 28, 239) / 2;
    let lowland = blend(blend(plain, rolling, hills), dune, dry);
    let ridge = Q - noise(seed, wx + wz / 2, wz - wx / 3, 96, 137).abs();
    let mountain = 10 * Q + ridge * ridge / Q * 12 + noise(seed, wx, wz, 128, 139) * 2;
    let detail = noise(seed, wx, wz, 12, 149) * (128 + mountains / 4) / Q;
    (
        (blend(lowland, mountain, mountains) + detail).div_euclid(Q),
        c,
    )
}
pub(super) fn height(seed: u64, x: i32, z: i32) -> i32 {
    shape(seed, x, z).0
}

#[derive(Clone, Copy)]
pub(super) struct Column {
    pub height: i32,
    pub climate: Climate,
    pub top: Block,
    pub soil: Block,
}
fn material(seed: u64, x: i32, z: i32, h: i32, c: Climate, slope: i32) -> Column {
    // One continuous 24-block field wrinkles the ecotone; no per-block dither.
    let edge = noise(seed, x, z, 24, 241) * 65 / Q;
    let cold = c.temperature - (h - 10).max(0) * 18;
    let top = if c.geography > 160 && (slope >= 2 || h >= 13) {
        if cold < -340 + edge {
            Block::SnowyGround
        } else {
            Block::Stone
        }
    } else if cold < -260 + edge {
        Block::SnowyGround
    } else if c.temperature > 180 + edge && c.moisture < -100 + edge && c.geography <= 240 {
        Block::Sand
    } else {
        Block::Grass
    };
    Column {
        height: h,
        climate: c,
        top,
        soil: if top == Block::Sand {
            Block::Sand
        } else {
            Block::Dirt
        },
    }
}
pub(super) fn column(seed: u64, x: i32, z: i32) -> Column {
    let (h, c) = shape(seed, x, z);
    let slope = [[-1, 0], [1, 0], [0, -1], [0, 1]]
        .into_iter()
        .map(|[dx, dz]| (height(seed, x + dx, z + dz) - h).abs())
        .max()
        .unwrap();
    material(seed, x, z, h, c, slope)
}
/// The 20² temporary height/climate grid supplies all 18² surface/halo samples.
pub(super) fn columns(seed: u64, min: [i32; 2]) -> Vec<Column> {
    let mut grid = Vec::with_capacity(400);
    for z in min[1] - 1..=min[1] + 18 {
        for x in min[0] - 1..=min[0] + 18 {
            grid.push(shape(seed, x, z));
        }
    }
    let mut result = Vec::with_capacity(324);
    for z in 1..19 {
        for x in 1..19 {
            let i = z * 20 + x;
            let (h, c) = grid[i];
            let slope = [i - 1, i + 1, i - 20, i + 20]
                .into_iter()
                .map(|n| (grid[n].0 - h).abs())
                .max()
                .unwrap();
            result.push(material(
                seed,
                min[0] + x as i32 - 1,
                min[1] + z as i32 - 1,
                h,
                c,
                slope,
            ));
        }
    }
    result
}
/// Probability per jittered eight-block candidate, per mille. Clearings vary at64 blocks.
pub(super) fn tree_density(seed: u64, x: i32, z: i32) -> u64 {
    let c = sample_climate(seed, x, z).unwrap();
    let forest = smooth(c.moisture, -40, 200);
    let cold = Q - smooth(c.temperature, -340, -180);
    let desert = smooth(c.temperature, 100, 260) * (Q - smooth(c.moisture, -200, -20)) / Q;
    let highland = smooth(c.geography, 100, 380);
    let density = blend(95, 780, forest) * (Q - cold * 4 / 5) / Q * (Q - desert) / Q
        * (Q - highland * 19 / 20)
        / Q;
    let clearing = 650 + noise(seed, x, z, 64, 251) * 350 / Q;
    (density * clearing / Q).max(0) as u64
}
pub(super) fn candidate(seed: u64, gx: i32, gz: i32) -> (i32, i32, u64) {
    let h = hash(seed, gx, gz, 257);
    (
        gx * 8 + (h % 6) as i32 + 1,
        gz * 8 + ((h >> 8) % 6) as i32 + 1,
        h,
    )
}
