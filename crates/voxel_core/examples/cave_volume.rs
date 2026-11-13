// SPDX-License-Identifier: MIT OR Apache-2.0
//! Export bounded, final voxel occupancy for cross-generator cave analysis.
//! No analysis buffers are linked into the game. The external analyzer also
//! accepts authorized vanilla world exports in this same documented format.
use std::{env, fs, io::Write, path::Path};
use voxel_core::{
    Block, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator, terrain_v5},
};

fn scramble(mut v: u64) -> u64 {
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}

fn plan() {
    println!("id,seed,x,z,n,stratum,attempt,mean_height,relief");
    let seeds = (0u64..32).map(|i| match i {
        0 => 0,
        1 => 42,
        2 => 2026,
        3 => u64::MAX,
        _ => scramble(i.wrapping_mul(0x9e3779b97f4a7c1)),
    });
    for (si, seed) in seeds.enumerate() {
        for site in 0..8 {
            let stratum = match site {
                0..=1 => "near-unconditional",
                2..=3 => "distant-unconditional",
                4..=5 => "lowland-conditional",
                _ => "mountain-conditional",
            };
            let mut selected = None;
            for attempt in 0..2048 {
                let h = scramble(seed ^ (site * 314159 + attempt * 271828 + 1));
                let limit = if site < 2 { 256 } else { 3584 };
                let x = ((h % limit) as i32 - limit as i32 / 2) * 16;
                let z = (((h >> 24) % limit) as i32 - limit as i32 / 2) * 16;
                let heights: Vec<_> = [0, 64, 127]
                    .into_iter()
                    .flat_map(|dz| {
                        [0, 64, 127].map(move |dx| terrain_v5::height(seed, x + dx, z + dz))
                    })
                    .collect();
                let mean = heights.iter().sum::<i32>() / 9;
                let relief = heights.iter().max().unwrap() - heights.iter().min().unwrap();
                if site < 4
                    || (site < 6 && (20..=42).contains(&mean) && relief <= 28)
                    || (site >= 6 && mean >= 78 && relief >= 12)
                {
                    selected = Some((x, z, attempt, mean, relief));
                    break;
                }
            }
            let (x, z, attempt, mean, relief) =
                selected.expect("bounded terrain-only stratum search exhausted");
            println!("s{si:02}-{site},{seed},{x},{z},128,{stratum},{attempt},{mean},{relief}");
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.get(1).is_some_and(|s| s == "--plan") {
        plan();
        return;
    }
    assert_eq!(
        args.len(),
        4,
        "cave_volume VERSION PLAN.csv OUTPUT_DIRECTORY | --plan"
    );
    let version: u32 = args[1].parse().unwrap();
    fs::create_dir_all(&args[3]).unwrap();
    for line in fs::read_to_string(&args[2]).unwrap().lines().skip(1) {
        let row: Vec<_> = line.split(',').collect();
        let seed: u64 = row[1].parse().unwrap();
        let min: [i32; 2] = [row[2].parse().unwrap(), row[3].parse().unwrap()];
        let n: usize = row[4].parse().unwrap();
        assert!(n.is_multiple_of(16) && n <= 256 && min.iter().all(|v| v.rem_euclid(16) == 0));
        assert!(min.iter().all(|&v| v >= -32768 && v + n as i32 <= 32768));
        let dest = Path::new(&args[3]).join(format!("{}.rcv", row[0]));
        assert!(!dest.exists(), "refusing to overwrite {}", dest.display());
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let mut cells = vec![0u8; n * n * 256];
        let mut heights = vec![-64i16; n * n];
        for cz in 0..n / 16 {
            for cx in 0..n / 16 {
                let s = g.sampler([min[0] / 16 + cx as i32, min[1] / 16 + cz as i32]);
                for cy in -4..12 {
                    let cp = ChunkPos([min[0] / 16 + cx as i32, cy, min[1] / 16 + cz as i32]);
                    let c = s.chunk(cp).unwrap();
                    for z in 0..16 {
                        for x in 0..16 {
                            let column = (cz * 16 + z) * n + cx * 16 + x;
                            for y in 0..16 {
                                let yy = cy * 16 + y as i32;
                                let b = c.get(LocalPos::new([x, y, z]).unwrap());
                                // 0 solid; 1 dry Air; 2 fluid; 4 other obstacles.
                                // Other non-solid blocks never count as floor support.
                                cells[column * 256 + (yy + 64) as usize] = if b == Block::Air {
                                    1
                                } else if b.fluid().is_some() {
                                    2
                                } else if b.is_solid() {
                                    0
                                } else {
                                    4
                                };
                                if matches!(
                                    b,
                                    Block::Stone
                                        | Block::Dirt
                                        | Block::Grass
                                        | Block::Sand
                                        | Block::SnowyGround
                                ) {
                                    heights[column] = yy as i16;
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut out = fs::File::create(&dest).unwrap();
        out.write_all(b"RCV1").unwrap();
        for v in [n as i16, 256, -64] {
            out.write_all(&v.to_le_bytes()).unwrap();
        }
        out.write_all(&seed.to_le_bytes()).unwrap();
        for v in min {
            out.write_all(&v.to_le_bytes()).unwrap();
        }
        for v in heights {
            out.write_all(&v.to_le_bytes()).unwrap();
        }
        out.write_all(&cells).unwrap();
        println!("{}", dest.display());
    }
}
