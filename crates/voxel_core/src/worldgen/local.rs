//! Local v3/v4 height/tree plans. All output changes are explicitly version-gated.
use super::*;
pub(super) fn height(seed: u64, x: i32, z: i32) -> i32 {
    5 + (noise(seed, x, z, 32, 11) * 7
        + noise(seed, x, z, 16, 23) * 4
        + noise(seed, x, z, 8, 37) * 2)
        / 1024
}
fn surface(g: &Generator, x: i32, z: i32) -> i32 {
    if g.config.version == 8 {
        super::hydrology::surface(g.config.seed, x, z).bed
    } else if g.config.version >= 5 {
        super::biome::height(g.config.seed, x, z)
    } else if g.config.version == 4 {
        super::terrain::height(g.config.seed, x, z)
    } else {
        height(g.config.seed, x, z)
    }
}
fn cave(g: &Generator, x: i32, y: i32, z: i32) -> bool {
    if y <= -14 || (x.abs() <= 3 && (8..=13).contains(&z)) {
        return false;
    }
    let base_z = (z + 64).div_euclid(128) * 128;
    let center = base_z - 10 + noise(g.config.seed, x, base_z, 16, 53) * 3 / 1024;
    let radius = if x.rem_euclid(19) < 5 { 5_i64 } else { 3 };
    let dz = i64::from(z - center);
    let dy = i64::from(y + 4);
    let main = dz * dz * 9 + dy * dy * radius * radius <= radius * radius * 9;
    let entrance_x = (x + 74).div_euclid(128) * 128 - 10;
    let local_z = z - base_z;
    if (x - entrance_x).abs() > 1 || !(-16..=22).contains(&local_z) {
        return main;
    }
    let top = surface(g, entrance_x, base_z + 20) + 1;
    let floor = top - ((20 - local_z).clamp(0, 36) * (top + 6) / 32).min(top + 6);
    main || (y >= floor && y < floor + 4)
}
pub(super) fn terrain(g: &Generator, p: BlockPos, h: i32) -> Block {
    if g.config.version == 8 {
        let water = super::hydrology::surface(g.config.seed, p.0[0], p.0[2]);
        if water.depth() > 0 {
            return Block::Stone;
        } // tree admission separately rejects wet roots
        if super::caves::point(g.config.seed, p, h) {
            return Block::Air;
        }
        return layers(g, p, h, None);
    }
    if g.config.version == 7 && super::caves::point(g.config.seed, p, h) {
        Block::Air
    } else {
        layers(g, p, h, None)
    }
}
fn layers(g: &Generator, p: BlockPos, h: i32, column: Option<super::biome::Column>) -> Block {
    let [x, y, z] = p.0;
    if g.config.version == 6
        && g.world_height().contains(y)
        && (-g.extent..g.extent).contains(&x)
        && (-g.extent..g.extent).contains(&z)
    {
        if y < crate::bounds::WorldHeight::HISTORICAL.min {
            return Block::Stone;
        }
        if y >= crate::bounds::WorldHeight::HISTORICAL.max {
            return Block::Air;
        }
    }
    if !(-g.extent..g.extent).contains(&x)
        || !(-g.extent..g.extent).contains(&z)
        || !g.world_height().contains(y)
        || y > h
        || (g.config.version < 7 && cave(g, x, y, z))
    {
        return Block::Air;
    }
    if g.config.version >= 5 {
        let c = column.unwrap_or_else(|| super::biome::column(g.config.seed, x, z));
        return if y == h {
            c.top
        } else if y >= h - 4 && c.soil == Block::Sand {
            Block::Sand
        } else if y >= h - 3 {
            c.soil
        } else {
            Block::Stone
        };
    }
    if y == h {
        Block::Grass
    } else if y >= h - 3 {
        Block::Dirt
    } else {
        Block::Stone
    }
}
fn trees(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Vec<Tree> {
    let mut result = Vec::new();
    for gz in (min[1] - 3).div_euclid(8)..=(max[1] + 3).div_euclid(8) {
        for gx in (min[0] - 3).div_euclid(8)..=(max[0] + 3).div_euclid(8) {
            let h = hash(g.config.seed, gx, gz, 71);
            if g.config.version < 5 && h.is_multiple_of(5) {
                continue;
            }
            let (x, z, h) = if g.config.version >= 5 {
                super::biome::candidate(g.config.seed, gx, gz)
            } else {
                (
                    gx * 8 + (h % 3) as i32 - 1,
                    gz * 8 + ((h >> 8) % 3) as i32 - 1,
                    h,
                )
            };
            if x < min[0] - 2
                || x > max[0] + 2
                || z < min[1] - 2
                || z > max[1] + 2
                || x < -g.extent + 2
                || x >= g.extent - 2
                || z < -g.extent + 2
                || z >= g.extent - 2
                || (x.abs() <= 4 && (z - 10).abs() <= 4)
            {
                continue;
            }
            if g.config.version >= 5
                && (h >> 24) % 1000 >= super::biome::tree_density(g.config.seed, x, z)
            {
                continue;
            }
            let ground = surface(g, x, z);
            if g.config.version == 8 && ground <= super::hydrology::SEA_LEVEL {
                continue;
            }
            let trunk = 4 + ((h >> 16) % 2) as i32;
            // v4 keeps the established species/grid, with sparse roots on gentle
            // ground and enough vertical/canopy clearance. v3 stays frozen.
            if g.config.version >= 4
                && ((g.config.version == 4 && h.is_multiple_of(2))
                    || ground + trunk + 1 > 30
                    || !(-2..=2).all(|dx| {
                        (-2..=2).all(|dz| (surface(g, x + dx, z + dz) - ground).abs() <= 2)
                    }))
            {
                continue;
            }
            let root = terrain(g, BlockPos([x, ground, z]), ground);
            if root != Block::Grass && !(g.config.version >= 5 && root == Block::SnowyGround) {
                continue;
            }
            if (-2..=2).all(|dx| (-2..=2).all(|dz| surface(g, x + dx, z + dz) < ground + trunk - 1))
            {
                result.push(Tree {
                    x,
                    z,
                    ground,
                    trunk,
                });
            }
        }
    }
    result
}
fn sample(
    g: &Generator,
    p: BlockPos,
    h: i32,
    trees: &[Tree],
    column: Option<super::biome::Column>,
) -> Block {
    let [x, y, z] = p.0;
    let b = layers(g, p, h, column);
    if b != Block::Air
        || y <= h
        || !g.world_height().contains(y)
        || !(-g.extent..g.extent).contains(&x)
        || !(-g.extent..g.extent).contains(&z)
    {
        return b;
    }
    if trees
        .iter()
        .any(|t| x == t.x && z == t.z && (1..=t.trunk).contains(&(y - t.ground)))
    {
        return Block::Wood;
    }
    if trees.iter().any(|t| {
        let dx = (x - t.x).abs();
        let dz = (z - t.z).abs();
        let dy = y - t.ground - t.trunk;
        (-1..=1).contains(&dy) && dx <= 2 && dz <= 2 && dx + dz <= if dy == 1 { 2 } else { 3 }
    }) {
        Block::Leaves
    } else {
        Block::Air
    }
}
pub(super) fn point(g: &Generator, p: BlockPos) -> Block {
    let [x, y, z] = p.0;
    if g.config.version == 8 {
        if !g.world_height().contains(y)
            || !(-g.extent..g.extent).contains(&x)
            || !(-g.extent..g.extent).contains(&z)
        {
            return Block::Air;
        }
        return Plan::new(g, [x.div_euclid(16), z.div_euclid(16)]).block(g, p);
    }
    if g.config.version == 6
        && g.world_height().contains(y)
        && (-g.extent..g.extent).contains(&x)
        && (-g.extent..g.extent).contains(&z)
    {
        if y < crate::bounds::WorldHeight::HISTORICAL.min {
            return Block::Stone;
        }
        if y >= crate::bounds::WorldHeight::HISTORICAL.max {
            return Block::Air;
        }
    }
    if !(-g.extent..g.extent).contains(&x)
        || !(-g.extent..g.extent).contains(&z)
        || !g.world_height().contains(y)
    {
        return Block::Air;
    }
    let h = surface(g, x, z);
    let column = (g.config.version >= 5).then(|| super::biome::column(g.config.seed, x, z));
    let b = layers(g, p, h, column);
    if g.config.version == 7 && y <= h && super::caves::point(g.config.seed, p, h) {
        return Block::Air;
    }
    // A v4 root may sit two blocks above this cell's surface. A five-block
    // trunk plus the crown then reaches h+8. Keep the frozen v3 shortcut.
    let overhead = if g.config.version >= 4 { 8 } else { 7 };
    if b != Block::Air || y <= h || y > h + overhead {
        return b;
    }
    sample(g, p, h, &trees(g, [x, z], [x, z]), column)
}
pub(super) struct Plan {
    min: [i32; 2],
    heights: Vec<i32>,
    materials: Vec<super::biome::Column>,
    trees: Vec<Tree>,
    caves: Option<super::caves::Plan>,
    water: Option<super::hydrology::Plan>,
}
impl Plan {
    fn new(g: &Generator, c: [i32; 2]) -> Self {
        let min = [c[0] * 16 - 1, c[1] * 16 - 1];
        let max = [min[0] + 17, min[1] + 17];
        let materials = if g.config.version >= 5 {
            super::biome::columns(g.config.seed, min)
        } else {
            Vec::new()
        };
        let water =
            (g.config.version == 8).then(|| super::hydrology::Plan::new(g, min, max, &materials));
        let mut heights = Vec::with_capacity(18 * 18);
        for z in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                heights.push(if let Some(w) = &water {
                    w.columns[heights.len()].bed
                } else if g.config.version >= 5 {
                    materials[heights.len()].height
                } else {
                    surface(g, x, z)
                });
            }
        }
        Self {
            min,
            heights,
            materials,
            trees: trees(g, min, max),
            water,
            caves: (g.config.version >= 7)
                .then(|| super::caves::Plan::new(g.config.seed, min, max)),
        }
    }
    fn water_material(
        &self,
        px: i32,
        pz: i32,
        h: i32,
        column: super::hydrology::Surface,
    ) -> super::biome::Column {
        let mut material = self.materials[((pz - self.min[1]) * 18 + px - self.min[0]) as usize];
        if column.depth() > 0 {
            material.top = if material.top == Block::Grass {
                Block::Dirt
            } else {
                material.top
            };
        }
        if h <= super::hydrology::SEA_LEVEL + 2
            && column.coastal
            && material.top != Block::SnowyGround
            && material.top != Block::Stone
        {
            material.top = Block::Sand;
            material.soil = Block::Sand;
        }
        material
    }
    fn water_chunk(&self, g: &Generator, p: ChunkPos) -> Chunk {
        let water = self.water.as_ref().unwrap();
        // Every voxel is above its bed and at/below the common water datum;
        // caves cannot carve above the bed. Keep homogeneous source slabs
        // allocation-free instead of materializing and compacting 4096 cells.
        if p.0[1] * 16 > *self.heights.iter().max().unwrap()
            && p.0[1] * 16 + 15 <= super::hydrology::SEA_LEVEL
        {
            return Chunk::filled(Block::WaterSource);
        }
        let mut c = if p.0[1] * 16 + 15 < *self.heights.iter().min().unwrap() - 4 {
            Chunk::filled(Block::Stone)
        } else {
            Chunk::default()
        };
        if c.uniform() != Some(Block::Stone) {
            for z in 0..16 {
                for x in 0..16 {
                    let px = p.0[0] * 16 + x as i32;
                    let pz = p.0[2] * 16 + z as i32;
                    let column = water.column(px, pz);
                    let h = column.bed;
                    let material = self.water_material(px, pz, h, column);
                    for y in 0..16 {
                        let l = LocalPos::new([x, y, z]).unwrap();
                        let pos = p.block_position(l).unwrap();
                        let b = if pos.0[1] > h && pos.0[1] <= super::hydrology::SEA_LEVEL {
                            Block::WaterSource
                        } else {
                            sample(g, pos, h, &self.trees, Some(material))
                        };
                        c.set(l, b);
                    }
                }
            }
        }
        self.caves.as_ref().unwrap().carve_if(
            &mut c,
            p,
            |x, z| water.column(x, z).bed,
            |pos, h| {
                !(water.sealed(pos.0[0], pos.0[2])
                    && pos.0[1] >= h.min(super::hydrology::SEA_LEVEL) - 4)
            },
        );
        water.fill_pools(&mut c, p);
        c.compact();
        c
    }
    fn block(&self, g: &Generator, p: BlockPos) -> Block {
        let Some(x) = p.0[0].checked_sub(self.min[0]) else {
            return point(g, p);
        };
        let Some(z) = p.0[2].checked_sub(self.min[1]) else {
            return point(g, p);
        };
        let h = if (0..18).contains(&x) && (0..18).contains(&z) {
            self.heights[(z * 18 + x) as usize]
        } else {
            return point(g, p);
        };
        if let Some(water) = &self.water {
            let [px, y, pz] = p.0;
            if !g.world_height().contains(y)
                || !(-g.extent..g.extent).contains(&px)
                || !(-g.extent..g.extent).contains(&pz)
            {
                return Block::Air;
            }
            let column = water.column(px, pz);
            if y > h && y <= super::hydrology::SEA_LEVEL {
                return Block::WaterSource;
            }
            // A four-block watertight bed/bank cap suppresses cave breaches in
            // low shores. The primitive cave shapes themselves remain frozen.
            let sealed = water.sealed(px, pz) && y >= h.min(super::hydrology::SEA_LEVEL) - 4;
            if y <= h && !sealed && self.caves.as_ref().unwrap().contains(p, h) {
                return if water.cave_water(p) {
                    Block::WaterSource
                } else {
                    Block::Air
                };
            }
            let material = self.water_material(px, pz, h, column);
            return sample(g, p, h, &self.trees, Some(material));
        }
        if p.0[1] <= h && self.caves.as_ref().is_some_and(|c| c.contains(p, h)) {
            return Block::Air;
        }
        sample(
            g,
            p,
            h,
            &self.trees,
            self.materials.get((z * 18 + x) as usize).copied(),
        )
    }
}
pub struct Sampler<'a> {
    g: &'a Generator,
    plan: Option<Plan>,
}
impl<'a> Sampler<'a> {
    pub(super) fn new(g: &'a Generator, c: [i32; 2]) -> Self {
        Self {
            g,
            plan: (g.config.version >= 3
                && c.iter()
                    .all(|v| (-g.chunk_limit()..g.chunk_limit()).contains(v)))
            .then(|| Plan::new(g, c)),
        }
    }
    pub fn block(&self, p: BlockPos) -> Block {
        self.plan
            .as_ref()
            .map_or_else(|| self.g.block(p), |plan| plan.block(self.g, p))
    }
    pub fn chunk(&self, p: ChunkPos) -> Option<Chunk> {
        let limit = self.g.chunk_limit();
        if !(-limit..limit).contains(&p.0[0])
            || !(-limit..limit).contains(&p.0[2])
            || !self.g.world_height().chunks().contains(&p.0[1])
        {
            return None;
        }
        // V6 freezes v5 surface/caves/trees; deeper slabs are solid Stone and
        // the old upper boundary remains empty. These proofs avoid 13 voxel scans.
        if self.g.config.version == 6 {
            if p.0[1] < crate::bounds::WorldHeight::HISTORICAL.chunks().start {
                return Some(Chunk::filled(Block::Stone));
            }
            if p.0[1] >= crate::bounds::WorldHeight::HISTORICAL.chunks().end {
                return Some(Chunk::default());
            }
        }
        if self.g.config.version == 8 && p.0[1] >= 2 {
            return Some(Chunk::default());
        }
        if self.g.config.version == 8
            && let Some(plan) = &self.plan
            && p.0[0] * 16 == plan.min[0] + 1
            && p.0[2] * 16 == plan.min[1] + 1
        {
            return Some(plan.water_chunk(self.g, p));
        }
        if self.g.config.version == 7 {
            if p.0[1] >= 2 {
                return Some(Chunk::default());
            }
            if let Some(plan) = &self.plan {
                // A sampler may be asked for a different column; never apply
                // this column's conservative bounds to that request.
                if p.0[0] * 16 == plan.min[0] + 1 && p.0[2] * 16 == plan.min[1] + 1 {
                    let mut c = if p.0[1] * 16 + 15 < *plan.heights.iter().min().unwrap() - 4 {
                        Chunk::filled(Block::Stone)
                    } else {
                        let mut c = Chunk::default();
                        for y in 0..16 {
                            for z in 0..16 {
                                for x in 0..16 {
                                    let l = LocalPos::new([x, y, z]).unwrap();
                                    let pos = p.block_position(l).unwrap();
                                    let i = (z + 1) * 18 + x + 1;
                                    c.set(
                                        l,
                                        sample(
                                            self.g,
                                            pos,
                                            plan.heights[i],
                                            &plan.trees,
                                            plan.materials.get(i).copied(),
                                        ),
                                    );
                                }
                            }
                        }
                        c
                    };
                    plan.caves.as_ref().unwrap().carve(&mut c, p, |x, z| {
                        plan.heights[((z - plan.min[1]) * 18 + x - plan.min[0]) as usize]
                    });
                    return Some(c);
                }
            }
        }
        let mut c = Chunk::default();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let local = LocalPos::new([x, y, z]).unwrap();
                    c.set(local, self.block(p.block_position(local).unwrap()));
                }
            }
        }
        c.compact();
        Some(c)
    }
}
