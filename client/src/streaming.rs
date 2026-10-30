//! One bounded worker owns CPU generation/mesh construction. Bevy alone owns
//! resident authoritative chunks, entities and GPU asset lifetime.
use crate::{
    player::PlayerState,
    scene::{
        ChunkRenders, DirtyChunks, RenderOrigin, TerrainMaterial, VoxelWorld, upsert_stream_mesh,
    },
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
    persistence::{SaveError, SaveStore},
    regions::EditSnapshot,
    streaming::{self, Column, ColumnResult, MeshBatchResult},
    worldgen::Generator,
};

pub struct StreamingPlugin;
impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StreamState>()
            .init_resource::<StreamFailure>()
            .add_systems(Update, update_streaming.run_if(crate::session::has_world));
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
    fn targets(&self, height: voxel_core::bounds::WorldHeight) -> BTreeSet<ChunkPos> {
        match self {
            Self::Load([x, z]) => height.chunks().map(|y| ChunkPos([*x, y, *z])).collect(),
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
struct JobResult {
    result: WorkResult,
    overlays: EditSnapshot,
    cache: streaming::TerrainCache,
}
#[derive(Resource, Default)]
pub(crate) struct StreamFailure(pub Option<String>);
struct StreamJob {
    handle: JoinHandle<Result<JobResult, SaveError>>,
    versions: BTreeMap<ChunkPos, u64>,
    work: Work,
}
impl StreamJob {
    #[allow(clippy::too_many_arguments)] // Bounded captured worker inputs.
    fn spawn(
        work: Work,
        world: &World,
        generator: Arc<Generator>,
        store: Option<Arc<SaveStore>>,
        mut cache: streaming::TerrainCache,
        previous: BTreeMap<ChunkPos, voxel_core::lighting::ChunkLight>,
        force: BTreeSet<ChunkPos>,
        water_only: bool,
    ) -> io::Result<Self> {
        let versions = work
            .targets(world.world_height())
            .into_iter()
            .map(|p| (p, world.mesh_revision(p)))
            .collect();
        let mut edits = world.overlay_snapshot();
        let regional = world.region_identity();
        let needed = streaming::lighting_regions(&work.targets(world.world_height()));
        edits.retain_regions(&needed);
        let revision = world.revision();
        let request = work.clone();
        let snapshot = streaming::mesh_snapshot(world, &work.targets(world.world_height()));
        let handle = thread::Builder::new()
            .name("rustcraft-terrain".into())
            .spawn(move || {
                if let Some(id) = regional {
                    let store = store
                        .ok_or_else(|| io::Error::other("regional world has no save store"))?;
                    for p in needed {
                        if !edits.contains_region(p) {
                            edits.insert_region(p, store.read_region(id, p)?);
                        }
                    }
                }
                let result = match request {
                    Work::Load(column) => WorkResult::Load(streaming::build_column_cached(
                        &generator,
                        snapshot,
                        edits.clone(),
                        column,
                        revision,
                        &mut cache,
                    )),
                    Work::Mesh(targets) if water_only => WorkResult::Mesh(streaming::remesh_water(
                        &generator,
                        snapshot,
                        edits.clone(),
                        &targets,
                        &mut cache,
                        &previous,
                    )),
                    Work::Mesh(targets) => WorkResult::Mesh(streaming::remesh_lit(
                        &generator,
                        snapshot,
                        edits.clone(),
                        &targets,
                        &mut cache,
                        &previous,
                        &force,
                    )),
                };
                Ok(JobResult {
                    result,
                    overlays: edits,
                    cache,
                })
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
    cache: streaming::TerrainCache,
    lights: BTreeMap<ChunkPos, voxel_core::lighting::ChunkLight>,
    geometry_dirty: BTreeSet<ChunkPos>,
    water_dirty: BTreeSet<ChunkPos>,
    pub light_cells: usize,
    pub light_propagated: usize,
    pub source_columns_generated: usize,

    radius: i32,
    center: Option<Column>,
    desired: BTreeSet<Column>,
    queued: BTreeSet<Column>,
    queued_at: BTreeMap<Column, Instant>,
    pub max_load_latency: Duration,
    // First invalidation time is retained through coalescing/stale jobs.
    dirty: BTreeMap<ChunkPos, Instant>,
    job: Option<StreamJob>,
    failures: BTreeMap<Column, (Instant, String)>,
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
    recent_jobs: VecDeque<Duration>,
}
impl StreamState {
    pub(crate) fn radius(&self) -> i32 {
        self.radius
    }
    pub(crate) fn set_radius(&mut self, radius: i32) {
        let radius = radius.clamp(1, streaming::MAX_RADIUS);
        if self.radius != radius {
            self.radius = radius;
            self.center = None;
        }
    }
    pub(crate) fn drain(&mut self) -> bool {
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let _ = self.job.take().unwrap().handle.join();
        }
        self.job.is_none()
    }
    pub fn new(generator: Arc<Generator>, radius: i32) -> Self {
        Self {
            generator: Some(generator),
            cache: streaming::TerrainCache::default(),
            lights: BTreeMap::new(),
            geometry_dirty: BTreeSet::new(),
            water_dirty: BTreeSet::new(),
            light_cells: 0,
            light_propagated: 0,
            source_columns_generated: 0,
            radius: radius.clamp(1, streaming::MAX_RADIUS),
            center: None,
            desired: BTreeSet::new(),
            queued: BTreeSet::new(),
            queued_at: BTreeMap::new(),
            max_load_latency: Duration::ZERO,
            dirty: BTreeMap::new(),
            job: None,
            failures: BTreeMap::new(),
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
            recent_jobs: VecDeque::new(),
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
    origin: Res<'w, RenderOrigin>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    flow: Option<Res<'w, crate::session::ClientFlow>>,
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

#[derive(SystemParam)]
struct StreamIo<'w> {
    time: Option<Res<'w, Time<Real>>>,
    save: Option<Res<'w, crate::persistence::SaveSession>>,
    keyboard: Option<Res<'w, ButtonInput<KeyCode>>>,
    failure: ResMut<'w, StreamFailure>,
}
fn update_streaming(
    mut state: ResMut<StreamState>,
    mut world: ResMut<VoxelWorld>,
    player: Res<PlayerState>,
    mut dirty: ResMut<DirtyChunks>,
    mut scene: StreamScene,
    mut io: StreamIo,
) {
    if scene.flow.as_ref().is_some_and(|f| f.leaving()) {
        return;
    }
    let Some(generator) = state.generator.clone() else {
        return;
    };
    let update_start = Instant::now();
    if let Some(time) = &io.time
        && !time.delta().is_zero()
    {
        if state.recent_frames.len() == 128 {
            state.recent_frames.pop_front();
        }
        state.recent_frames.push_back(time.delta());
    }
    if io
        .keyboard
        .as_ref()
        .is_some_and(|k| k.just_pressed(KeyCode::F5))
    {
        for (deadline, _) in state.failures.values_mut() {
            *deadline = Instant::now();
        }
    }
    let limit = world.0.bounds().1[0] / 16;
    let mut changed = false;
    let center = [
        (player.body.position[0].floor() as i32)
            .div_euclid(16)
            .clamp(-limit, limit - 1),
        (player.body.position[2].floor() as i32)
            .div_euclid(16)
            .clamp(-limit, limit - 1),
    ];
    if state.center != Some(center) {
        changed = true;
        let initial = state.center.is_none();
        state.center = Some(center);
        state.desired = streaming::desired_with_limit(center, state.radius, limit);
        let desired = state.desired.clone();
        state
            .lights
            .retain(|p, _| desired.contains(&[p.0[0], p.0[2]]));
        state
            .geometry_dirty
            .retain(|p| desired.contains(&[p.0[0], p.0[2]]));
        state
            .water_dirty
            .retain(|p| desired.contains(&[p.0[0], p.0[2]]));
        state.cache.retain(&desired);
        state.queued.retain(|c| desired.contains(c));
        state.queued_at.retain(|c, _| desired.contains(c));
        state.failures.retain(|c, _| desired.contains(c));
        state
            .dirty
            .retain(|p, _| desired.contains(&[p.0[0], p.0[2]]));
        for &c in &desired {
            if world.0.chunk(ChunkPos([c[0], 0, c[1]])).is_none() {
                state.queued.insert(c);
                state.queued_at.entry(c).or_insert_with(Instant::now);
            } else if initial {
                // Startup meshes need a procedural halo, even before their
                // neighbors become resident. Do not regenerate cached spawn.
                for y in world.0.world_height().chunks() {
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
                world.0.edit_count()
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
        let remove: Vec<_> = scene
            .renders
            .1
            .keys()
            .filter(|p| !desired.contains(&[p.0[0], p.0[2]]))
            .copied()
            .collect();
        for p in remove {
            if let Some(render) = scene.renders.1.remove(&p) {
                scene.commands.entity(render.entity).despawn();
                scene.meshes.remove(&render.mesh);
            }
        }
        let remove: Vec<_> = scene
            .renders
            .2
            .keys()
            .filter(|p| !desired.contains(&[p.0[0], p.0[2]]))
            .copied()
            .collect();
        for p in remove {
            if let Some(r) = scene.renders.2.remove(&p) {
                scene.commands.entity(r.entity).despawn();
                scene.meshes.remove(&r.mesh);
            }
        }
    }
    while let Some(p) = dirty.0.pop_first() {
        if state.desired.contains(&[p.0[0], p.0[2]]) && world.0.chunk(p).is_some() {
            state.water_dirty.remove(&p);
            state.geometry_dirty.insert(p);
            state.dirty.entry(p).or_insert_with(Instant::now);
        }
    }
    while let Some(p) = dirty.1.pop_first() {
        state.water_dirty.remove(&p);
        if state.desired.contains(&[p.0[0], p.0[2]]) && world.0.chunk(p).is_some() {
            state.dirty.entry(p).or_insert_with(Instant::now);
        }
    }
    while let Some(p) = dirty.2.pop_first() {
        if state.desired.contains(&[p.0[0], p.0[2]]) && world.0.chunk(p).is_some() {
            if !state.dirty.contains_key(&p) {
                state.water_dirty.insert(p);
            }
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
            Ok(Ok(payload)) => {
                state.cache = payload.cache;
                let desired = state.desired.clone();
                state.cache.retain(&desired);
                let result = payload.result;
                state.worker_time += result.elapsed();
                if state.recent_jobs.len() == 128 {
                    state.recent_jobs.pop_front();
                }
                state.recent_jobs.push_back(result.elapsed());
                state.max_job = state.max_job.max(result.elapsed());
                let metrics = match &result {
                    WorkResult::Load(r) => &r.lighting,
                    WorkResult::Mesh(r) => &r.lighting,
                };
                state.light_cells += metrics.cells;
                state.light_propagated += metrics.propagated;
                state.source_columns_generated += metrics.generated_columns;
                if wanted && current {
                    let install_start = Instant::now();
                    changed = true;
                    if world.0.region_identity().is_some() {
                        for (p, m) in payload.overlays.regions() {
                            world.0.install_region(p, m);
                        }
                    }
                    for p in job.versions.keys() {
                        state.failures.remove(&[p.0[0], p.0[2]]);
                    }
                    state.completed += 1;
                    let light_samples = match &result {
                        WorkResult::Load(r) => &r.lights,
                        WorkResult::Mesh(r) => &r.lights,
                    };

                    // Water-only results deliberately carry no new light data.
                    // Only a full replacement can prune sparse exact-light caches.
                    let water_only_result = matches!(&result, WorkResult::Mesh(r) if r.meshes.iter().all(|(_,m)| m.water_only));
                    if !water_only_result {
                        for p in job.versions.keys() {
                            if !light_samples.contains_key(p) {
                                state.lights.remove(p);
                            }
                        }
                    }
                    state
                        .lights
                        .extend(light_samples.iter().map(|(&p, l)| (p, l.clone())));
                    let meshes = match result {
                        WorkResult::Load(result) => {
                            // A center change may have rediscovered this pending
                            // in-flight column. Its valid install satisfies that
                            // request too; do not generate it a second time.
                            state.queued.remove(&result.column);
                            if let Some(since) = state.queued_at.remove(&result.column) {
                                state.max_load_latency =
                                    state.max_load_latency.max(since.elapsed());
                            }
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
                    if matches!(job.work, Work::Mesh(_)) {
                        state.remeshed_chunks += meshes.len();
                    }
                    for p in job.versions.keys() {
                        state.geometry_dirty.remove(p);
                        state.water_dirty.remove(p);
                        if let Some(since) = state.dirty.remove(p) {
                            state.max_dirty_latency = state.max_dirty_latency.max(since.elapsed());
                        }
                    }
                    // All targets are validated first, then published in this
                    // single Update (including both sides of dirty seams).
                    for (p, surface) in meshes {
                        upsert_stream_mesh(
                            &mut scene.commands,
                            &mut scene.meshes,
                            &mut scene.renders,
                            p,
                            surface,
                            &scene.material,
                            &scene.origin,
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
            Ok(Err(e)) => {
                error!("Terrain region load failed: {e}; data preserved; F5 retries");
                for p in job
                    .versions
                    .keys()
                    .filter(|p| state.desired.contains(&[p.0[0], p.0[2]]))
                    .copied()
                    .collect::<Vec<_>>()
                {
                    state.failures.insert(
                        [p.0[0], p.0[2]],
                        (Instant::now() + Duration::from_secs(1), e.to_string()),
                    );
                }
                state.retry(&job.work);
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
        let now = Instant::now();
        let blocked = |c: &Column| {
            state
                .failures
                .get(c)
                .is_some_and(|(deadline, _)| *deadline > now)
        };
        let mut eligible = state.dirty.clone();
        let mut targets = BTreeSet::new();
        while !eligible.is_empty() {
            let group = nearest_dirty(&eligible, player.body.position);
            if group.iter().all(|p| !blocked(&[p.0[0], p.0[2]])) {
                targets = group;
                break;
            }
            for p in group {
                eligible.remove(&p);
            }
        }
        let work = if !targets.is_empty() {
            Some(Work::Mesh(targets))
        } else {
            let queued = state
                .queued
                .iter()
                .filter(|c| !blocked(c))
                .copied()
                .collect();
            streaming::nearest_ahead(
                &queued,
                center,
                [player.body.velocity[0], player.body.velocity[2]],
            )
            .map(Work::Load)
        };
        if let Some(work) = work {
            let cache = std::mem::take(&mut state.cache);
            let previous = work
                .targets(world.0.world_height())
                .into_iter()
                .filter_map(|p| state.lights.get(&p).map(|l| (p, l.clone())))
                .collect();
            let force = state
                .geometry_dirty
                .union(&state.water_dirty)
                .copied()
                .collect();
            let water_only = matches!(work, Work::Mesh(_))
                && work
                    .targets(world.0.world_height())
                    .iter()
                    .all(|p| state.water_dirty.contains(p) && state.lights.contains_key(p));
            match StreamJob::spawn(
                work.clone(),
                &world.0,
                generator,
                io.save.as_ref().map(|s| Arc::clone(&s.store)),
                cache,
                previous,
                force,
                water_only,
            ) {
                Ok(job) => {
                    match work {
                        Work::Load(c) => {
                            state.queued.remove(&c);
                            state.generated_columns += 1;
                        }
                        Work::Mesh(_) => {}
                    }
                    state.job = Some(job);
                }
                Err(e) => error!("Could not start terrain worker: {e}"),
            }
        }
    }
    io.failure.0 = state
        .failures
        .values()
        .next()
        .map(|(_, message)| message.clone());
    if changed && world.0.region_identity().is_some() {
        let targets = state.desired.iter().map(|c| ChunkPos([c[0], 0, c[1]]));
        let mut keep = streaming::lighting_regions(&targets.collect());
        if let Some(job) = &state.job {
            keep.extend(streaming::lighting_regions(
                &job.work.targets(world.0.world_height()),
            ));
        }
        world.0.retain_regions(&keep);
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
            .chain(
                scene
                    .renders
                    .2
                    .values()
                    .filter_map(|r| scene.meshes.get(&r.mesh)),
            )
            .map(|m| m.count_vertices() * 56 + m.indices().map_or(0, |i| i.len() * 4))
            .sum::<usize>()
            + scene
                .renders
                .1
                .values()
                .filter_map(|r| scene.meshes.get(&r.mesh))
                .map(|m| m.count_vertices() * 48 + m.indices().map_or(0, |i| i.len() * 4))
                .sum::<usize>();
    }
    state.max_update = state.max_update.max(update_start.elapsed());
    if changed && state.queued.is_empty() && state.dirty.is_empty() && state.job.is_none() {
        let mut frames: Vec<_> = state.recent_frames.iter().copied().collect();
        frames.sort_unstable();
        let frame_p95 = frames
            .get(frames.len() * 95 / 100)
            .copied()
            .unwrap_or_default();
        let frame_p50 = frames.get(frames.len() / 2).copied().unwrap_or_default();
        let frame_p99 = frames
            .get(frames.len() * 99 / 100)
            .copied()
            .unwrap_or_default();
        let seconds = frames.iter().map(Duration::as_secs_f64).sum::<f64>();
        let fps = if seconds > 0. {
            frames.len() as f64 / seconds
        } else {
            0.
        };
        let mut jobs: Vec<_> = state.recent_jobs.iter().copied().collect();
        jobs.sort_unstable();
        let percentile = |p: usize| {
            jobs.get(jobs.len().saturating_sub(1) * p / 100)
                .copied()
                .unwrap_or_default()
        };
        info!(
            "Terrain jobs: p50 {:?} p95 {:?} p99 {:?}; ready descriptors {}, nonempty chunks {}, voxel bytes {}, cache voxel bytes {}",
            percentile(50),
            percentile(95),
            percentile(99),
            world.0.chunk_count(),
            world.0.nonempty_chunk_count(),
            world.0.allocated_voxel_bytes(),
            state.cache.allocated_voxel_bytes()
        );
        info!(
            "Light work: {} cells, {} propagated, {} source columns generated, {} halo columns cached, {} applied chunk samples",
            state.light_cells,
            state.light_propagated,
            state.source_columns_generated,
            state.cache.columns(),
            state.lights.len()
        );
        info!(
            "Stream metrics: regions {} edits {} dirty regions {}, load queue-to-install max {:?}, frames p50 {:?} p95 {:?} p99 {:?}, mean fps {:.1}",
            world.0.cached_regions(),
            world.0.edit_count(),
            world.0.dirty_regions(),
            state.max_load_latency,
            frame_p50,
            frame_p95,
            frame_p99,
            fps
        );
        info!(
            "Stream idle: {} resident chunks, {} meshes, {} block bytes, {} terrain buffer bytes, peak {}, queue peak {}, jobs load {} mesh chunks {} discarded {}, worker total {:?} max {:?}, install max {:?}, stream update max {:?}, dirty latency max {:?}, recent frame p95 {:?} ({} samples)",
            world.0.chunks().count(),
            scene.renders.0.len() + scene.renders.1.len() + scene.renders.2.len(),
            world.0.allocated_voxel_bytes(),
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
            "{prefix} | Chunks: {} pending {} dirty {} | Seed {} gen{} radius {} | Pos {:.1} {:.1} {:.1}",
            world.0.chunks().count(),
            state.queued.len() + usize::from(state.job.is_some()),
            state.dirty.len(),
            world.0.identity().unwrap().seed,
            world.0.identity().unwrap().generator,
            state.radius,
            player.body.position[0],
            player.body.position[1],
            player.body.position[2]
        );
        if window.title != title {
            window.title = title;
        }
    }
}
#[cfg(test)]
mod tests;
