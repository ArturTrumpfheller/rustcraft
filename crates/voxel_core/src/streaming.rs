//! Bounded, engine-independent streaming policy and worker payloads.
use crate::regions::EditSnapshot;
use crate::{
    BlockPos, Chunk, ChunkMesh, ChunkPos, StreamIdentity, World,
    worldgen::{GenerationConfig, Generator},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};
pub const DEFAULT_RADIUS: i32 = 2;
pub const MAX_RADIUS: i32 = 3;
pub type Column = [i32; 2];
pub fn desired(center: Column, radius: i32) -> BTreeSet<Column> {
    desired_with_limit(center, radius, 4)
}
pub fn desired_with_limit(center: Column, radius: i32, limit: i32) -> BTreeSet<Column> {
    let r = radius.clamp(1, MAX_RADIUS);
    let center = center.map(|v| v.clamp(-limit, limit - 1));
    let mut result = BTreeSet::new();
    for z in (center[1] - r).max(-limit)..=(center[1] + r).min(limit - 1) {
        for x in (center[0] - r).max(-limit)..=(center[0] + r).min(limit - 1) {
            result.insert([x, z]);
        }
    }
    result
}
pub fn nearest(columns: &BTreeSet<Column>, center: Column) -> Option<Column> {
    columns
        .iter()
        .min_by_key(|p| {
            let dx = i128::from(p[0]) - i128::from(center[0]);
            let dz = i128::from(p[1]) - i128::from(center[1]);
            (dx * dx + dz * dz, **p)
        })
        .copied()
}
#[derive(Default)]
pub struct LightMetrics {
    pub cells: usize,
    pub propagated: usize,
    pub generated_columns: usize,
}
#[derive(Default)]
pub struct TerrainCache {
    identity: Option<GenerationConfig>,
    columns: BTreeMap<Column, Vec<Chunk>>,
}
impl TerrainCache {
    pub fn retain(&mut self, desired: &BTreeSet<Column>) {
        self.columns.retain(|c, _| {
            desired
                .iter()
                .any(|d| (c[0] - d[0]).abs() <= 1 && (c[1] - d[1]).abs() <= 1)
        });
    }
    pub fn allocated_voxel_bytes(&self) -> usize {
        self.columns
            .values()
            .flatten()
            .map(Chunk::allocated_bytes)
            .sum()
    }
    pub fn columns(&self) -> usize {
        self.columns.len()
    }
}
pub fn lighting_regions(targets: &BTreeSet<ChunkPos>) -> BTreeSet<crate::regions::RegionPos> {
    crate::lighting::source_columns(targets)
        .into_iter()
        .map(|c| crate::regions::RegionPos([c[0].div_euclid(4), c[1].div_euclid(4)]))
        .filter(|p| p.valid())
        .collect()
}
pub struct ColumnResult {
    pub column: Column,
    pub revision: u64,
    pub chunks: Vec<(ChunkPos, Chunk, ChunkMesh)>,
    pub lights: BTreeMap<ChunkPos, crate::lighting::ChunkLight>,
    pub lighting: LightMetrics,
    pub elapsed: Duration,
}
pub fn build_column(
    generator: &Generator,
    edits: impl Into<EditSnapshot>,
    column: Column,
    revision: u64,
) -> ColumnResult {
    build_column_cached(
        generator,
        World::default(),
        edits.into(),
        column,
        revision,
        &mut TerrainCache::default(),
    )
}
pub fn build_column_cached(
    generator: &Generator,
    snapshot: World,
    edits: EditSnapshot,
    column: Column,
    revision: u64,
    cache: &mut TerrainCache,
) -> ColumnResult {
    let start = Instant::now();
    let limit = generator.chunk_limit();
    let targets = if column.iter().all(|c| (-limit..limit).contains(c)) {
        generator
            .world_height()
            .chunks()
            .map(|y| ChunkPos([column[0], y, column[1]]))
            .collect()
    } else {
        BTreeSet::new()
    };
    if targets.is_empty() {
        return ColumnResult {
            column,
            revision,
            chunks: vec![],
            lights: BTreeMap::new(),
            lighting: LightMetrics::default(),
            elapsed: start.elapsed(),
        };
    }
    let (source, generated) = lit_source(generator, snapshot, &edits, &targets, cache);
    let batch = lit_batch(
        &source,
        &targets,
        &BTreeMap::new(),
        &targets,
        generated,
        start,
    );
    let mut meshes: BTreeMap<_, _> = batch.meshes.into_iter().collect();
    let chunks = targets
        .iter()
        .map(|&p| {
            (
                p,
                source.chunk(p).unwrap().clone(),
                meshes.remove(&p).unwrap_or_default(),
            )
        })
        .collect();
    ColumnResult {
        column,
        revision,
        chunks,
        lights: batch.lights,
        lighting: batch.lighting,
        elapsed: start.elapsed(),
    }
}
/// Initial spawn preparation is one compact column, before the App.
/// No full-world generation is performed by the streamed client.
pub fn prepare_spawn(
    world: &mut World,
) -> Result<Arc<Generator>, crate::worldgen::UnsupportedGenerator> {
    prepare_column(world, [0, 0])
}
pub fn prepare_column(
    world: &mut World,
    column: Column,
) -> Result<Arc<Generator>, crate::worldgen::UnsupportedGenerator> {
    let identity = world.identity().expect("streamed world identity");
    let generator = Arc::new(Generator::new(GenerationConfig {
        seed: identity.seed,
        version: identity.generator,
    })?);
    let sampler = generator.sampler(column);
    let edits = world.overlay_snapshot();
    // Startup preparation only: this column can later be evicted like any other.
    for y in generator.world_height().chunks() {
        let p = ChunkPos([column[0], y, column[1]]);
        let mut chunk = sampler.chunk(p).unwrap();
        for (&pos, &block) in edits.range_x(
            BlockPos([column[0] * 16, i32::MIN, i32::MIN]),
            BlockPos([column[0] * 16 + 16, i32::MIN, i32::MIN]),
        ) {
            if pos.split().0 == p {
                chunk.set(pos.split().1, block);
            }
        }
        world.insert_chunk(p, chunk);
    }
    Ok(generator)
}
pub fn new_world(seed: u64) -> World {
    World::streaming(StreamIdentity { seed, generator: 2 })
}

/// Copy cached authoritative full-height source columns. Missing streamed data
/// is supplied on the worker, never interpreted as transparent pending terrain.
pub fn mesh_snapshot(world: &World, targets: &BTreeSet<ChunkPos>) -> World {
    let mut snapshot = World::default();
    if let Some(id) = world.identity() {
        snapshot.set_mesh_identity(id);
    }
    for c in crate::lighting::source_columns(targets) {
        for y in world.world_height().chunks() {
            let p = ChunkPos([c[0], y, c[1]]);
            if let Some(chunk) = world.chunk(p) {
                snapshot.insert_mesh_chunk(p, chunk.clone());
            }
        }
    }
    snapshot
}
fn lit_source(
    generator: &Generator,
    mut source: World,
    edits: &EditSnapshot,
    targets: &BTreeSet<ChunkPos>,
    cache: &mut TerrainCache,
) -> (World, usize) {
    if cache.identity != Some(generator.config()) {
        cache.columns.clear();
        cache.identity = Some(generator.config());
    }
    source.set_mesh_identity(StreamIdentity {
        seed: generator.config().seed,
        generator: generator.config().version,
    });
    let height = generator.world_height();
    let mut generated = 0;
    let limit = generator.chunk_limit();
    for c in crate::lighting::source_columns(targets) {
        if !c.iter().all(|v| (-limit..limit).contains(v)) {
            continue;
        }
        if height
            .chunks()
            .all(|y| source.chunk(ChunkPos([c[0], y, c[1]])).is_some())
        {
            continue;
        }
        let chunks = cache.columns.entry(c).or_insert_with(|| {
            generated += 1;
            let sampler = generator.sampler(c);
            height
                .chunks()
                .map(|y| sampler.chunk(ChunkPos([c[0], y, c[1]])).unwrap())
                .collect()
        });
        for y in height.chunks() {
            let p = ChunkPos([c[0], y, c[1]]);
            if source.chunk(p).is_none() {
                source.insert_mesh_chunk(p, chunks[(y - height.chunks().start) as usize].clone());
            }
        }
    }
    if let Some(min) = source.chunks().map(|(p, _)| p.0[0]).min() {
        let max = source.chunks().map(|(p, _)| p.0[0]).max().unwrap() + 1;
        for (&p, &b) in edits.range_x(
            BlockPos([min * 16, i32::MIN, i32::MIN]),
            BlockPos([max * 16, i32::MIN, i32::MIN]),
        ) {
            source.apply_cached_edit(p, b);
        }
    }
    (source, generated)
}
pub fn needs_mesh(world: &World, p: ChunkPos) -> bool {
    let Some(c) = world.chunk(p) else {
        return false;
    };
    if c.is_empty() {
        return false;
    }
    if c.uniform().is_some_and(|b| b.is_opaque()) {
        return crate::fluid::NEIGHBORS.iter().any(|d| {
            let n = ChunkPos(std::array::from_fn(|a| p.0[a] + d[a]));
            !world
                .chunk(n)
                .and_then(Chunk::uniform)
                .is_some_and(|b| b.is_opaque())
        });
    }
    true
}
pub struct MeshBatchResult {
    pub meshes: Vec<(ChunkPos, ChunkMesh)>,
    pub lights: BTreeMap<ChunkPos, crate::lighting::ChunkLight>,
    pub lighting: LightMetrics,
    pub sampled_border_cells: usize,
    pub elapsed: Duration,
}
fn lit_batch(
    source: &World,
    targets: &BTreeSet<ChunkPos>,
    previous: &BTreeMap<ChunkPos, crate::lighting::ChunkLight>,
    force: &BTreeSet<ChunkPos>,
    generated: usize,
    start: Instant,
) -> MeshBatchResult {
    let sparse = source.world_height() == crate::bounds::WorldHeight::EXTENDED;
    if sparse && targets.iter().all(|&p| !needs_mesh(source, p)) {
        return MeshBatchResult {
            meshes: targets
                .iter()
                .filter(|p| force.contains(p) || previous.contains_key(p))
                .map(|&p| (p, ChunkMesh::default()))
                .collect(),
            lights: BTreeMap::new(),
            lighting: LightMetrics {
                generated_columns: generated,
                ..Default::default()
            },
            sampled_border_cells: 0,
            elapsed: start.elapsed(),
        };
    }
    let light = crate::lighting::Skylight::for_chunks(source, targets);
    let mut meshes = vec![];
    let mut lights = BTreeMap::new();
    for &p in targets {
        // Air has no faces and needs no 18³ light cache. Enclosed uniform Stone
        // likewise has no geometry. Edits can materialize either on the next job.
        if sparse && !needs_mesh(source, p) {
            if force.contains(&p) || previous.contains_key(&p) {
                meshes.push((p, ChunkMesh::default()));
            }
            continue;
        }
        let data = light.chunk(p);
        if force.contains(&p) || previous.get(&p) != Some(&data) {
            meshes.push((p, crate::mesh::mesh_chunk_lit(source, p, &light)));
        }
        lights.insert(p, data);
    }
    MeshBatchResult {
        meshes,
        lights,
        lighting: LightMetrics {
            cells: light.cells(),
            propagated: light.propagated + light.block_propagated,
            generated_columns: generated,
        },
        sampled_border_cells: generated
            * crate::bounds::WorldHeight::HISTORICAL.chunk_count()
            * 4096,
        elapsed: start.elapsed(),
    }
}
/// Reference entry point: remesh every requested chunk. Production additionally
/// compares exact applied light samples to avoid remeshing unaffected geometry.
pub fn remesh_cached(
    generator: &Generator,
    snapshot: World,
    edits: impl Into<EditSnapshot>,
    targets: BTreeSet<ChunkPos>,
) -> MeshBatchResult {
    remesh_lit(
        generator,
        snapshot,
        edits.into(),
        &targets,
        &mut TerrainCache::default(),
        &BTreeMap::new(),
        &targets,
    )
}
pub fn remesh_lit(
    generator: &Generator,
    snapshot: World,
    edits: EditSnapshot,
    targets: &BTreeSet<ChunkPos>,
    cache: &mut TerrainCache,
    previous: &BTreeMap<ChunkPos, crate::lighting::ChunkLight>,
    force: &BTreeSet<ChunkPos>,
) -> MeshBatchResult {
    let start = Instant::now();
    let (source, generated) = lit_source(generator, snapshot, &edits, targets, cache);
    lit_batch(&source, targets, previous, force, generated, start)
}

/// Water-only geometry publication reuses exact applied light and skips solid greedy meshing.
pub fn remesh_water(
    generator: &Generator,
    snapshot: World,
    edits: EditSnapshot,
    targets: &BTreeSet<ChunkPos>,
    cache: &mut TerrainCache,
    previous: &BTreeMap<ChunkPos, crate::lighting::ChunkLight>,
) -> MeshBatchResult {
    let start = Instant::now();
    let (source, generated) = lit_source(generator, snapshot, &edits, targets, cache);
    let meshes = targets
        .iter()
        .map(|&p| {
            (
                p,
                ChunkMesh {
                    water_only: true,
                    water: crate::water_mesh::mesh_water_cached(&source, p, &previous[&p]),
                    ..Default::default()
                },
            )
        })
        .collect();
    MeshBatchResult {
        meshes,
        lights: BTreeMap::new(),
        lighting: LightMetrics {
            generated_columns: generated,
            ..Default::default()
        },
        sampled_border_cells: generated
            * crate::bounds::WorldHeight::HISTORICAL.chunk_count()
            * 4096,
        elapsed: start.elapsed(),
    }
}

#[cfg(test)]
mod tests;

/// New large-world identity. Time/process/counter nonce distinguishes worlds;
/// it is an identifier, not a cryptographic authentication token.
pub fn new_large_world(seed: u64) -> World {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let nonce = (now
        ^ (u128::from(std::process::id()) << 64)
        ^ (u128::from(NEXT.fetch_add(1, Ordering::Relaxed)) << 96))
        .max(1)
        .to_le_bytes();
    World::regional(crate::regions::RegionIdentity {
        seed,
        generator: crate::worldgen::CURRENT_LOCAL_GENERATOR,
        nonce,
    })
}

/// Nearest rings first, with a deterministic preference ahead of motion inside
/// each ring. This never expands residency or starves nearer terrain.
pub fn nearest_ahead(
    columns: &BTreeSet<Column>,
    center: Column,
    motion: [f64; 2],
) -> Option<Column> {
    let dir = motion.map(|v| {
        if v.is_finite() && v.abs() > 0.1 {
            if v > 0. { 1_i64 } else { -1 }
        } else {
            0
        }
    });
    columns
        .iter()
        .min_by_key(|p| {
            let dx = i64::from(p[0]) - i64::from(center[0]);
            let dz = i64::from(p[1]) - i64::from(center[1]);
            (
                dx.abs().max(dz.abs()),
                -(dx * dir[0] + dz * dir[1]),
                i128::from(dx).pow(2) + i128::from(dz).pow(2),
                **p,
            )
        })
        .copied()
}

#[cfg(test)]
use crate::mesh_chunk;
