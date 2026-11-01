// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use bevy::{
    camera::{
        CameraProjection,
        primitives::{Frustum, MeshAabb},
        visibility::{VisibilityPlugin, VisibleEntities},
    },
    ecs::system::SystemState,
};
use voxel_core::{Block, Chunk, LocalPos};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        bevy::asset::AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::transform::TransformPlugin,
        VisibilityPlugin,
    ));
    app.insert_resource(VoxelWorld(World::default()))
        .add_plugins(VoxelScenePlugin);
    app.update();
    app
}
fn publish(app: &mut App, cp: ChunkPos, surface: voxel_core::ChunkMesh) {
    let mut state = SystemState::<(
        Commands,
        ResMut<Assets<Mesh>>,
        ResMut<ChunkRenders>,
        Res<TerrainMaterial>,
        Res<RenderOrigin>,
    )>::new(app.world_mut());
    let (mut commands, mut meshes, mut renders, material, origin) =
        state.get_mut(app.world_mut()).unwrap();
    upsert_stream_mesh(
        &mut commands,
        &mut meshes,
        &mut renders,
        cp,
        surface,
        &material,
        &origin,
    );
    state.apply(app.world_mut());
}
fn surface(cp: ChunkPos, n: usize) -> voxel_core::ChunkMesh {
    let mut world = World::default();
    let mut chunk = Chunk::default();
    for (p, b) in [
        ([n, n, n], Block::Stone),
        ([n, n + 1, n], Block::WaterFalling),
        ([n + 1, n, n], Block::Fern),
    ] {
        chunk.set(LocalPos::new(p).unwrap(), b);
    }
    world.insert_chunk(cp, chunk);
    voxel_core::mesh_chunk(&world, cp)
}
fn slots(app: &App, cp: ChunkPos) -> Vec<(Entity, Handle<Mesh>)> {
    let r = app.world().resource::<ChunkRenders>();
    [&r.0, &r.1, &r.2]
        .into_iter()
        .map(|m| (m[&cp].entity, m[&cp].mesh.clone()))
        .collect()
}
fn assert_bounds(app: &App, slots: &[(Entity, Handle<Mesh>)]) {
    for (e, h) in slots {
        let expected = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(h)
            .unwrap()
            .get_aabb()
            .unwrap();
        assert_eq!(*app.world().get::<Aabb>(*e).unwrap(), expected);
    }
}
fn view(app: &mut App, camera: Entity, eye: Vec3, target: Vec3) {
    let transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    let projection = PerspectiveProjection {
        fov: 78f32.to_radians(),
        aspect_ratio: 16. / 9.,
        near: 0.1,
        far: 100.,
        ..default()
    };
    let frustum = projection.compute_frustum(&GlobalTransform::from(transform));
    app.world_mut()
        .entity_mut(camera)
        .insert((transform, frustum));
    app.update();
}
fn visible(app: &App, camera: Entity, e: Entity) -> bool {
    app.world()
        .get::<VisibleEntities>(camera)
        .unwrap()
        .get(std::any::TypeId::of::<Mesh3d>())
        .contains(&e)
}

#[test]
fn bevy_refreshes_all_three_retained_bounds_in_the_publication_frame() {
    let mut app = app();
    let cp = ChunkPos([-1, -1, -1]);
    publish(&mut app, cp, surface(cp, 1));
    app.update();
    let retained = slots(&app, cp);
    assert_bounds(&app, &retained);
    let old = *app.world().get::<Aabb>(retained[0].0).unwrap();
    publish(&mut app, cp, surface(cp, 13));
    // No component removal and no deferred entity reconstruction.
    assert_eq!(*app.world().get::<Aabb>(retained[0].0).unwrap(), old);
    app.update();
    assert_eq!(slots(&app, cp), retained);
    assert_bounds(&app, &retained);
    assert_ne!(*app.world().get::<Aabb>(retained[0].0).unwrap(), old);
    let mut water_only = surface(cp, 2);
    water_only.water_only = true;
    let opaque = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&retained[0].1)
        .unwrap()
        .get_aabb();
    publish(&mut app, cp, water_only);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&retained[0].1)
            .unwrap()
            .get_aabb(),
        opaque
    );
    assert_bounds(&app, &retained);
    publish(&mut app, cp, Default::default());
    app.update();
    for (e, h) in retained {
        assert!(app.world().get_entity(e).is_err());
        assert!(app.world().resource::<Assets<Mesh>>().get(&h).is_none());
    }
}

#[test]
fn real_bevy_frustum_handles_rotations_pitch_and_rebasing_for_all_surfaces() {
    let mut app = app();
    let cp = ChunkPos([-1, -1, -1]);
    publish(&mut app, cp, surface(cp, 7));
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            Frustum::default(),
            VisibleEntities::default(),
            Transform::default(),
        ))
        .id();
    let target = Vec3::splat(-8.);
    for offset in [
        Vec3::X * 24.,
        Vec3::NEG_X * 24.,
        Vec3::Z * 24.,
        Vec3::NEG_Z * 24.,
        Vec3::new(0.01, 24., 0.),
        Vec3::new(0.01, -24., 0.),
    ] {
        view(&mut app, camera, target + offset, target);
        for (e, _) in slots(&app, cp) {
            assert!(visible(&app, camera, e));
        }
        view(&mut app, camera, target + offset, target + offset * 2.);
        for (e, _) in slots(&app, cp) {
            assert!(!visible(&app, camera, e));
        }
    }
    let shift = Vec3::new(-32000., 0., 16000.);
    for (e, _) in slots(&app, cp) {
        app.world_mut().get_mut::<Transform>(e).unwrap().translation -= shift;
    }
    view(
        &mut app,
        camera,
        target + Vec3::Z * 24. - shift,
        target - shift,
    );
    for (e, _) in slots(&app, cp) {
        assert!(visible(&app, camera, e));
    }
    // Near plane intersects the bounds. Bevy 0.20 deliberately omits the
    // perspective far plane in CPU culling (infinite reverse-Z projection).
    view(
        &mut app,
        camera,
        target + Vec3::Z * 0.15 - shift,
        target - Vec3::Z - shift,
    );
    assert!(visible(&app, camera, slots(&app, cp)[0].0));
    view(
        &mut app,
        camera,
        target + Vec3::Z * 1000. - shift,
        target - shift,
    );
    for (e, _) in slots(&app, cp) {
        assert!(visible(&app, camera, e));
        assert!(!app.world().get::<Frustum>(camera).unwrap().intersects_obb(
            app.world().get::<Aabb>(e).unwrap(),
            &app.world().get::<GlobalTransform>(e).unwrap().affine(),
            true,
            true
        ));
    }
    assert_bounds(&app, &slots(&app, cp));
}
