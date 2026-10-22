//! One bounded worker owns CPU generation/mesh construction. Bevy alone owns
//! resident authoritative chunks, entities and GPU asset lifetime.
use crate::{
    player::PlayerState,
    scene::{ChunkRenders, DirtyChunks, TerrainMaterial, VoxelWorld, upsert_stream_mesh},
};
use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::OsString,
    io,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use voxel_core::{
    ChunkPos, World,
    streaming::{self, Column, ColumnResult, MeshBatchResult},
    worldgen::Generator,
};

pub struct StreamingPlugin;
impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StreamState>()
            .add_systems(Update, update_streaming);
    }
}
pub(crate) fn radius_from(value: Option<OsString>) -> io::Result<i32> {
    match value {
        None => Ok(streaming::DEFAULT_RADIUS),
        Some(v) => v
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|r| (1..=streaming::MAX_RADIUS).contains(r))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "RUSTCRAFT_LOAD_RADIUS must be 1, 2 or 3",
                )
            }),
    }
}

#[derive(Clone)]
enum Work {
    Load(Column),
    Mesh(BTreeSet<ChunkPos>),
}
impl Work {
    fn targets(&self) -> BTreeSet<ChunkPos> {
        match self {
            Self::Load([x, z]) => (-1..2).map(|y| ChunkPos([*x, y, *z])).collect(),
            Self::Mesh(targets) => targets.clone(),
        }
    }
}
enum WorkResult {
    Load(ColumnResult),
    Mesh(MeshBatchResult),
}
impl WorkResult {
    fn elapsed(&self) -> Duration {
        match self {
            Self::Load(r) => r.elapsed,
            Self::Mesh(r) => r.elapsed,
        }
    }
}
struct StreamJob {
    handle: JoinHandle<WorkResult>,
    versions: BTreeMap<ChunkPos, u64>,
    work: Work,
}
impl StreamJob {
    fn spawn(work: Work, world: &World, generator: Arc<Generator>) -> io::Result<Self> {
        let versions = work
            .targets()
            .into_iter()
            .map(|p| (p, world.mesh_revision(p)))
            .collect();
        let edits = world.edits();
        let revision = world.revision();
        let request = work.clone();
        // Only cached target/face-neighbor chunks are copied, not the world.
        let snapshot = match &work {
            Work::Mesh(targets) => Some(streaming::mesh_snapshot(world, targets)),
            Work::Load(_) => None,
        };
        let handle = thread::Builder::new()
            .name("rustcraft-terrain".into())
            .spawn(move || match request {
                Work::Load(column) => {
                    WorkResult::Load(streaming::build_column(&generator, edits, column, revision))
                }
                Work::Mesh(targets) => WorkResult::Mesh(streaming::remesh_cached(
                    &generator,
                    snapshot.unwrap(),
                    edits,
                    targets,
                )),
            })?;
        Ok(Self {
            handle,
            versions,
            work,
        })
    }
}

#[derive(Resource, Default)]
pub(crate) struct StreamState {
    generator: Option<Arc<Generator>>,
    radius: i32,
    center: Option<Column>,
    desired: BTreeSet<Column>,
    queued: BTreeSet<Column>,
    // First invalidation time is retained through coalescing/stale jobs.
    dirty: BTreeMap<ChunkPos, Instant>,
    job: Option<StreamJob>,
    pub peak_chunks: usize,
    pub peak_pending: usize,
    pub completed: usize,
    pub discarded: usize,
    pub generated_columns: usize,
    pub remeshed_chunks: usize,
    pub sampled_border_cells: usize,
    pub worker_time: Duration,
    pub max_job: Duration,
    pub max_install: Duration,
    pub max_update: Duration,
    pub max_dirty_latency: Duration,
    pub mesh_bytes: usize,
    recent_frames: VecDeque<Duration>,
}
impl StreamState {
    pub fn new(generator: Arc<Generator>, radius: i32) -> Self {
        Self {
            generator: Some(generator),
            radius: radius.clamp(1, streaming::MAX_RADIUS),
            center: None,
            desired: BTreeSet::new(),
            queued: BTreeSet::new(),
            dirty: BTreeMap::new(),
            job: None,
            peak_chunks: 0,
            peak_pending: 0,
            completed: 0,
            discarded: 0,
            generated_columns: 0,
            remeshed_chunks: 0,
            sampled_border_cells: 0,
            worker_time: Duration::ZERO,
            max_job: Duration::ZERO,
            max_install: Duration::ZERO,
            max_update: Duration::ZERO,
            max_dirty_latency: Duration::ZERO,
            mesh_bytes: 0,
            recent_frames: VecDeque::new(),
        }
    }
    fn retry(&mut self, work: &Work) {
        if let Work::Load(c) = work
            && self.desired.contains(c)
        {
            self.queued.insert(*c);
        }
        // Mesh invalidations stay in dirty until an up-to-date atomic install.
    }
}
impl Drop for StreamState {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.handle.join();
        }
    }
}
#[derive(SystemParam)]
struct StreamScene<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    renders: ResMut<'w, ChunkRenders>,
    material: Res<'w, TerrainMaterial>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
}

// A connected dirty component is one publication unit: splitting shared dirty
// faces between frames can expose holes. Separate components can be prioritized.
fn nearest_dirty(dirty: &BTreeMap<ChunkPos, Instant>, player: [f64; 3]) -> BTreeSet<ChunkPos> {
    let pc = player.map(|v| (v.floor() as i32).div_euclid(16));
    let Some(&first) = dirty.keys().min_by_key(|p| {
        p.0.iter()
            .zip(pc)
            .map(|(&a, b)| (i64::from(a) - i64::from(b)).pow(2))
            .sum::<i64>()
    }) else {
        return BTreeSet::new();
    };
    let mut result = BTreeSet::from([first]);
    let mut frontier = vec![first];
    while let Some(p) = frontier.pop() {
        for axis in 0..3 {
            for sign in [-1, 1] {
                let mut n = p.0;
                n[axis] += sign; // Only bounded world chunk positions enter dirty.
                let n = ChunkPos(n);
                if dirty.contains_key(&n) && result.insert(n) {
                    frontier.push(n);
                }
            }
        }
    }
    result
}

fn update_streaming(
    mut state: ResMut<StreamState>,
    mut world: ResMut<VoxelWorld>,
    player: Res<PlayerState>,
    mut dirty: ResMut<DirtyChunks>,
    mut scene: StreamScene,
    time: Option<Res<Time<Real>>>,
) {
    let Some(generator) = state.generator.clone() else {
        return;
    };
    let update_start = Instant::now();
    if let Some(time) = time
        && !time.delta().is_zero()
    {
        if state.recent_frames.len() == 128 {
            state.recent_frames.pop_front();
        }
        state.recent_frames.push_back(time.delta());
    }
    let mut changed = false;
    let center = [
        (player.body.position[0].floor() as i32)
            .div_euclid(16)
            .clamp(-4, 3),
        (player.body.position[2].floor() as i32)
            .div_euclid(16)
            .clamp(-4, 3),
    ];
    if state.center != Some(center) {
        changed = true;
        let initial = state.center.is_none();
        state.center = Some(center);
        state.desired = streaming::desired(center, state.radius);
        let desired = state.desired.clone();
        state.queued.retain(|c| desired.contains(c));
        state
            .dirty
            .retain(|p, _| desired.contains(&[p.0[0], p.0[2]]));
        for &c in &desired {
            if world.0.chunk(ChunkPos([c[0], 0, c[1]])).is_none() {
                state.queued.insert(c);
            } else if initial {
                // Startup meshes need a procedural halo, even before their
                // neighbors become resident. Do not regenerate cached spawn.
                for y in -1..2 {
                    state
                        .dirty
                        .entry(ChunkPos([c[0], y, c[1]]))
                        .or_insert_with(Instant::now);
                }
            }
        }
        let evict: BTreeSet<_> = world
            .0
            .chunks()
            .map(|(p, _)| [p.0[0], p.0[2]])
            .filter(|c| !desired.contains(c))
            .collect();
        if !evict.is_empty() {
            info!(
                "Stream evict: {} columns; {} edit records retained",
                evict.len(),
                world.0.edits().len()
            );
        }
        for column in evict {
            world.0.evict_column(column);
        }
        let remove: Vec<_> = scene
            .renders
            .0
            .keys()
            .filter(|p| !desired.contains(&[p.0[0], p.0[2]]))
            .copied()
            .collect();
        for p in remove {
            if let Some(render) = scene.renders.0.remove(&p) {
                scene.commands.entity(render.entity).despawn();
                scene.meshes.remove(&render.mesh);
            }
        }
    }
    while let Some(p) = dirty.0.pop_first() {
        if state.desired.contains(&[p.0[0], p.0[2]]) && world.0.chunk(p).is_some() {
            state.dirty.entry(p).or_insert_with(Instant::now);
        }
    }
    if state
        .job
        .as_ref()
        .is_some_and(|job| job.handle.is_finished())
    {
        let job = state.job.take().unwrap();
        let wanted = job
            .versions
            .keys()
            .all(|p| state.desired.contains(&[p.0[0], p.0[2]]));
        let current = job
            .versions
            .iter()
            .all(|(&p, &v)| world.0.mesh_revision(p) == v);
        match job.handle.join() {
            Ok(result) => {
                state.worker_time += result.elapsed();
                state.max_job = state.max_job.max(result.elapsed());
                if wanted && current {
                    let install_start = Instant::now();
                    changed = true;
                    state.completed += 1;
                    let meshes = match result {
                        WorkResult::Load(result) => {
                            // A center change may have rediscovered this pending
                            // in-flight column. Its valid install satisfies that
                            // request too; do not generate it a second time.
                            state.queued.remove(&result.column);
                            result
                                .chunks
                                .into_iter()
                                .map(|(p, c, m)| {
                                    world.0.insert_chunk(p, c);
                                    (p, m)
                                })
                                .collect()
                        }
                        WorkResult::Mesh(result) => {
                            state.sampled_border_cells += result.sampled_border_cells;
                            result.meshes
                        }
                    };
                    // All targets are validated first, then published in this
                    // single Update (including both sides of dirty seams).
                    for (p, surface) in meshes {
                        upsert_stream_mesh(
                            &mut scene.commands,
                            &mut scene.meshes,
                            &mut scene.renders,
                            p,
                            surface,
                            &scene.material.0,
                        );
                        if let Some(since) = state.dirty.remove(&p) {
                            state.max_dirty_latency = state.max_dirty_latency.max(since.elapsed());
                        }
                    }
                    state.peak_chunks = state.peak_chunks.max(world.0.chunks().count());
                    state.max_install = state.max_install.max(install_start.elapsed());
                } else {
                    state.discarded += 1;
                    state.retry(&job.work);
                }
            }
            Err(_) => {
                error!(
                    "Terrain worker stopped; pending terrain remains a safe barrier; retrying bounded requests"
                );
                state.retry(&job.work);
            }
        }
    }
    if state.job.is_none() {
        let targets = nearest_dirty(&state.dirty, player.body.position);
        let work = if !targets.is_empty() {
            Some(Work::Mesh(targets))
        } else {
            streaming::nearest(&state.queued, center).map(Work::Load)
        };
        if let Some(work) = work {
            match StreamJob::spawn(work.clone(), &world.0, generator) {
                Ok(job) => {
                    match work {
                        Work::Load(c) => {
                            state.queued.remove(&c);
                            state.generated_columns += 1;
                        }
                        Work::Mesh(ref targets) => state.remeshed_chunks += targets.len(),
                    }
                    state.job = Some(job);
                }
                Err(e) => error!("Could not start terrain worker: {e}"),
            }
        }
    }
    state.peak_pending = state
        .peak_pending
        .max(state.queued.len() + state.dirty.len());
    if changed {
        state.mesh_bytes = scene
            .renders
            .0
            .values()
            .filter_map(|r| scene.meshes.get(&r.mesh))
            .map(|m| m.count_vertices() * 40 + m.indices().map_or(0, |i| i.len() * 4))
            .sum();
    }
    state.max_update = state.max_update.max(update_start.elapsed());
    if changed && state.queued.is_empty() && state.dirty.is_empty() && state.job.is_none() {
        let mut frames: Vec<_> = state.recent_frames.iter().copied().collect();
        frames.sort_unstable();
        let frame_p95 = frames
            .get(frames.len() * 95 / 100)
            .copied()
            .unwrap_or_default();
        info!(
            "Stream idle: {} resident chunks, {} meshes, {} block bytes, {} terrain buffer bytes, peak {}, queue peak {}, jobs load {} mesh chunks {} discarded {}, worker total {:?} max {:?}, install max {:?}, stream update max {:?}, dirty latency max {:?}, recent frame p95 {:?} ({} samples)",
            world.0.chunks().count(),
            scene.renders.0.len(),
            world.0.chunks().count() * 4096 * std::mem::size_of::<voxel_core::Block>(),
            state.mesh_bytes,
            state.peak_chunks,
            state.peak_pending,
            state.generated_columns,
            state.remeshed_chunks,
            state.discarded,
            state.worker_time,
            state.max_job,
            state.max_install,
            state.max_update,
            state.max_dirty_latency,
            frame_p95,
            frames.len()
        );
    }
    for mut window in &mut scene.windows {
        let prefix = window
            .title
            .split(" | Chunks:")
            .next()
            .unwrap_or("RustCraft");
        let title = format!(
            "{prefix} | Chunks: {} pending {} dirty {} | Seed {} gen2 radius {}",
            world.0.chunks().count(),
            state.queued.len() + usize::from(state.job.is_some()),
            state.dirty.len(),
            world.0.identity().unwrap().seed,
            state.radius
        );
        if window.title != title {
            window.title = title;
        }
    }
}
#[cfg(test)]
mod tests;
