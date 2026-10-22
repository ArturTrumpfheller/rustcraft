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
    let mut result = BTreeSet::new();
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
    /// Snapshot provenance only. Scheduling validity uses local mesh tokens,
    /// not equality with the global revision used by persistence.
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
    // Startup preparation only: this column can later be evicted like any other.
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

fn neighbors(p: ChunkPos) -> impl Iterator<Item = (usize, i32, ChunkPos)> {
    (0..3).flat_map(move |axis| {
        [-1, 1].into_iter().filter_map(move |sign| {
            let mut n = p.0;
            n[axis] = n[axis].checked_add(sign)?;
            Some((axis, sign, ChunkPos(n)))
        })
    })
}

/// Copy only target chunks and their available face neighbors. The snapshot
/// uses known-Air semantics; the worker fills missing face cells from the
/// immutable generator + edit overlay, never from pending collision barriers.
pub fn mesh_snapshot(world: &World, targets: &BTreeSet<ChunkPos>) -> World {
    let mut snapshot = World::default();
    for &p in targets {
        for p in std::iter::once(p).chain(neighbors(p).map(|(_, _, p)| p)) {
            if snapshot.chunk(p).is_none()
                && let Some(chunk) = world.chunk(p)
            {
                snapshot.insert_chunk(p, chunk.clone());
            }
        }
    }
    snapshot
}

pub struct MeshBatchResult {
    pub meshes: Vec<(ChunkPos, ChunkMesh)>,
    pub sampled_border_cells: usize,
    pub elapsed: Duration,
}

/// Remesh cached voxels without regenerating any complete chunk. All targets
/// are returned as one publication unit so dirty face-neighbor seams agree.
pub fn remesh_cached(
    generator: &Generator,
    mut snapshot: World,
    edits: Arc<BTreeMap<BlockPos, Block>>,
    targets: BTreeSet<ChunkPos>,
) -> MeshBatchResult {
    let start = Instant::now();
    let cached: BTreeSet<_> = snapshot.chunks().map(|(p, _)| p).collect();
    let mut sampled_border_cells = 0;
    for &p in &targets {
        for (axis, sign, neighbor) in neighbors(p) {
            if cached.contains(&neighbor)
                || !(-4..4).contains(&neighbor.0[0])
                || !(-1..2).contains(&neighbor.0[1])
                || !(-4..4).contains(&neighbor.0[2])
            {
                continue;
            }
            // A missing chunk may border several targets on different faces.
            // Preserve earlier faces, but do not mistake a thin face for a full
            // cached chunk. Other cells are never read by this batch's mesher.
            let mut border = snapshot.chunk(neighbor).cloned().unwrap_or_default();
            for a in 0..16 {
                for b in 0..16 {
                    let mut local = [0; 3];
                    local[axis] = if sign < 0 { 15 } else { 0 };
                    local[(axis + 1) % 3] = a;
                    local[(axis + 2) % 3] = b;
                    let local = LocalPos::new(local).unwrap();
                    let pos = neighbor.block_position(local).unwrap();
                    border.set(
                        local,
                        edits
                            .get(&pos)
                            .copied()
                            .unwrap_or_else(|| generator.block(pos)),
                    );
                    sampled_border_cells += 1;
                }
            }
            snapshot.insert_chunk(neighbor, border);
        }
    }
    MeshBatchResult {
        meshes: targets
            .into_iter()
            .map(|p| (p, mesh_chunk(&snapshot, p)))
            .collect(),
        sampled_border_cells,
        elapsed: start.elapsed(),
    }
}

#[cfg(test)]
mod tests;
