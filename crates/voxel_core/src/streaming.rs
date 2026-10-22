//! Bounded, engine-independent streaming policy and worker payloads.
use crate::{
    Block, BlockPos, Chunk, ChunkMesh, ChunkPos, LocalPos, StreamIdentity, World, mesh_chunk,
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
    let r = radius.clamp(1, MAX_RADIUS);
    let center = center.map(|v| v.clamp(-4, 3));
    let mut result = BTreeSet::from([[0, 0]]); // Bounded permanent respawn pin.
    for z in (center[1] - r).max(-4)..=(center[1] + r).min(3) {
        for x in (center[0] - r).max(-4)..=(center[0] + r).min(3) {
            result.insert([x, z]);
        }
    }
    result
}
pub fn nearest(columns: &BTreeSet<Column>, center: Column) -> Option<Column> {
    let center = center.map(|v| v.clamp(-4, 3));
    columns
        .iter()
        .min_by_key(|p| {
            let dx = i64::from(p[0]) - i64::from(center[0]);
            let dz = i64::from(p[1]) - i64::from(center[1]);
            (dx * dx + dz * dz, **p)
        })
        .copied()
}
pub struct ColumnResult {
    pub column: Column,
    pub revision: u64,
    pub chunks: Vec<(ChunkPos, Chunk, ChunkMesh)>,
    pub elapsed: Duration,
}
/// Generator + edits in a halo gives correct faces even when neighboring GPU
/// chunks are absent. Halo memory is disposable; only the center is installed.
pub fn build_column(
    generator: &Generator,
    edits: Arc<BTreeMap<BlockPos, Block>>,
    column: Column,
    revision: u64,
) -> ColumnResult {
    let start = Instant::now();
    if !(-4..4).contains(&column[0]) || !(-4..4).contains(&column[1]) {
        return ColumnResult {
            column,
            revision,
            chunks: Vec::new(),
            elapsed: start.elapsed(),
        };
    }
    let mut halo = World::default();
    for y in -1..2 {
        let p = ChunkPos([column[0], y, column[1]]);
        if let Some(chunk) = generator.chunk(p) {
            halo.insert_chunk(p, chunk);
        }
    }
    // Meshing samples only the immediately adjacent face cells: generating
    // eight complete neighboring columns would repeat almost nine times the work.
    for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
        let [x, z] = [column[0] + dx, column[1] + dz];
        if !(-4..4).contains(&x) || !(-4..4).contains(&z) {
            continue;
        }
        for y in -1..2 {
            let p = ChunkPos([x, y, z]);
            let mut border = Chunk::default();
            for ly in 0..16 {
                for edge in 0..16 {
                    let local = LocalPos::new(if dx != 0 {
                        [if dx < 0 { 15 } else { 0 }, ly, edge]
                    } else {
                        [edge, ly, if dz < 0 { 15 } else { 0 }]
                    })
                    .unwrap();
                    border.set(local, generator.block(p.block_position(local).unwrap()));
                }
            }
            halo.insert_chunk(p, border);
        }
    }
    // Apply only relevant edits; the shared snapshot remains immutable.
    let min = BlockPos([(column[0] - 1) * 16, i32::MIN, i32::MIN]);
    let max = BlockPos([(column[0] + 2) * 16, i32::MIN, i32::MIN]);
    for (&p, &b) in edits.range(min..max) {
        halo.apply_cached_edit(p, b);
    }
    let chunks = (-1..2)
        .filter_map(|y| {
            let p = ChunkPos([column[0], y, column[1]]);
            halo.chunk(p)
                .map(|chunk| (p, chunk.clone(), mesh_chunk(&halo, p)))
        })
        .collect();
    ColumnResult {
        column,
        revision,
        chunks,
        elapsed: start.elapsed(),
    }
}
/// Initial spawn preparation is exactly one column (12 KiB), before the App.
/// No full-world generation is performed by the streamed client.
pub fn prepare_spawn(
    world: &mut World,
) -> Result<Arc<Generator>, crate::worldgen::UnsupportedGenerator> {
    let identity = world.identity().expect("streamed world identity");
    let generator = Arc::new(Generator::new(GenerationConfig {
        seed: identity.seed,
        version: identity.generator,
    })?);
    // No meshes/halo are needed before Startup; install the pinned column.
    for y in -1..2 {
        let p = ChunkPos([0, y, 0]);
        let mut chunk = generator.chunk(p).unwrap();
        for (&pos, &block) in world.edits().iter() {
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

#[cfg(test)]
mod tests;
