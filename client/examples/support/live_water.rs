// SPDX-License-Identifier: MIT OR Apache-2.0
//! Active gallery fixture: the production core, one bounded mesh worker and retained assets.
use crate::terrain_material::WaterSurface;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    thread::JoinHandle,
    time::Instant,
};
use voxel_core::{
    Block, BlockChange, BlockPos, ChunkPos, World, fluid::FluidConfig, lighting::ChunkLight,
    water_mesh::WaterMesh,
};
type Payload = Vec<(ChunkPos, WaterMesh)>;
#[derive(Resource)]
pub struct LiveWater {
    pub world: World,
    pub targets: BTreeSet<ChunkPos>,
    pub lights: Arc<BTreeMap<ChunkPos, ChunkLight>>,
    pub handles: BTreeMap<ChunkPos, (Entity, Handle<Mesh>)>,
    pub material: Handle<WaterSurface>,
    pub sources: &'static [[i32; 3]],
    job: Option<(u64, JoinHandle<Payload>)>,
    elapsed: f32,
    ticks: u64,
    dirty: bool,
    changes: Vec<BlockChange>,
    pub sim_max_us: f64,
    pub peak_pending: usize,
    pub mesh_jobs: u64,
}
impl LiveWater {
    pub fn new(
        world: World,
        targets: BTreeSet<ChunkPos>,
        lights: BTreeMap<ChunkPos, ChunkLight>,
        handles: BTreeMap<ChunkPos, (Entity, Handle<Mesh>)>,
        material: Handle<WaterSurface>,
        sources: &'static [[i32; 3]],
    ) -> Self {
        Self {
            world,
            targets,
            lights: Arc::new(lights),
            handles,
            material,
            sources,
            job: None,
            elapsed: 0.,
            ticks: 0,
            dirty: false,
            changes: Vec::with_capacity(256),
            sim_max_us: 0.,
            peak_pending: 0,
            mesh_jobs: 0,
        }
    }
}
impl Drop for LiveWater {
    fn drop(&mut self) {
        if let Some((_, job)) = self.job.take() {
            let _ = job.join();
        }
    }
}
pub fn update(
    mut live: Option<ResMut<LiveWater>>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(ref mut live) = live else {
        return;
    };
    if live.job.as_ref().is_some_and(|(_, j)| j.is_finished()) {
        let (revision, job) = live.job.take().unwrap();
        let batch = job.join().expect("water fixture worker");
        if revision == live.world.revision() {
            for (p, surface) in batch {
                if surface.indices.is_empty() {
                    if let Some((e, h)) = live.handles.remove(&p) {
                        commands.entity(e).despawn();
                        meshes.remove(&h);
                    }
                    continue;
                }
                let mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors)
                .with_inserted_indices(Indices::U32(surface.indices));
                if let Some((entity, h)) = live.handles.get(&p) {
                    *meshes.get_mut(h).unwrap() = mesh;
                    commands
                        .entity(*entity)
                        .remove::<bevy::camera::primitives::Aabb>();
                } else {
                    let h = meshes.add(mesh);
                    let e = commands
                        .spawn((
                            Mesh3d(h.clone()),
                            MeshMaterial3d(live.material.clone()),
                            Transform::from_translation(Vec3::from_array(
                                p.0.map(|v| (v * 16) as f32),
                            )),
                        ))
                        .id();
                    live.handles.insert(p, (e, h));
                }
            }
        } else {
            live.dirty = true;
        }
    }
    live.elapsed += time.delta_secs().min(0.25);
    if live.elapsed >= 0.2 {
        live.elapsed = 0.;
        live.ticks += 1;
        // Explicitly pulse the same sources every second, so the measured window
        // contains both expansion and recession rather than only settled geometry.
        if live.ticks.is_multiple_of(5) {
            let on = (live.ticks / 5).is_multiple_of(2);
            for &p in live.sources {
                live.world
                    .set_block(
                        BlockPos(p),
                        if on { Block::WaterSource } else { Block::Air },
                    )
                    .unwrap();
            }
            live.dirty = true;
        }
        let start = Instant::now();
        let mut changes = std::mem::take(&mut live.changes);
        live.world.tick_fluids(FluidConfig::default(), &mut changes);
        live.dirty |= !changes.is_empty();
        live.changes = changes;
        live.sim_max_us = live.sim_max_us.max(start.elapsed().as_secs_f64() * 1e6);
        live.peak_pending = live.peak_pending.max(live.world.fluid_updates().pending());
    }
    if live.dirty && live.job.is_none() {
        let snapshot = voxel_core::streaming::mesh_snapshot(&live.world, &live.targets);
        let targets = live.targets.clone();
        let lights = Arc::clone(&live.lights);
        let revision = live.world.revision();
        let worker = std::thread::spawn(move || {
            targets
                .into_iter()
                .map(|p| {
                    (
                        p,
                        voxel_core::water_mesh::mesh_water_cached(&snapshot, p, &lights[&p]),
                    )
                })
                .collect()
        });
        live.job = Some((revision, worker));
        live.mesh_jobs += 1;
        live.dirty = false;
    }
}
