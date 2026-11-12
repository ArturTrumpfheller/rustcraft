// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generator 10 geography. Q12 fields, Q8 warped coordinates, no float decisions.
//! Heights are independent of discrete biome classifications. All neighborhoods
//! are bounded and evaluated in absolute coordinates, including outside the halo.
use super::{biome, hash};
use crate::Block;
const Q: i32 = 4096;
fn mul(a: i32, b: i32) -> i32 {
    (i64::from(a) * i64::from(b) / i64::from(Q)) as i32
}
fn smooth(v: i32, lo: i32, hi: i32) -> i32 {
    let t = ((v - lo) * Q / (hi - lo)).clamp(0, Q);
    mul(mul(t, t), 3 * Q - 2 * t)
}
fn blend(a: i32, b: i32, t: i32) -> i32 {
    a + mul(b - a, t)
}
// Q8 positions preserve sub-block warp precision. Q12 interpolation prevents
// multiplying the historical Q10 quantization into visible mountain terraces.
// Specialize fixed scales at call sites; avoid repeated dynamic lattice divides.
#[inline]
fn field(seed: u64, x: i32, z: i32, scale: i32, salt: u64) -> i32 {
    let size = scale * 256;
    let cx = x.div_euclid(size);
    let cz = z.div_euclid(size);
    let fade = |v| smooth(v, 0, size);
    let tx = fade(x.rem_euclid(size));
    let tz = fade(z.rem_euclid(size));
    let at = |dx, dz| (hash(seed, cx + dx, cz + dz, salt) % 8193) as i32 - Q;
    blend(
        blend(at(0, 0), at(1, 0), tx),
        blend(at(0, 1), at(1, 1), tx),
        tz,
    )
}
/// Pre-water surface, useful for reproducible macro-scale quality surveys.
/// The runtime samples this same function once per cached material-grid cell.
pub fn height(seed: u64, x: i32, z: i32) -> i32 {
    let px = x * 256;
    let pz = z * 256;
    let wx = px + field(seed, px, pz, 896, 0xa001) * 112 * 256 / Q;
    let wz = pz + field(seed, px, pz, 896, 0xa002) * 112 * 256 / Q;
    let continental = field(seed, wx, wz, 1536, 0xa003);
    let region = field(seed, wx, wz, 1024, 0xa004);
    let erosion = field(seed, wx, wz, 768, 0xa005);
    let upland = mul(smooth(region, -700, 2100), Q - smooth(erosion, 0, 2900));
    let ridge_field = field(seed, wx + wz / 3, wz - wx / 4, 256, 0xa006).abs();
    // Rounded broad crests; secondary folds vary the massifs without thin spikes.
    let ridge = Q - smooth(ridge_field, 0, Q);
    let folds = field(seed, wx, wz, 112, 0xa007);
    let valley = Q - smooth(field(seed, wx, wz, 640, 0xa008).abs(), 160, 1550);
    let relief = mul(upland, Q - mul(valley, 3400));
    let plain = 8 * Q + continental * 6 + field(seed, wx, wz, 384, 0xa009) * 2;
    let hills = field(seed, wx, wz, 144, 0xa00a) * 12;
    let foothills = mul(smooth(region, -1900, 1500), Q - mul(valley, 3000));
    let mountain = 38 * Q + ridge * 85 + folds * 9 + field(seed, wx, wz, 512, 0xa00b) * 18;
    let land = plain + mul(hills, foothills) + mul(mountain, relief);
    // Coast shelves are part of the same continuous height field. A convex
    // blend avoids abruptly subtracting a fixed ocean depth from tall mountains.
    let coast = smooth(continental, -2400, -400);
    let ocean = -22 * Q + (continental + Q) * 3;
    let detail = mul(field(seed, wx, wz, 32, 0xa00c), 500 + relief / 4);
    let h = (blend(ocean, land, coast) + detail).div_euclid(Q);
    // Preserve the supported spawn shore with a smooth 64-block approach.
    let distance = x.abs().max((z - 10).abs());
    blend(h.max(4) * Q, h * Q, smooth(distance, 12, 76)).div_euclid(Q)
}
pub(super) fn material(seed: u64, x: i32, z: i32, h: i32, slope: i32) -> biome::Column {
    let c = biome::sample_climate(seed, x.clamp(-32768, 32767), z.clamp(-32768, 32767)).unwrap();
    resurface(seed, x, z, h, slope, c)
}
pub(super) fn resurface(
    seed: u64,
    x: i32,
    z: i32,
    h: i32,
    slope: i32,
    c: biome::Climate,
) -> biome::Column {
    let edge = super::noise(seed, x, z, 48, 0xa010) * 55 / 1024;
    let cold = c.temperature - (h - 32).max(0) * 9;
    let top = if slope >= 3 {
        Block::Stone
    } else if cold < -280 + edge {
        Block::SnowyGround
    } else if h > 82 || (c.geography > 240 && h > 28) {
        Block::Stone
    } else if biome::classify(c) == biome::Biome::Desert {
        Block::Sand
    } else {
        Block::Grass
    };
    biome::Column {
        height: h,
        climate: c,
        top,
        soil: match top {
            Block::Sand => Block::Sand,
            Block::Stone => Block::Stone,
            _ if h > 64 || slope >= 2 => Block::Stone,
            _ => Block::Dirt,
        },
    }
}
pub(super) fn columns(seed: u64, min: [i32; 2]) -> Vec<biome::Column> {
    let grid: Vec<_> = (0..400)
        .map(|i| height(seed, min[0] + i % 20 - 1, min[1] + i / 20 - 1))
        .collect();
    (0..324)
        .map(|i| {
            let x = i % 18 + 1;
            let z = i / 18 + 1;
            let j = (z * 20 + x) as usize;
            let slope = [j - 1, j + 1, j - 20, j + 20]
                .into_iter()
                .map(|n| (grid[n] - grid[j]).abs())
                .max()
                .unwrap();
            material(seed, min[0] + x - 1, min[1] + z - 1, grid[j], slope)
        })
        .collect()
}
