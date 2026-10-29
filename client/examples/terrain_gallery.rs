// SPDX-License-Identifier: MIT OR Apache-2.0
//! Real versioned terrain, production textures/lighting/materials. No save access.
//! terrain_gallery SEED X Z YAW_DEGREES [--capture=path.ppm] [--overview]
#[allow(dead_code)]
#[path = "../src/atmosphere.rs"]
mod atmosphere;
#[path = "../src/terrain_material.rs"]
mod terrain_material;
#[path = "../src/terrain_texture.rs"]
mod terrain_texture;
use atmosphere::{AtmospherePlugin, AtmosphereUniform, SkyMaterial};
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::{render_resource::PrimitiveTopology, view::Tonemapping},
};
use terrain_material::{AtlasRepeat, TerrainRenderingPlugin, TerrainSurface};
use voxel_core::{
    ChunkPos,
    lighting::Skylight,
    mesh_chunk_lit,
    worldgen::{GenerationConfig, Generator},
};
#[derive(Resource)]
struct Scene {
    seed: u64,
    version: u32,
    x: i32,
    z: i32,
    yaw: f32,
    overview: bool,
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() >= 4,
        "SEED X Z YAW_DEGREES [--capture=path.ppm] [--overview]"
    );
    let scene = Scene {
        version: args
            .iter()
            .find_map(|s| s.strip_prefix("--generator="))
            .map_or(4, |s| s.parse().unwrap()),
        seed: args[0].parse().unwrap(),
        x: args[1].parse().unwrap(),
        z: args[2].parse().unwrap(),
        yaw: args[3].parse::<f32>().unwrap().to_radians(),
        overview: args.iter().any(|s| s == "--overview"),
    };
    assert!(scene.x.abs() < 32000 && scene.z.abs() < 32000 && scene.yaw.is_finite());
    let mut app = App::new();
    app.insert_resource(scene)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustCraft terrain gallery".into(),
                resolution: (1280, 720).into(),
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((TerrainRenderingPlugin, AtmospherePlugin))
        .init_resource::<Assets<SkyMaterial>>()
        .add_systems(Startup, setup)
        .add_systems(Update, capture);
    let embedded = app
        .world()
        .resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>();
    for (name, bytes) in [
        (
            "terrain.wesl",
            include_bytes!("../src/terrain.wesl").as_slice(),
        ),
        ("sky.wesl", include_bytes!("../src/sky.wesl").as_slice()),
        ("water.wesl", include_bytes!("../src/water.wesl").as_slice()),
    ] {
        embedded.insert_asset(
            std::path::PathBuf::new(),
            &std::path::Path::new("rustcraft").join(name),
            bytes,
        );
    }
    app.run();
}
fn setup(
    mut commands: Commands,
    scene: Res<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainSurface>>,
    mut sky: ResMut<Assets<SkyMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let radius = 3;
    let origin = [scene.x.div_euclid(16) * 16, scene.z.div_euclid(16) * 16];
    let center = origin.map(|v| v / 16);
    let g = Generator::new(GenerationConfig {
        seed: scene.seed,
        version: scene.version,
    })
    .unwrap();
    let mut world = voxel_core::World::regional(voxel_core::regions::RegionIdentity {
        seed: scene.seed,
        generator: scene.version,
        nonce: [1; 16],
    });
    for z in center[1] - radius - 1..=center[1] + radius + 1 {
        for x in center[0] - radius - 1..=center[0] + radius + 1 {
            let s = g.sampler([x, z]);
            for y in -1..2 {
                let p = ChunkPos([x, y, z]);
                world.insert_chunk(p, s.chunk(p).unwrap());
            }
        }
    }
    let targets = world
        .chunks()
        .map(|(p, _)| p)
        .filter(|p| (p.0[0] - center[0]).abs() <= radius && (p.0[2] - center[1]).abs() <= radius)
        .collect();
    let light = Skylight::for_chunks(&world, &targets);
    let clock = voxel_core::time::WorldTime::from_parts(0, 6000 * 3).unwrap();
    let params = AtmosphereUniform::at_time(radius, clock);
    let material = materials.add(TerrainSurface {
        base: StandardMaterial {
            unlit: true,
            ..default()
        },
        extension: AtlasRepeat {
            atlas: images.add(terrain_texture::atlas_image()),
            atmosphere: params,
        },
    });
    for (p, _) in world.chunks().filter(|(p, _)| {
        (p.0[0] - center[0]).abs() <= radius && (p.0[2] - center[1]).abs() <= radius
    }) {
        let m = mesh_chunk_lit(&world, p, &light);
        if m.indices.is_empty() {
            continue;
        }
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, m.atlas_uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors)
        .with_inserted_indices(Indices::U32(m.indices));
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                (p.0[0] * 16 - origin[0]) as f32,
                (p.0[1] * 16) as f32,
                (p.0[2] * 16 - origin[1]) as f32,
            ),
        ));
    }
    atmosphere::spawn_sky(&mut commands, &mut meshes, &mut sky, params);
    let h = g.height(scene.x, scene.z).unwrap();
    let feet = voxel_core::physics::spawn_position(
        &world,
        &voxel_core::physics::PhysicsConfig::default(),
        [
            f64::from(scene.x) + 0.5,
            f64::from(h + 1),
            f64::from(scene.z) + 0.5,
        ],
    );
    let eye = Vec3::new(
        (feet[0] - f64::from(origin[0])) as f32,
        feet[1] as f32 + if scene.overview { 12. } else { 1.62 },
        (feet[2] - f64::from(origin[1])) as f32,
    );
    let direction = Vec3::new(
        scene.yaw.sin(),
        if scene.overview { -0.3 } else { -0.08 },
        -scene.yaw.cos(),
    );
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 78f32.to_radians(),
            ..default()
        }),
        Tonemapping::Reinhard,
        Transform::from_translation(eye).looking_at(eye + direction, Vec3::Y),
    ));
    println!(
        "TERRAIN_SCENE generator={} seed={} x={} z={} surface_y={h} yaw={} radius={radius} overview={} feet={feet:?}",
        scene.version,
        scene.seed,
        scene.x,
        scene.z,
        scene.yaw.to_degrees(),
        scene.overview
    );
}

fn capture(
    mut commands: Commands,
    mut frames: Local<u32>,
    time: Res<Time<Real>>,
    mut captured: Local<bool>,
) {
    *frames += 1;
    if !*captured
        && *frames >= 120
        && time.elapsed_secs() > 10.
        && let Some(path) =
            std::env::args().find_map(|a| a.strip_prefix("--capture=").map(str::to_owned))
    {
        *captured = true;
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(
                move |shot: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                      mut commands: Commands| {
                    use std::io::Write;
                    let image = &shot.image;
                    let size = image.texture_descriptor.size;
                    let mut file = std::fs::File::create(&path).expect("screenshot destination");
                    write!(file, "P6\n{} {}\n255\n", size.width, size.height).unwrap();
                    let bgra = image.texture_descriptor.format
                        == bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb;
                    for p in image.data.as_ref().unwrap().as_chunks::<4>().0 {
                        file.write_all(&if bgra {
                            [p[2], p[1], p[0]]
                        } else {
                            [p[0], p[1], p[2]]
                        })
                        .unwrap();
                    }
                    commands.write_message(AppExit::Success);
                },
            );
    }
}
