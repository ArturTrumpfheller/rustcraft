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
    vertical: bool,
    deep: bool,
    eye: Option<Vec3>,
    direction: Option<Vec3>,
    cave_torches: bool,
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() >= 4,
        "SEED X Z YAW_DEGREES [--capture=path.ppm] [--overview]"
    );
    let vector = |prefix: &str| {
        args.iter().find_map(|s| s.strip_prefix(prefix)).map(|s| {
            let v: Vec<f32> = s.split(',').map(|v| v.parse().unwrap()).collect();
            assert!(v.len() == 3 && v.iter().all(|v| v.is_finite()));
            Vec3::new(v[0], v[1], v[2])
        })
    };
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
        vertical: args.iter().any(|s| s == "--vertical"),
        deep: args.iter().any(|s| s == "--deep"),
        eye: vector("--eye="),
        direction: vector("--direction="),
        cave_torches: args.iter().any(|s| s == "--cave-torches"),
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
    mut water_materials: ResMut<Assets<terrain_material::WaterSurface>>,
) {
    let radius = std::env::args()
        .find_map(|a| a.strip_prefix("--radius=").map(str::to_owned))
        .map_or(if scene.eye.is_some() { 5 } else { 3 }, |s| {
            s.parse::<i32>().unwrap()
        })
        .clamp(1, 10);
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
            world.install_region(
                voxel_core::regions::RegionPos::of(voxel_core::BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            for y in g.world_height().chunks() {
                let p = ChunkPos([x, y, z]);
                world.insert_chunk(p, s.chunk(p).unwrap());
            }
        }
    }
    if scene.vertical || scene.deep {
        assert_eq!(scene.version, 6, "vertical gallery fixtures require v6");
        use voxel_core::{Block, BlockPos};
        if scene.vertical {
            for y in 12..180 {
                for dx in -3i32..=3 {
                    for dz in -3i32..=3 {
                        if (dx.abs() == 3 && dz.abs() == 3) || y % 16 == 0 {
                            world
                                .set_block(
                                    BlockPos([scene.x + dx, y, scene.z + dz]),
                                    if y % 16 == 0 {
                                        Block::Stone
                                    } else {
                                        Block::Wood
                                    },
                                )
                                .unwrap();
                        }
                    }
                }
            }
        }
        if scene.deep {
            for y in -52..=-44 {
                for dx in -6i32..=6 {
                    for dz in -12i32..=12 {
                        world
                            .set_block(
                                BlockPos([scene.x + dx, y, scene.z + dz]),
                                if y == -52 || y == -44 || dx.abs() == 6 || dz.abs() == 12 {
                                    Block::Stone
                                } else {
                                    Block::Air
                                },
                            )
                            .unwrap();
                    }
                }
            }
            for dz in [-8, 0, 8] {
                world
                    .set_block(BlockPos([scene.x - 4, -51, scene.z + dz]), Block::Torch)
                    .unwrap();
            }
        }
    }
    if scene.cave_torches {
        assert!(
            scene.version == 7 || scene.version == 8,
            "procedural cave torch fixture requires v7/v8"
        );
        use voxel_core::{Block, BlockPos};
        let mut count = 0;
        for x in (scene.x - 36..=scene.x + 36).step_by(6) {
            for z in (scene.z - 36..=scene.z + 36).step_by(6) {
                for y in -59..g.height(x, z).unwrap() {
                    let p = BlockPos([x, y, z]);
                    if world.ready(p)
                        && world.block(p) == Block::Air
                        && world.block(BlockPos([x, y - 1, z])).is_opaque()
                    {
                        world.set_block(p, Block::Torch).unwrap();
                        count += 1;
                    }
                }
            }
        }
        println!(
            "CAVE_LIGHTING_FIXTURE supported_floor_torches={count}; procedural geometry unmodified"
        );
    }
    // Gallery-only larger context: each solve still respects the production
    // seven-column target bound. Fifteen-cell halos make independent fields
    // agree exactly at the batch edges.
    let batch_key = |p: ChunkPos| {
        [
            (p.0[0] - center[0] + radius) / 7,
            (p.0[2] - center[1] + radius) / 7,
        ]
    };
    let mut batches = std::collections::BTreeMap::<_, std::collections::BTreeSet<_>>::new();
    for (p, _) in world.chunks().filter(|(p, _)| {
        (p.0[0] - center[0]).abs() <= radius && (p.0[2] - center[1]).abs() <= radius
    }) {
        batches.entry(batch_key(p)).or_default().insert(p);
    }
    let lights: std::collections::BTreeMap<_, _> = batches
        .into_iter()
        .map(|(key, targets)| (key, Skylight::for_chunks(&world, &targets)))
        .collect();
    let time = std::env::args()
        .find_map(|a| a.strip_prefix("--time=").map(str::to_owned))
        .map_or(6000, |s| s.parse().unwrap());
    let clock = voxel_core::time::WorldTime::from_parts(0, time * 3).unwrap();
    let mut params = AtmosphereUniform::at_time(radius, clock);
    if std::env::args().any(|a| a == "--survey-fog") {
        params.fog.x = 96.;
        params.fog.y = 160.;
    }
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
    let water_material = water_materials.add(terrain_material::WaterSurface {
        base: StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: terrain_material::WaterEffect {
            texture: images.add(terrain_texture::water_image()),
            atmosphere: params,
            effect: Vec4::ZERO,
        },
    });
    for (p, _) in world.chunks().filter(|(p, _)| {
        (p.0[0] - center[0]).abs() <= radius && (p.0[2] - center[1]).abs() <= radius
    }) {
        let m = mesh_chunk_lit(&world, p, &lights[&batch_key(p)]);
        if !m.water.indices.is_empty() {
            let water = m.water;
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, water.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, water.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, water.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, water.colors)
            .with_inserted_indices(Indices::U32(water.indices));
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(water_material.clone()),
                Transform::from_xyz(
                    (p.0[0] * 16 - origin[0]) as f32,
                    (p.0[1] * 16) as f32,
                    (p.0[2] * 16 - origin[1]) as f32,
                ),
            ));
        }
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
    let mut eye = Vec3::new(
        (feet[0] - f64::from(origin[0])) as f32,
        feet[1] as f32 + if scene.overview { 12. } else { 1.62 },
        (feet[2] - f64::from(origin[1])) as f32,
    );
    let mut direction = Vec3::new(
        scene.yaw.sin(),
        if scene.overview { -0.3 } else { -0.08 },
        -scene.yaw.cos(),
    );
    if scene.vertical {
        eye = Vec3::new(
            (scene.x - origin[0] + 12) as f32,
            124.,
            (scene.z - origin[1] + 14) as f32,
        );
        direction = Vec3::new(-12., -8., -14.);
    }
    if scene.deep {
        eye = Vec3::new(
            (scene.x - origin[0] + 3) as f32,
            -49.8,
            (scene.z - origin[1] + 9) as f32,
        );
        direction = Vec3::new(-0.15, -0.1, -1.);
    }
    if let Some(p) = scene.eye {
        assert!(p.x.abs() < 32000. && p.z.abs() < 32000. && (-63. ..192.).contains(&p.y));
        eye = p - Vec3::new(origin[0] as f32, 0., origin[1] as f32);
    }
    if let Some(d) = scene.direction {
        assert!(d.length_squared() > 0.);
        direction = d;
    }
    println!(
        "CAVE_CAMERA seed={} generator={} world_eye={:?} direction={direction:?} torch_fixture={}",
        scene.seed,
        scene.version,
        eye + Vec3::new(origin[0] as f32, 0., origin[1] as f32),
        scene.cave_torches
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
    println!(
        "VERTICAL_FIXTURE tower={} deep={} eye={eye:?}",
        scene.vertical, scene.deep
    );
}

fn capture(
    mut commands: Commands,
    mut frames: Local<u32>,
    time: Res<Time<Real>>,
    mut captured: Local<bool>,
    mut intervals: Local<Vec<f64>>,
) {
    *frames += 1;
    if *frames > 30 {
        intervals.push(time.delta_secs_f64() * 1000.);
    }
    if !*captured
        && *frames
            >= std::env::args()
                .find_map(|a| a.strip_prefix("--capture-frames=").map(str::to_owned))
                .map_or(120, |s| s.parse().unwrap())
        && time.elapsed_secs() > 10.
        && let Some(path) =
            std::env::args().find_map(|a| a.strip_prefix("--capture=").map(str::to_owned))
    {
        *captured = true;
        intervals.sort_by(f64::total_cmp);
        if !intervals.is_empty() {
            let p = |n: usize| intervals[(intervals.len() - 1) * n / 100];
            println!(
                "CAPTURE_FRAME_MS samples={} p50={:.2} p95={:.2} p99={:.2}",
                intervals.len(),
                p(50),
                p(95),
                p(99)
            );
        }
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
