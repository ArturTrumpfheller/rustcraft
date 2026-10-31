// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared by the read-only audit CLI and the bounded integration regression.
use std::collections::BTreeMap;
use voxel_core::{
    Block, BlockPos, Chunk, ChunkPos, LocalPos, World,
    fluid::FluidConfig,
    regions::{RegionIdentity, RegionPos},
    worldgen::{GenerationConfig, Generator},
};

pub const SEEDS: [u64; 3] = [0, 42, 2026];
// Fixed workload, never an unbounded search for a conveniently passing scene.
// Names identify survey locations; actual biome/feature counts are reported.
pub const SITES: [(&str, [i32; 2]); 15] = [
    ("open-plains", [-160, -256]),
    ("flower-plains", [-152, -256]),
    ("dense-forest", [-136, -256]),
    ("snow-spruce", [-256, -256]),
    ("desert", [216, -248]),
    ("rocky", [-236, -256]),
    ("lake-shore", [-28, 1]),
    ("river", [-7, -30]),
    ("ocean", [6, 6]),
    ("small-cave", [-4, -9]),
    ("large-cavern", [3, -4]),
    ("underground-lake", [-7, -2]),
    ("signed-region-seam", [-4, -1]),
    ("negative-limit", [-2048, 2047]),
    ("positive-limit", [2047, -2048]),
];

#[derive(Debug)]
pub struct Report {
    pub seed: u64,
    pub version: u32,
    pub column: [i32; 2],
    pub biome: String,
    pub crc: u32,
    pub plants: usize,
    pub wood: usize,
    pub leaves: usize,
    pub water: usize,
    pub cave_air: usize,
    pub underground_water: usize,
    pub voxel_bytes: usize,
    pub findings: Vec<String>,
}
impl Report {
    fn check(&mut self, ok: bool, p: BlockPos, feature: &str, expected: &str, actual: &str) {
        if !ok && self.findings.len() < 16 {
            self.findings.push(format!(
                "seed={} generator={} pos={:?} feature={feature} expected={expected} actual={actual}; reproduce: cargo run --release --locked -p voxel_core --example worldgen_audit -- {} {} {} {}",
                self.seed, self.version, p.0, self.version, self.seed, self.column[0], self.column[1]
            ));
        }
    }
}

pub fn column(g: &Generator, c: [i32; 2]) -> Vec<(ChunkPos, Chunk)> {
    let s = g.sampler(c);
    g.world_height()
        .chunks()
        .map(|y| {
            let p = ChunkPos([c[0], y, c[1]]);
            (p, s.chunk(p).unwrap())
        })
        .collect()
}

pub fn crc(chunks: &[(ChunkPos, Chunk)]) -> u32 {
    let mut crc = crc32fast::Hasher::new();
    for (_, ch) in chunks {
        for i in 0..4096 {
            let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
            crc.update(&(ch.get(l) as u16).to_le_bytes());
        }
    }
    crc.finalize()
}

pub fn audit(seed: u64, version: u32, c: [i32; 2]) -> Report {
    let g = Generator::new(GenerationConfig { seed, version }).unwrap();
    assert!(version >= 3, "regional audit requires a local generator");
    let expected = column(&g, c);
    let mut world = World::regional(RegionIdentity {
        seed,
        generator: version,
        nonce: [30; 16],
    });
    let limit = g.chunk_limit();
    // At most nine columns / 144 compact chunk descriptors, dropped per site.
    for z in (c[1] - 1).max(-limit)..=(c[1] + 1).min(limit - 1) {
        for x in (c[0] - 1).max(-limit)..=(c[0] + 1).min(limit - 1) {
            world.install_region(
                RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            for (p, ch) in column(&g, [x, z]).into_iter().rev() {
                world.insert_chunk(p, ch);
            }
        }
    }
    let mut r = Report {
        seed,
        version,
        column: c,
        biome: g
            .biome(c[0] * 16 + 8, c[1] * 16 + 8)
            .map_or("historical", |b| b.name())
            .into(),
        crc: crc(&expected),
        plants: 0,
        wood: 0,
        leaves: 0,
        water: 0,
        cave_air: 0,
        underground_water: 0,
        voxel_bytes: world.allocated_voxel_bytes(),
        findings: vec![],
    };
    let sampler = g.sampler(c);
    let heights: Vec<_> = (0..256)
        .map(|i| g.height(c[0] * 16 + i % 16, c[1] * 16 + i / 16).unwrap())
        .collect();
    for (cp, ch) in &expected {
        r.check(
            world.chunk(*cp) == Some(ch),
            BlockPos(cp.0.map(|v| v * 16)),
            "reverse-order",
            "identical chunks",
            "different chunks",
        );
        for i in 0..4096 {
            let lp = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
            let p = cp.block_position(lp).unwrap();
            let b = ch.get(lp);
            let h = heights[i % 256];
            r.water += usize::from(b.fluid().is_some());
            r.cave_air += usize::from(p.0[1] <= h && b == Block::Air);
            r.underground_water += usize::from(p.0[1] <= h && b.fluid().is_some());
            if version >= 7 && p.0[1] < g.world_height().min + 4 {
                r.check(
                    b == Block::Stone,
                    p,
                    "foundation",
                    "Stone",
                    &format!("{b:?}"),
                );
            }
            if b.is_plant() {
                r.plants += 1;
                r.check(
                    b.supported_by(world.block(p.offset([0, -1, 0]).unwrap())),
                    p,
                    "plant-support",
                    "compatible dry support",
                    "unsupported plant",
                );
                r.check(
                    p.0[1] == h + 1,
                    p,
                    "plant-height",
                    "surface + 1",
                    "wrong height",
                );
            }
            if matches!(b, Block::Wood | Block::BirchWood | Block::SpruceWood) {
                r.wood += 1;
                let below = world.block(p.offset([0, -1, 0]).unwrap());
                r.check(
                    below == b || matches!(below, Block::Grass | Block::SnowyGround),
                    p,
                    "tree-root",
                    "same trunk or dry ground",
                    &format!("{below:?}"),
                );
            }
            if matches!(b, Block::Leaves | Block::BirchLeaves | Block::SpruceLeaves) {
                r.leaves += 1;
                let wood = match b {
                    Block::Leaves => Block::Wood,
                    Block::BirchLeaves => Block::BirchWood,
                    _ => Block::SpruceWood,
                };
                // Every released crown is within two horizontal blocks of its
                // stem, at the same level or immediately below the crown tip.
                let anchored = (-2..=2).any(|dx| {
                    (-2..=2).any(|dz| {
                        [-1, 0]
                            .into_iter()
                            .any(|dy| world.block(p.offset([dx, dy, dz]).unwrap()) == wood)
                    })
                });
                r.check(
                    anchored,
                    p,
                    "canopy-anchor",
                    "nearby species trunk",
                    "no trunk in crown footprint",
                );
            }
            if i % 997 == 0 || b.is_plant() {
                r.check(
                    sampler.block(p) == b,
                    p,
                    "sampler-chunk",
                    &format!("{b:?}"),
                    &format!("{:?}", sampler.block(p)),
                );
            }
        }
        for lp in [[0; 3], [15; 3], [0, 15, 15], [15, 0, 0]] {
            let lp = LocalPos::new(lp).unwrap();
            let p = cp.block_position(lp).unwrap();
            r.check(
                g.block(p) == ch.get(lp),
                p,
                "point-chunk",
                &format!("{:?}", ch.get(lp)),
                &format!("{:?}", g.block(p)),
            );
        }
    }
    for [dx, dz] in [[-1, 0], [16, 15], [0, -1], [15, 16]] {
        for y in [
            g.world_height().min,
            -17,
            -1,
            0,
            15,
            16,
            31,
            47,
            g.world_height().max - 1,
        ] {
            if !g.world_height().contains(y) {
                continue;
            }
            let p = BlockPos([c[0] * 16 + dx, y, c[1] * 16 + dz]);
            r.check(
                sampler.block(p) == world.block(p),
                p,
                "halo-seam",
                &format!("{:?}", world.block(p)),
                &format!("{:?}", sampler.block(p)),
            );
        }
    }
    for y in [g.world_height().min - 1, g.world_height().max] {
        let p = BlockPos([c[0] * 16, y, c[1] * 16]);
        r.check(
            g.block(p) == Block::Air,
            p,
            "vertical-bounds",
            "Air",
            &format!("{:?}", g.block(p)),
        );
        r.check(
            g.chunk(p.split().0).is_none(),
            p,
            "chunk-bounds",
            "None",
            "chunk outside height",
        );
    }
    if version >= 8 {
        let mut changes = vec![];
        for _ in 0..8 {
            world.tick_fluids(FluidConfig::default(), &mut changes);
            r.check(
                changes.is_empty() && world.fluid_updates().processed == 0,
                BlockPos([c[0] * 16, 0, c[1] * 16]),
                "pristine-fluid-work",
                "zero work and edits",
                "fluid work in stable baseline",
            );
        }
    }
    r.check(
        world.edit_count() == 0,
        BlockPos([c[0] * 16, 0, c[1] * 16]),
        "baseline-authority",
        "no edit records",
        "generated edit records",
    );
    r.check(
        world.chunk_count() <= 9 * g.world_height().chunk_count(),
        BlockPos([c[0] * 16, 0, c[1] * 16]),
        "residency",
        "at most nine columns",
        "excess descriptors",
    );
    r
}

pub fn parallel_parity(seed: u64, version: u32, c: [i32; 2]) {
    let g = Generator::new(GenerationConfig { seed, version }).unwrap();
    let other = [
        if c[0] == g.chunk_limit() - 1 {
            c[0] - 1
        } else {
            c[0] + 1
        },
        c[1],
    ];
    let expected: BTreeMap<_, _> = [c, other].map(|c| (c, column(&g, c))).into();
    std::thread::scope(|s| {
        let jobs: Vec<_> = [other, c]
            .into_iter()
            .map(|c| {
                let g = &g;
                s.spawn(move || (c, column(g, c)))
            })
            .collect();
        for j in jobs {
            let (c, chunks) = j.join().unwrap();
            assert_eq!(chunks, expected[&c]);
        }
    });
    let mut world = World::streaming(voxel_core::StreamIdentity {
        seed,
        generator: version,
    });
    for (p, ch) in expected[&c].clone() {
        world.insert_chunk(p, ch);
    }
    world.evict_column(c);
    assert_eq!(world.chunk_count(), 0);
    assert_eq!(column(&g, c), expected[&c]);
}
