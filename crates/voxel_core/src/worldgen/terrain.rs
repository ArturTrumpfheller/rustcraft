//! Frozen generator v4 geographical shape. All intermediate heights are Q10;
//! no floats, global maps, clipping, or chunk-aligned terrain classifications.
use super::noise;
const Q: i32 = 1024;

fn smooth(value: i32, low: i32, high: i32) -> i32 {
    let t = ((value - low) * Q / (high - low)).clamp(0, Q);
    t * t / Q * (3 * Q - 2 * t) / Q
}
fn blend(a: i32, b: i32, weight: i32) -> i32 {
    a + (b - a) * weight / Q
}

pub(super) fn height(seed: u64, x: i32, z: i32) -> i32 {
    // A bounded 36-block warp breaks lattice alignment over many chunks.
    let wx = x + noise(seed, x, z, 384, 101) * 36 / Q;
    let wz = z + noise(seed, x, z, 384, 103) * 36 / Q;
    let geography = noise(seed, wx, wz, 512, 107);
    let hills = smooth(geography, -450, 100);
    let mountains = smooth(geography, 50, 650);

    // Broad usable lowlands; high-frequency noise stays below one block.
    let plain = 4 * Q + noise(seed, wx, wz, 384, 109) * 2 + noise(seed, wx, wz, 64, 113) / 2;
    let rolling = 7 * Q + noise(seed, wx, wz, 96, 127) * 5 + noise(seed, wx, wz, 40, 131) * 2;

    // Skewed coordinates and squared ridges form connected crests and valleys,
    // rather than circular hills everywhere. Broad geography controls exposure.
    let ridge = Q - noise(seed, wx + wz / 2, wz - wx / 3, 96, 137).abs();
    let ridge = ridge * ridge / Q;
    let mountain = 10 * Q + ridge * 12 + noise(seed, wx, wz, 128, 139) * 2;
    let detail = noise(seed, wx, wz, 12, 149) * (128 + mountains / 4) / Q;
    // Convex blends bound the rounded surface to [-1,24]: no flattened height cap.
    // The highest surface is Y=24; tallest crown ends at30, leaving sky at31.
    (blend(blend(plain, rolling, hills), mountain, mountains) + detail).div_euclid(Q)
}
