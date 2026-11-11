// SPDX-License-Identifier: MIT OR Apache-2.0
// Appended identically to scene/tests.rs in disposable matched source copies.
#[test]
#[ignore = "Matched CPU installation/real Bevy visibility probe; run via benchmark_streaming.py --rendering-install"]
fn slice33_install_measurements() {
    use crate::slice32_probe as probe;
    use bevy::{
        camera::{
            CameraProjection,
            primitives::Frustum,
            visibility::{VisibilityPlugin, VisibleEntities},
        },
        ecs::system::SystemState,
    };
    use std::time::Instant;
    use voxel_core::{Block, BlockPos};
    assert_eq!(probe::SOURCE, std::env::var("S32_EXPECT_CLIENT").unwrap());
    assert_eq!(
        voxel_core::streaming::slice32_fingerprint(),
        std::env::var("S32_EXPECT_CORE").unwrap()
    );
    let repeats: usize = std::env::var("S32_REPEATS").unwrap().parse().unwrap();
    let mut fixture = World::default();
    for z in 0..16 {
        for x in 0..16 {
            fixture
                .set_block(BlockPos([x, 0, z]), Block::Grass)
                .unwrap();
            fixture
                .set_block(
                    BlockPos([x, 1, z]),
                    if (x + z) % 3 == 0 {
                        Block::Fern
                    } else {
                        Block::Water1
                    },
                )
                .unwrap();
            if x % 3 == 0 && z % 3 == 0 {
                fixture
                    .set_block(BlockPos([x, 3, z]), Block::Stone)
                    .unwrap();
            }
        }
    }
    let fixture = voxel_core::mesh_chunk(&fixture, ChunkPos([0; 3]));
    for repeat in 0..repeats {
        for radius in 1..=3 {
            // 1, 9 and 49 changed chunks form single publication units.
            let edge = [0, 1, 3][radius as usize - 1];
            let targets: Vec<_> = (-edge..=edge)
                .flat_map(|z| (-edge..=edge).map(move |x| ChunkPos([x, 0, z])))
                .collect();
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
            let transform =
                Transform::from_xyz(8., 12., 36.).looking_at(Vec3::new(8., 0., 0.), Vec3::Y);
            let frustum = PerspectiveProjection {
                aspect_ratio: 16. / 9.,
                fov: 78f32.to_radians(),
                ..default()
            }
            .compute_frustum(&GlobalTransform::from(transform));
            let camera = app
                .world_mut()
                .spawn((
                    Camera::default(),
                    frustum,
                    VisibleEntities::default(),
                    transform,
                ))
                .id();
            let mut state = SystemState::<(
                Commands,
                ResMut<Assets<Mesh>>,
                ResMut<ChunkRenders>,
                Res<TerrainMaterial>,
                Res<RenderOrigin>,
            )>::new(app.world_mut());
            probe::reset();
            for sample in 0..144 {
                // Buffers stand in for completed worker output. Their cloning is
                // deliberately outside the CPU publication/bounds measurements.
                let batch: Vec<_> = targets
                    .iter()
                    .map(|&p| {
                        let mut mesh = fixture.clone();
                        let shift = if sample % 2 == 0 { 0. } else { 0.125 };
                        for positions in [
                            &mut mesh.positions,
                            &mut mesh.water.positions,
                            &mut mesh.plants.positions,
                        ] {
                            for vertex in positions {
                                vertex[1] += shift;
                            }
                        }
                        (p, mesh)
                    })
                    .collect();
                let total = Instant::now();
                let (mut commands, mut meshes, mut renders, material, origin) =
                    state.get_mut(app.world_mut()).unwrap();
                let start = Instant::now();
                for (p, m) in batch {
                    upsert_stream_mesh(
                        &mut commands,
                        &mut meshes,
                        &mut renders,
                        p,
                        m,
                        &material,
                        &origin,
                    );
                }
                let assets = start.elapsed();
                let start = Instant::now();
                state.apply(app.world_mut());
                let flush = start.elapsed();
                let start = Instant::now();
                app.update();
                let post = start.elapsed();
                let publication = total.elapsed();
                if sample == 0 {
                    probe::duration("new_surface_install_us", assets);
                    probe::duration("new_entity_apply_us", flush);
                    probe::duration("new_surface_visibility_us", post);
                }
                if sample >= 16 {
                    probe::duration("surface_install_us", assets);
                    probe::duration("command_apply_us", flush);
                    probe::duration("ecs_visibility_update_us", post);
                    probe::duration("publication_total_us", publication);
                }
            }
            let renders = app.world().resource::<ChunkRenders>();
            let count = renders.0.len() + renders.1.len() + renders.2.len();
            assert_eq!(count, targets.len() * 3);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
            let visible = app
                .world()
                .get::<VisibleEntities>(camera)
                .unwrap()
                .len(std::any::TypeId::of::<Mesh3d>());
            probe::value("resident_meshes", count as f64);
            probe::value("frustum_visible_meshes", visible as f64);
            probe::value("frustum_culled_meshes", (count - visible) as f64);
            let meshes = app.world().resource::<Assets<Mesh>>();
            for (metric, map) in [
                ("resident_opaque_triangles", &renders.0),
                ("resident_water_triangles", &renders.1),
                ("resident_cutout_triangles", &renders.2),
            ] {
                let triangles: usize = map
                    .values()
                    .map(|r| meshes.get(&r.mesh).unwrap().indices().unwrap().len() / 3)
                    .sum();
                probe::value(metric, triangles as f64);
            }
            let before = app.world().resource::<Assets<Mesh>>().len();
            let (mut commands, mut meshes, mut renders, material, origin) =
                state.get_mut(app.world_mut()).unwrap();
            let start = Instant::now();
            for &p in &targets {
                upsert_stream_mesh(
                    &mut commands,
                    &mut meshes,
                    &mut renders,
                    p,
                    Default::default(),
                    &material,
                    &origin,
                );
            }
            state.apply(app.world_mut());
            app.update();
            probe::duration("batch_cleanup_us", start.elapsed());
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), before - count);
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<Mesh3d>>()
                    .iter(app.world())
                    .count(),
                0
            );
            probe::report("retained-install", radius, repeat);
        }
    }
}
