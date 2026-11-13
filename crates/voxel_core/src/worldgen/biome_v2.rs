// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generator12 surface ecology over the exact generator11 solid/water geometry.
//! Original interval-distance climate selection; no external biome tables/code.
use super::{Biome, Generator, biome, hydrology, noise, terrain_v4};
use crate::Block;
const Q: i32 = 1024;

/// Surface climate in signed Q10. C/E/W are the actual v11 terrain fields /4.
/// Depth is zero at this surface; underground biome selection is deferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub temperature: i32,
    pub humidity: i32,
    pub continentalness: i32,
    pub erosion: i32,
    pub weirdness: i32,
    /// Q10 low-pass geographical elevation, interpolated on an absolute32 grid.
    pub elevation: i32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sample {
    pub biome: Biome,
    pub climate: Parameters,
    pub height: i32,
    pub slope: i32,
    pub top: Block,
    pub soil: Block,
}
fn in_bounds(x: i32, z: i32) -> bool {
    (-32768..32768).contains(&x) && (-32768..32768).contains(&z)
}
fn fields(
    seed: u64,
    x: i32,
    z: i32,
    terrain: terrain_v4::Parameters,
    elevation: i32,
) -> Parameters {
    // Broad continuous climate with a weak smaller octave, never biome cells.
    // Rotation/warp avoids an axis-aligned visual grain without moving terrain.
    let wx = x + noise(seed, x, z, 640, 0xc001) * 64 / Q;
    let wz = z + noise(seed, x, z, 640, 0xc002) * 64 / Q;
    let px = (wx * 4 + wz * 3) / 5;
    let pz = (wz * 4 - wx * 3) / 5;
    Parameters {
        temperature: (noise(seed, px, pz, 1536, 0xc003) * 5 + noise(seed, px, pz, 640, 0xc004)) / 6,
        humidity: (noise(seed, px, pz, 1024, 0xc005) * 4 + noise(seed, px, pz, 448, 0xc006)) / 5,
        continentalness: terrain.continentalness / 4,
        erosion: terrain.erosion / 4,
        weirdness: terrain.weirdness / 4,
        elevation,
    }
}
fn elevation(x: i32, z: i32, mut at: impl FnMut(i32, i32) -> i32) -> i32 {
    let cx = x.div_euclid(32) * 32;
    let cz = z.div_euclid(32) * 32;
    let fx = x.rem_euclid(32) * Q / 32;
    let fz = z.rem_euclid(32) * Q / 32;
    let mix = |a, b, t| a + (b - a) * t / Q;
    mix(
        mix(at(cx, cz), at(cx + 32, cz), fx),
        mix(at(cx, cz + 32), at(cx + 32, cz + 32), fx),
        fz,
    )
}
pub(super) fn parameters_with_elevation(
    seed: u64,
    x: i32,
    z: i32,
    at: impl FnMut(i32, i32) -> i32,
) -> Parameters {
    parameters_with_elevation_for(12, seed, x, z, at)
}
pub(super) fn parameters_with_elevation_for(
    version: u32,
    seed: u64,
    x: i32,
    z: i32,
    at: impl FnMut(i32, i32) -> i32,
) -> Parameters {
    fields(
        seed,
        x,
        z,
        super::terrain_v5::parameters_for(version, seed, x, z),
        elevation(x, z, at),
    )
}
pub fn parameters(seed: u64, x: i32, z: i32) -> Option<Parameters> {
    in_bounds(x, z).then(|| {
        parameters_with_elevation(seed, x, z, |x, z| {
            terrain_v4::parameters(seed, x, z).offset / 4
        })
    })
}

fn ramp(v: i32, lo: i32, hi: i32) -> i32 {
    let t = ((v - lo) * Q / (hi - lo)).clamp(0, Q);
    t * t / Q * (3 * Q - 2 * t) / Q
}
impl Parameters {
    /// Continuous regional snowline, in blocks. Warm ranges keep bare summits.
    /// Maritime climate moderates cold; C/E/W perturb by only a few blocks.
    pub fn snowline(self) -> i32 {
        76 + self.temperature * 90 / Q
            + (-self.continentalness).clamp(0, Q) * 5 / Q
            + self.erosion / 160
            - self.weirdness / 160
    }
    pub fn treeline(self) -> i32 {
        self.snowline() - 18
    }
}
/// Original lowland climate rectangles; squared distance to each interval.
/// Gaps become stable, broad intermediate families instead of random mask edges.
fn lowland(c: Parameters) -> Biome {
    const REGIONS: [(Biome, [i32; 2], [i32; 2]); 6] = [
        (Biome::SnowyPlains, [-1024, -510], [-1024, 1024]),
        (Biome::Taiga, [-450, -150], [-60, 1024]),
        (Biome::Plains, [-380, 650], [-300, 20]),
        (Biome::Forest, [-90, 1024], [300, 1024]),
        (Biome::BirchForest, [40, 520], [100, 240]),
        (Biome::Desert, [360, 1024], [-1024, -380]),
    ];
    let distance = |v: i32, [lo, hi]: [i32; 2]| (lo - v).max(0).max(v - hi);
    REGIONS
        .into_iter()
        .filter(|(biome, _, _)| match biome {
            Biome::Desert => c.temperature >= 200,
            Biome::SnowyPlains => c.temperature <= -420,
            _ => true,
        })
        .min_by_key(|(_, t, h)| {
            let dt = i64::from(distance(c.temperature, *t));
            let dh = i64::from(distance(c.humidity, *h));
            dt * dt * 2 + dh * dh
        })
        .unwrap()
        .0
}
fn terrestrial(c: Parameters, h: i32) -> Biome {
    let family = lowland(c);
    // Density recesses/rock steps do not create a separate ecosystem per ledge.
    // Keep a quarter of actual height and cap the proxy over projected dry banks.
    let h = ((c.elevation * 3 + h * Q) / 4).min((h + 8) * Q);
    // Elevation is the actual final surface, never a biome-specific height.
    // Terrain erosion/weirdness allow elevated shoulders as well as summits.
    let mountain = h >= 38 * Q && (c.erosion < 220 || c.weirdness > 160 || h >= 64 * Q);
    if mountain {
        if h >= c.snowline() * Q {
            return Biome::SnowySlopes;
        }
        if h >= 94 * Q || (h >= 78 * Q && c.humidity < -220) {
            return Biome::StonyPeaks;
        }
        if h >= (c.treeline() - 20) * Q && c.humidity > -100 && c.temperature < 330 {
            return if h < c.treeline() * Q {
                Biome::Grove
            } else {
                Biome::RockyHighlands
            };
        }
        if h >= c.treeline() * Q {
            return Biome::RockyHighlands;
        }
        if c.humidity > -260 && family != Biome::Desert {
            return Biome::Meadow;
        }
        return Biome::RockyHighlands;
    }
    family
}
fn ocean_near_with(g: &Generator, x: i32, z: i32, mut height: impl FnMut(i32, i32) -> i32) -> bool {
    // Real marine water within eight blocks (eleven on diagonals), not a
    // continentalness guess. Only queried for low, gentle, dry shore candidates.
    [
        [-8, 0],
        [8, 0],
        [0, -8],
        [0, 8],
        [-6, -6],
        [6, -6],
        [-6, 6],
        [6, 6],
    ]
    .into_iter()
    .any(|[dx, dz]| {
        let (nx, nz) = (x + dx, z + dz);
        if !in_bounds(nx, nz) {
            return false;
        }
        // V11 hydrology starts Ocean iff the unmodified density top is below
        // sea level. Basins/channels only lower beds and preserve that identity;
        // they never turn dry land into Ocean. This exact predicate avoids
        // rebuilding unrelated lake/river geography for each shore probe.
        if height(nx, nz) >= hydrology::SEA_LEVEL {
            return false;
        }
        // The protected starting shore can raise an originally marine bed.
        // Preserve its exact final-depth test, including basin/channel lowering.
        if nx.abs().max((nz - 10).abs()) < 40 {
            let s = hydrology::surface_for(g, nx, nz);
            return s.depth() > 0 && s.body == hydrology::WaterBody::Ocean;
        }
        true
    })
}
fn ocean_near(g: &Generator, x: i32, z: i32) -> bool {
    ocean_near_with(g, x, z, |x, z| {
        super::terrain_v5::height_for(g.config.version, g.config.seed, x, z)
    })
}
/// Reuse the already-computed unmodified20² density tops for shore probes.
/// Only the eight-block exterior stencil needs additional memoized point tops.
/// This cache is discarded after classifying one18² tile, never retained globally.
pub(super) struct Coast<'a> {
    min: [i32; 2],
    rim: &'a [i32],
    extra: std::collections::BTreeMap<[i32; 2], i32>,
}
impl<'a> Coast<'a> {
    pub(super) fn new(min: [i32; 2], rim: &'a [i32]) -> Self {
        debug_assert_eq!(rim.len(), 400);
        Self {
            min: min.map(|v| v - 1),
            rim,
            extra: Default::default(),
        }
    }
    pub(super) fn near(&mut self, g: &Generator, x: i32, z: i32) -> bool {
        ocean_near_with(g, x, z, |px, pz| {
            let dx = px - self.min[0];
            let dz = pz - self.min[1];
            if (0..20).contains(&dx) && (0..20).contains(&dz) {
                self.rim[(dz * 20 + dx) as usize]
            } else {
                *self.extra.entry([px, pz]).or_insert_with(|| {
                    super::terrain_v5::height_for(g.config.version, g.config.seed, px, pz)
                })
            }
        })
    }
}
pub(super) fn evaluate(
    g: &Generator,
    x: i32,
    z: i32,
    c: Parameters,
    water: hydrology::Surface,
    slope: i32,
) -> Sample {
    evaluate_with_coast(c, water, slope, || ocean_near(g, x, z))
}
pub(super) fn evaluate_with_coast(
    c: Parameters,
    water: hydrology::Surface,
    slope: i32,
    coastal: impl FnOnce() -> bool,
) -> Sample {
    let h = water.bed;
    let biome = if water.depth() > 0 {
        match water.body {
            hydrology::WaterBody::Ocean => Biome::Ocean,
            hydrology::WaterBody::River => Biome::River,
            hydrology::WaterBody::Lake | hydrology::WaterBody::Pond => Biome::Lake,
            hydrology::WaterBody::Dry => terrestrial(c, h),
        }
    } else if (hydrology::SEA_LEVEL + 1..=hydrology::SEA_LEVEL + 3).contains(&h)
        && water.coastal
        && slope <= 1
        && coastal()
    {
        Biome::Beach
    } else {
        terrestrial(c, h)
    };
    let top = if slope >= 3 || biome == Biome::StonyPeaks {
        Block::Stone
    } else if biome == Biome::SnowySlopes
        || biome == Biome::SnowyPlains
        || (matches!(biome, Biome::Grove | Biome::RockyHighlands) && h >= c.snowline() - 28)
    {
        Block::SnowyGround
    } else if biome == Biome::RockyHighlands {
        Block::Stone
    } else if biome == Biome::Beach || (biome == Biome::Desert && h < 38) {
        Block::Sand
    } else if water.depth() > 0 {
        if water.body == hydrology::WaterBody::Ocean {
            Block::Sand
        } else {
            Block::Dirt
        }
    } else if h >= 52 && slope >= 2 {
        Block::Stone
    } else {
        Block::Grass
    };
    Sample {
        biome,
        climate: c,
        height: h,
        slope,
        top,
        soil: match top {
            Block::Sand => Block::Sand,
            Block::Stone => Block::Stone,
            _ => Block::Dirt,
        },
    }
}
impl Sample {
    pub(super) fn column(self) -> biome::Column {
        biome::Column {
            height: self.height,
            climate: biome::Climate {
                temperature: self.climate.temperature,
                moisture: self.climate.humidity,
                geography: self.climate.continentalness,
            },
            top: self.top,
            soil: self.soil,
        }
    }
}
pub fn sample(g: &Generator, x: i32, z: i32) -> Option<Sample> {
    if !matches!(g.config().version, 12 | 13) || !in_bounds(x, z) {
        return None;
    }
    let c = parameters_with_elevation_for(g.config.version, g.config.seed, x, z, |px, pz| {
        super::terrain_v5::parameters_for(g.config.version, g.config.seed, px, pz).offset / 4
    });
    let water = hydrology::surface_for(g, x, z);
    let slope = [[-1, 0], [1, 0], [0, -1], [0, 1]]
        .into_iter()
        .map(|[dx, dz]| (hydrology::surface_for(g, x + dx, z + dz).bed - water.bed).abs())
        .max()
        .unwrap();
    Some(evaluate(g, x, z, c, water, slope))
}
/// Reuse the v11 profile parameter evaluation; no old climate sampling in v12.
pub(super) fn columns(
    seed: u64,
    version: u32,
    min: [i32; 2],
) -> (
    Vec<terrain_v4::Profile>,
    Vec<biome::Column>,
    Vec<i32>,
    Vec<Parameters>,
) {
    let (grid, terrain) = terrain_v4::profiles_with(seed, min.map(|v| v - 1), 20, |seed, x, z| {
        super::terrain_v5::parameters_for(version, seed, x, z)
    });
    let origin = min.map(|v| v.div_euclid(32) * 32);
    let nx = (min[0] + 17 - origin[0]) / 32 + 2;
    let nz = (min[1] + 17 - origin[1]) / 32 + 2;
    let heights: Vec<_> = (0..nx * nz)
        .map(|i| {
            super::terrain_v5::parameters_for(
                version,
                seed,
                origin[0] + (i % nx) * 32,
                origin[1] + (i / nx) * 32,
            )
            .offset
                / 4
        })
        .collect();
    let mut profiles = Vec::with_capacity(324);
    let mut materials = Vec::with_capacity(324);
    let mut climate = Vec::with_capacity(324);
    for z in 1..19 {
        for x in 1..19 {
            let j = z * 20 + x;
            let c = fields(
                seed,
                min[0] + x as i32 - 1,
                min[1] + z as i32 - 1,
                terrain[j],
                elevation(min[0] + x as i32 - 1, min[1] + z as i32 - 1, |px, pz| {
                    heights[(((pz - origin[1]) / 32) * nx + (px - origin[0]) / 32) as usize]
                }),
            );
            profiles.push(grid[j]);
            climate.push(c);
            materials.push(biome::Column {
                height: grid[j].top,
                climate: biome::Climate {
                    temperature: c.temperature,
                    moisture: c.humidity,
                    geography: c.continentalness,
                },
                top: Block::Grass,
                soil: Block::Dirt,
            });
        }
    }
    (
        profiles,
        materials,
        grid.into_iter().map(|p| p.top).collect(),
        climate,
    )
}
/// Tree acceptance per jittered candidate, per mille. Density is continuous
/// through compatible forest families; elevation smoothly removes upper trees.
pub(super) fn tree_density(seed: u64, x: i32, z: i32, s: Sample) -> u64 {
    let base = match s.biome {
        Biome::Forest | Biome::BirchForest | Biome::Taiga | Biome::Plains => {
            85 + ramp(s.climate.humidity, -20, 280) * 695 / Q
        }
        Biome::Grove => 850,
        Biome::Meadow => 65,

        Biome::SnowyPlains => 35,
        _ => 0,
    };
    let moisture = 600 + ramp(s.climate.humidity, -400, 240) * 424 / Q;
    let clearing = (650 + noise(seed, x, z, 80, 0xc010) * 650 / Q).clamp(80, Q);
    let elevation = Q - ramp(s.height, s.climate.treeline() - 4, s.climate.treeline() + 4);
    (base * moisture / Q * clearing / Q * elevation / Q) as u64
}
pub(super) fn plant(seed: u64, x: i32, z: i32, s: Sample) -> Option<Block> {
    let h = super::hash(seed, x, z, 0xc020);
    let patch = noise(seed, x, z, 24, 0xc021);
    let flower = noise(seed, x, z, 40, 0xc022);
    let (chance, ferns, flowers) = match s.biome {
        Biome::Desert => return (patch > 0 && h % 1000 < 18).then_some(Block::DeadBush),
        Biome::Meadow => (190, 5, 65),
        Biome::Plains => (95, 5, 18),
        Biome::Forest => (125, 40, 15),
        Biome::BirchForest => (110, 20, 20),
        Biome::Taiga | Biome::Grove => (140, 85, 2),
        Biome::SnowyPlains => return (patch > 200 && h % 1000 < 10).then_some(Block::Fern),
        _ => return None,
    };
    if h % 1000 >= (chance + patch / 5).clamp(0, 340) as u64 {
        return None;
    }
    Some(if flower > 0 && (h >> 16) % 100 < flowers {
        if noise(seed, x, z, 64, 0xc023) > 0 {
            Block::RedFlower
        } else {
            Block::YellowFlower
        }
    } else if (h >> 24) % 100 < ferns {
        Block::Fern
    } else {
        Block::TallGrass
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn climate(t: i32, h: i32) -> Parameters {
        Parameters {
            temperature: t,
            humidity: h,
            continentalness: 200,
            erosion: -200,
            weirdness: 250,
            elevation: 0,
        }
    }
    fn at(c: Parameters, h: i32) -> Biome {
        terrestrial(
            Parameters {
                elevation: h * Q,
                ..c
            },
            h,
        )
    }
    #[test]
    fn optimized_shore_proof_matches_actual_hydrology_including_world_edges() {
        let mut marine = 0;
        let mut inland = 0;
        for seed in [0, 42, 2026, u64::MAX] {
            let g = Generator::new(super::super::GenerationConfig { seed, version: 12 }).unwrap();
            for x in [-32768, -24000, -4096, -39, -12, -1, 0, 12, 39, 3328, 32760] {
                for z in [-32768, -3264, -29, 0, 10, 22, 49, 20000, 32760] {
                    let actual = [
                        [-8, 0],
                        [8, 0],
                        [0, -8],
                        [0, 8],
                        [-6, -6],
                        [6, -6],
                        [-6, 6],
                        [6, 6],
                    ]
                    .into_iter()
                    .any(|[dx, dz]| {
                        if !in_bounds(x + dx, z + dz) {
                            return false;
                        }
                        let s = hydrology::surface_for(&g, x + dx, z + dz);
                        s.depth() > 0 && s.body == hydrology::WaterBody::Ocean
                    });
                    assert_eq!(ocean_near(&g, x, z), actual, "{seed} {x} {z}");
                    if actual {
                        marine += 1;
                    } else {
                        inland += 1;
                    }
                }
            }
            for chunk in [[208, -204], [-1, 0], [-2048, 2047]] {
                let min = chunk.map(|v| v * 16 - 1);
                let (_, _, rim, _) = columns(seed, 12, min);
                let mut coast = Coast::new(min, &rim);
                for i in 0..324 {
                    let x = min[0] + i % 18;
                    let z = min[1] + i / 18;
                    assert_eq!(coast.near(&g, x, z), ocean_near(&g, x, z));
                }
                assert!(coast.extra.len() <= 756);
            }
        }
        assert!(marine > 0 && inland > 0);
    }
    #[test]
    fn classified_water_matches_real_basins_channels_and_coast() {
        let mut observed = std::collections::BTreeSet::new();
        for (seed, x, z) in [(42, 3328, -3264), (u64::MAX, -4032, 2112), (0, -1856, 592)] {
            let g = Generator::new(super::super::GenerationConfig { seed, version: 12 }).unwrap();
            for dz in (-24..=24).step_by(4) {
                for dx in (-24..=24).step_by(4) {
                    let (px, pz) = (x + dx, z + dz);
                    let water = hydrology::surface_for(&g, px, pz);
                    let s = sample(&g, px, pz).unwrap();
                    if water.depth() > 0 {
                        let expected = match water.body {
                            hydrology::WaterBody::Ocean => Biome::Ocean,
                            hydrology::WaterBody::River => Biome::River,
                            hydrology::WaterBody::Lake | hydrology::WaterBody::Pond => Biome::Lake,
                            hydrology::WaterBody::Dry => panic!("wet dry geography"),
                        };
                        assert_eq!(s.biome, expected);
                        assert_eq!(
                            g.block(crate::BlockPos([px, hydrology::SEA_LEVEL, pz])),
                            Block::WaterSource
                        );
                        observed.insert(expected as u8);
                    } else {
                        assert!(!matches!(
                            s.biome,
                            Biome::Ocean | Biome::River | Biome::Lake
                        ));
                        if s.biome == Biome::Beach {
                            assert!((3..=5).contains(&water.bed) && s.slope <= 1 && water.coastal);
                            assert_eq!(s.top, Block::Sand);
                            observed.insert(Biome::Beach as u8);
                        }
                    }
                }
            }
        }
        assert_eq!(
            observed,
            [Biome::Beach, Biome::Ocean, Biome::River, Biome::Lake]
                .map(|b| b as u8)
                .into()
        );
    }
    #[test]
    fn altitude_zones_follow_temperature_moisture_and_actual_relief() {
        let c = climate(100, 180);
        assert_eq!(at(c, 20), Biome::BirchForest);
        assert_eq!(at(c, 40), Biome::Meadow);
        assert_eq!(at(c, 55), Biome::Grove);
        assert_eq!(at(c, 100), Biome::SnowySlopes);
        let warm = climate(600, 180);
        assert_eq!(at(warm, 100), Biome::StonyPeaks);
        assert!(warm.snowline() > c.snowline() + 30);
        assert_eq!(at(climate(600, -700), 55), Biome::RockyHighlands);
        assert_eq!(at(climate(-600, -700), 20), Biome::SnowyPlains);
        assert_eq!(at(climate(-280, 400), 20), Biome::Taiga);
        assert_eq!(at(climate(300, 500), 20), Biome::Forest);
        assert_eq!(at(climate(600, -700), 20), Biome::Desert);
    }
    #[test]
    fn hot_dry_to_cold_always_crosses_an_intermediate_family() {
        for humidity in (-1024..=1024).step_by(32) {
            let mut previous = Biome::Plains;
            for temperature in -1024..=1024 {
                let b = lowland(climate(temperature, humidity));
                assert!(!matches!(
                    (previous, b),
                    (Biome::Desert, Biome::SnowyPlains) | (Biome::SnowyPlains, Biome::Desert)
                ));
                previous = b;
            }
        }
    }
    #[test]
    fn steep_cliffs_and_upper_dry_ranges_never_use_grass_or_sand() {
        let g = Generator::new(super::super::GenerationConfig {
            seed: 42,
            version: 12,
        })
        .unwrap();
        for h in 40..150 {
            for t in [-600, 0, 600] {
                let water = hydrology::Surface {
                    bed: h,
                    body: hydrology::WaterBody::Dry,
                    coastal: false,
                };
                let s = evaluate(
                    &g,
                    100,
                    100,
                    Parameters {
                        elevation: h * Q,
                        ..climate(t, -700)
                    },
                    water,
                    3,
                );
                assert_eq!(s.top, Block::Stone);
                let s = evaluate(
                    &g,
                    100,
                    100,
                    Parameters {
                        elevation: h * Q,
                        ..climate(t, -700)
                    },
                    water,
                    0,
                );
                assert_ne!(s.top, Block::Sand);
                assert_eq!(tree_density(42, 100, 100, s), 0);
            }
        }
    }
}
