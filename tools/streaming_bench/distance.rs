// SPDX-License-Identifier: MIT OR Apache-2.0
// Optional real Bevy CPU visibility and edited fixtures; no RenderApp or GPU.
fn distance_app(path: &std::path::Path, scene: &str) -> App {
    let seed = if matches!(scene,"coast"|"cavern"|"origin") { 42 } else { 0 };
    let (store, world, _) = SaveStore::open_with(path, || Ok(voxel_core::World::regional(
        voxel_core::regions::RegionIdentity { seed, generator:9, nonce:[27;16] }
    ))).unwrap();
    store.save(&world).unwrap();
    drop(store);
    large_app_with_generator(path,9)
}
fn distance_view(app: &mut App) -> Entity {
    let camera = app.world_mut().spawn((Camera::default(),
        bevy::camera::primitives::Frustum::default(),
        bevy::camera::visibility::VisibleEntities::default(),
        Transform::default())).id();
    distance_camera(app, camera);
    app.update();
    camera
}
fn distance_camera(app: &mut App, camera: Entity) {
    use bevy::camera::CameraProjection;
    let p = app.world().resource::<PlayerState>().body.position;
    let origin = [(p[0]/16.).floor()*16.,0.,(p[2]/16.).floor()*16.];
    let eye = Vec3::new((p[0]-origin[0]) as f32, (p[1]-origin[1]+1.62) as f32, (p[2]-origin[2]) as f32);
    let transform = Transform::from_translation(eye).looking_at(eye+Vec3::new(0.,-0.12,-1.),Vec3::Y);
    let frustum = PerspectiveProjection { fov:78f32.to_radians(), aspect_ratio:16./9., ..default() }
        .compute_frustum(&GlobalTransform::from(transform));
    app.world_mut().entity_mut(camera).insert((transform,frustum));
}
fn distance_edits(app: &mut App, center: Column, water: bool) {
    // Real edits at a signed chunk seam. Construction intentionally stresses
    // greedy emission; flowing water is a bounded edited fixture, not a fluid FPS test.
    for x in 12..20 {
        for z in 4..12 {
            let ground = BlockPos([center[0]*16+x,127,center[1]*16+z]);
            edit(app,ground,Block::Stone);
            for y in 128..136 {
                let block = if water {
                    if y==128 { Block::WaterSource } else { Block::Air }
                } else if (x+y+z)%2==0 { Block::Wood } else { Block::Air };
                let p=BlockPos([center[0]*16+x,y,center[1]*16+z]);
                if app.world().resource::<VoxelWorld>().0.block(p)!=block { edit(app,p,block); }
            }
        }
    }
    drain(app);
}
fn distance_resources(app: &App, camera: Entity) {
    use crate::slice32_probe as probe;
    let state=app.world().resource::<StreamState>();
    let world=&app.world().resource::<VoxelWorld>().0;
    let visible=app.world().get::<bevy::camera::visibility::VisibleEntities>(camera).unwrap();
    let renders=app.world().resource::<ChunkRenders>();
    let meshes=app.world().resource::<Assets<Mesh>>();
    for (label,map) in [("visible_opaque",&renders.0),("visible_water",&renders.1),("visible_cutout",&renders.2)] {
        probe::value(label,map.values().filter(|r|visible.get(std::any::TypeId::of::<Mesh3d>()).contains(&r.entity)).count() as f64);
    }
    let light_bytes=state.lights.values().map(|l|l.0.len()).sum::<usize>();
    probe::value("light_cache_bytes",light_bytes as f64);
    probe::value("cached_regions",world.cached_regions() as f64);
    probe::value("fluid_pending",world.fluid_updates().pending() as f64);
    probe::value("resident_triangles", renders.0.values().chain(renders.1.values()).chain(renders.2.values())
        .map(|r|meshes.get(&r.mesh).unwrap().indices().unwrap().len()/3).sum::<usize>() as f64);
    assert!(state.queued.is_empty() && state.dirty.is_empty() && state.job.is_none());
    assert!(state.first_load_dispatch.is_empty() && state.first_mesh_dispatch.is_empty());
}
