//! One bounded worker owns CPU generation/mesh construction. Bevy alone owns
//! resident authoritative chunks, entities and GPU asset lifetime.
use crate::{
    player::PlayerState,
    scene::{ChunkRenders, DirtyChunks, TerrainMaterial, VoxelWorld, upsert_stream_mesh},
};
use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    io,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};
use voxel_core::{
    streaming::{self, Column, ColumnResult},
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
#[derive(Resource, Default)]
pub(crate) struct StreamState {
    generator: Option<Arc<Generator>>,
    radius: i32,
    center: Option<Column>,
    desired: BTreeSet<Column>,
    queued: BTreeSet<Column>,
    job: Option<JoinHandle<ColumnResult>>,
    pub peak_chunks: usize,
    pub completed: usize,
    pub discarded: usize,
    pub max_job: Duration,
    pub mesh_bytes: usize,
}
impl StreamState {
    pub fn new(generator: Arc<Generator>, radius: i32) -> Self {
        Self {
            generator: Some(generator),
            radius: radius.clamp(1, streaming::MAX_RADIUS),
            center: None,
            desired: BTreeSet::new(),
            queued: BTreeSet::new(),
            job: None,
            peak_chunks: 0,
            completed: 0,
            discarded: 0,
            max_job: Duration::ZERO,
            mesh_bytes: 0,
        }
    }
}
impl Drop for StreamState {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.join();
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
fn update_streaming(
    mut state: ResMut<StreamState>,
    mut world: ResMut<VoxelWorld>,
    player: Res<PlayerState>,
    mut dirty: ResMut<DirtyChunks>,
    mut scene: StreamScene,
) {
    let Some(generator) = state.generator.clone() else {
        return;
    };
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
        state.center = Some(center);
        state.desired = streaming::desired(center, state.radius);
        let desired = state.desired.clone();
        state.queued.retain(|c| desired.contains(c));
        for &c in &desired {
            if world
                .0
                .chunk(voxel_core::ChunkPos([c[0], 0, c[1]]))
                .is_none()
                || state.completed == 0
            {
                state.queued.insert(c);
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
        let c = [p.0[0], p.0[2]];
        if state.desired.contains(&c) {
            state.queued.insert(c);
        }
    }
    if state.job.as_ref().is_some_and(|job| job.is_finished()) {
        let result = state.job.take().unwrap().join();
        match result {
            Ok(result)
                if state.desired.contains(&result.column)
                    && result.revision == world.0.revision() =>
            {
                changed = true;
                state.max_job = state.max_job.max(result.elapsed);
                state.completed += 1;
                for (position, chunk, surface) in result.chunks {
                    world.0.insert_chunk(position, chunk);
                    upsert_stream_mesh(
                        &mut scene.commands,
                        &mut scene.meshes,
                        &mut scene.renders,
                        position,
                        surface,
                        &scene.material.0,
                    );
                }
                state.peak_chunks = state.peak_chunks.max(world.0.chunks().count());
                info!(
                    "Stream installed {:?}: {} resident chunks, {} pending columns, job {:?}",
                    result.column,
                    world.0.chunks().count(),
                    state.queued.len(),
                    result.elapsed
                );
            }
            Ok(result) => {
                state.discarded += 1;
                if state.desired.contains(&result.column) {
                    state.queued.insert(result.column);
                }
            }
            Err(_) => {
                error!(
                    "Chunk worker stopped; pending terrain remains a safe barrier; retrying bounded requests"
                );
                let wanted = state.desired.clone();
                state.queued.extend(wanted);
            }
        }
    }
    if state.job.is_none()
        && let Some(column) = streaming::nearest(&state.queued, center)
    {
        state.queued.remove(&column);
        let edits = world.0.edits();
        let revision = world.0.revision();
        match thread::Builder::new()
            .name("rustcraft-chunk".into())
            .spawn(move || streaming::build_column(&generator, edits, column, revision))
        {
            Ok(job) => state.job = Some(job),
            Err(e) => {
                state.queued.insert(column);
                error!("Could not start chunk worker: {e}");
            }
        }
    }
    // Quantify retained terrain buffers, excluding save HUD/selection assets.
    if changed {
        state.mesh_bytes = scene
            .renders
            .0
            .values()
            .filter_map(|r| scene.meshes.get(&r.mesh))
            .map(|m| m.count_vertices() * 40 + m.indices().map_or(0, |i| i.len() * 4))
            .sum();
    }
    if changed && state.queued.is_empty() && state.job.is_none() {
        info!(
            "Stream idle: {} resident chunks, {} meshes, {} block bytes, {} terrain buffer bytes, peak {}, max worker {:?}",
            world.0.chunks().count(),
            scene.renders.0.len(),
            world.0.chunks().count() * 4096 * std::mem::size_of::<voxel_core::Block>(),
            state.mesh_bytes,
            state.peak_chunks,
            state.max_job
        );
    }
    for mut window in &mut scene.windows {
        let prefix = window
            .title
            .split(" | Chunks:")
            .next()
            .unwrap_or("RustCraft");
        let title = format!(
            "{prefix} | Chunks: {} pending {} | Seed {} gen2 radius {}",
            world.0.chunks().count(),
            state.queued.len() + usize::from(state.job.is_some()),
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
