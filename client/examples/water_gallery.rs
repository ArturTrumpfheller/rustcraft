//! Repeatable Slice22 water fixtures using the production mesh/material shaders.
//! No save access or gameplay time shortcuts. --time=0..23999, --advance-time, --capture=path.ppm.
#[allow(dead_code)]
#[path = "../src/atmosphere.rs"]
mod atmosphere;
#[path = "../../crates/voxel_core/examples/support/water_fixture.rs"]
mod fixture;
#[path = "../src/terrain_material.rs"]
mod terrain_material;
#[path = "../src/terrain_texture.rs"]
mod terrain_texture;
// SLICE22_BEGIN
#[path = "support/live_water.rs"]
mod live_water;
// SLICE22_END
use atmosphere::{AtmospherePlugin, AtmosphereUniform, SkyMaterial};
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::{render_resource::PrimitiveTopology, view::Tonemapping},
};
use terrain_material::{AtlasRepeat, TerrainRenderingPlugin, TerrainSurface};
use voxel_core::{lighting::Skylight, mesh_chunk_lit};
#[derive(Resource)]
struct Options {
    scene: String,
    radius: i32,
    baseline: bool,
    frames: u32,
    motion: bool,
    cycle: bool,
    clock: voxel_core::time::WorldTime,
    advance: bool,
    focus: Option<String>,
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let scene = args.first().cloned().unwrap_or_else(|| "flat".into());
    assert!(
        [
            "dry",
            "pool",
            "flat",
            "waterfall",
            "channel",
            "night",
            "several",
            "boundary"
        ]
        .contains(&scene.as_str())
    );
    let radius = args
        .get(1)
        .map_or(2, |s| s.parse::<i32>().expect("radius 1..3"));
    assert!((1..=3).contains(&radius));
    let baseline = args.iter().any(|s| s == "--baseline");
    let frames = args
        .iter()
        .find_map(|a| a.strip_prefix("--frames="))
        .map_or(0, |a| a.parse::<u32>().expect("frame count"));
    assert!(frames <= 2000);
    let motion = args.iter().any(|a| a == "--motion");
    let cycle = args.iter().any(|a| a == "--cycle");
    let units = args
        .iter()
        .find_map(|a| a.strip_prefix("--time="))
        .map_or(1000, |a| a.parse::<u32>().expect("time 0..23999"));
    assert!(units < 24000);
    let clock = voxel_core::time::WorldTime::from_parts(0, units * 3).unwrap();
    let advance = args.iter().any(|a| a == "--advance-time");
    // Optional visual-review controls; the existing gallery defaults stay intact.
    let resolution =
        args.iter()
            .find_map(|a| a.strip_prefix("--size="))
            .map_or([960, 640], |size| {
                let (w, h) = size.split_once('x').expect("size WIDTHxHEIGHT");
                [w.parse().expect("width"), h.parse().expect("height")]
            });
    assert!((320..=3840).contains(&resolution[0]) && (240..=2160).contains(&resolution[1]));
    let focus = args
        .iter()
        .find_map(|a| a.strip_prefix("--focus="))
        .map(str::to_owned);
    assert!(
        focus
            .as_deref()
            .is_none_or(|f| ["sun", "moon", "pool"].contains(&f))
    );
    let mut app = App::new();
    app.insert_resource(Options {
        scene,
        radius,
        baseline,
        frames,
        motion,
        cycle,
        clock,
        advance,
        focus,
    })
    .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "RustCraft atmosphere gallery".into(),
            resolution: (resolution[0], resolution[1]).into(),
            present_mode: bevy::window::PresentMode::AutoNoVsync,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(TerrainRenderingPlugin)
    .init_resource::<Assets<SkyMaterial>>()
    .init_resource::<Run>()
    .add_systems(Startup, setup)
    .add_systems(Update, (animate, capture));
    if !baseline {
        app.add_plugins(AtmospherePlugin);
    }
    let embedded = app
        .world()
        .resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>();
    for (name, bytes) in [
        (
            "terrain.wesl",
            include_bytes!("../src/terrain.wesl").as_slice(),
        ),
        ("sky.wesl", include_bytes!("../src/sky.wesl").as_slice()),
        // SLICE22_BEGIN
        ("water.wesl", include_bytes!("../src/water.wesl").as_slice()),
        // SLICE22_END
    ] {
        embedded.insert_asset(
            std::path::PathBuf::new(),
            &std::path::Path::new("rustcraft").join(name),
            bytes,
        );
    }
    // SLICE22_BEGIN
    app.add_systems(Update, live_water::update);
    // SLICE22_END
    app.run();
}
#[allow(clippy::too_many_arguments)] // Standalone renderer fixture asset stores.
fn setup(
    mut commands: Commands,
    options: Res<Options>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainSurface>>,
    mut sky_materials: ResMut<Assets<SkyMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut run: ResMut<Run>,
    // SLICE22_BEGIN
    mut water_materials: ResMut<Assets<terrain_material::WaterSurface>>,
    // SLICE22_END
) {
    let r = options.radius;
    let mut world = fixture::fixture(&options.scene);
    // SLICE22_BEGIN
    fixture::add_water(&mut world, &options.scene);
    let mut changes = Vec::with_capacity(256);
    for _ in 0..if std::env::args().any(|a| a == "--active") {
        0
    } else {
        10000
    } {
        world.tick_fluids(voxel_core::fluid::FluidConfig::default(), &mut changes);
        if world.fluid_updates().pending() == 0 {
            break;
        }
    }
    if !std::env::args().any(|a| a == "--active") {
        assert_eq!(world.fluid_updates().pending(), 0);
    }
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
            atmosphere: AtmosphereUniform::at_time(r, options.clock),
            effect: Vec4::ZERO,
        },
    });
    run.water_material = water_material.clone();
    // SLICE22_END
    let targets = world
        .chunks()
        .map(|(p, _)| p)
        .filter(|p| p.0[0].abs() <= r && p.0[2].abs() <= r)
        .collect();
    let light = Skylight::for_chunks(&world, &targets);
    let params = if options.baseline {
        AtmosphereUniform::disabled()
    } else {
        AtmosphereUniform::at_time(r, options.clock)
    };
    let mut quads = 0;
    let atlas = images.add(terrain_texture::atlas_image());
    let material = materials.add(TerrainSurface {
        base: StandardMaterial {
            unlit: true,
            ..default()
        },
        extension: AtlasRepeat {
            atlas,
            atmosphere: params,
        },
    });
    // SLICE22_BEGIN
    let mut live_handles = std::collections::BTreeMap::new();
    let mut terrain_handles = std::collections::BTreeMap::new();
    // SLICE22_END
    for (cp, _) in world
        .chunks()
        .filter(|(p, _)| p.0[0].abs() <= r && p.0[2].abs() <= r)
    {
        let m = mesh_chunk_lit(&world, cp, &light);
        // SLICE22_BEGIN
        if !m.water.indices.is_empty() {
            let surface = &m.water;
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors.clone())
            .with_inserted_indices(Indices::U32(surface.indices.clone()));
            let h = meshes.add(mesh);
            let e = commands
                .spawn((
                    Mesh3d(h.clone()),
                    MeshMaterial3d(water_material.clone()),
                    Transform::from_translation(Vec3::from_array(cp.0.map(|a| (a * 16) as f32))),
                ))
                .id();
            live_handles.insert(cp, (e, h));
        }
        // SLICE22_END
        if m.indices.is_empty() {
            continue;
        }
        quads += m.indices.len() / 6;
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
        let h = meshes.add(mesh);
        let _e = commands
            .spawn((
                Mesh3d(h.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(Vec3::from_array(cp.0.map(|a| (a * 16) as f32))),
            ))
            .id();
        // SLICE22_BEGIN
        terrain_handles.insert(cp, (_e, h));
        // SLICE22_END
    }
    run.terrain = material;
    if !options.baseline {
        run.sky = Some(
            atmosphere::spawn_sky(&mut commands, &mut meshes, &mut sky_materials, params).material,
        );
    }
    println!(
        "WATER_ASSETS radius={} quads={} mesh_bytes={} vertex_stride=56 light_bytes={} terrain_materials={} sky_materials={} meshes={} images={}",
        r,
        quads,
        quads * 248,
        light.bytes(),
        materials.len(),
        sky_materials.len(),
        meshes.len(),
        images.len()
    );
    let center = if options.scene == "boundary" { 15. } else { 0. };
    let (eye, target) = (
        Vec3::new(center + 11., 9., 14.),
        Vec3::new(center + 1., 2., 0.),
    );
    let transform = if options.focus.as_deref() == Some("pool") {
        Transform::from_xyz(2.6, 6., 3.2).looking_at(Vec3::new(0., 1., 0.), Vec3::Y)
    } else if let Some(focus) = &options.focus {
        let direction = Vec3::from_array(if focus == "sun" {
            options.clock.sun_direction()
        } else {
            options.clock.moon_direction()
        });
        // A zenith-facing diagnostic camera needs a nonparallel up vector.
        let up = if direction.y.abs() > 0.99 {
            Vec3::NEG_Z
        } else {
            Vec3::Y
        };
        Transform::from_translation(eye).looking_at(eye + direction, up)
    } else {
        Transform::from_translation(eye).looking_at(target, Vec3::Y)
    };
    // SLICE22_BEGIN
    if std::env::args().any(|a| a == "--active") {
        let lights = world
            .chunks()
            .filter(|(p, _)| p.0[0].abs() <= r && p.0[2].abs() <= r)
            .map(|(p, _)| (p, light.chunk(p)))
            .collect();
        let targets = world
            .chunks()
            .map(|(p, _)| p)
            .filter(|p| p.0[0].abs() <= r && p.0[2].abs() <= r)
            .collect();
        commands.insert_resource(live_water::LiveWater::new(
            world,
            targets,
            lights,
            live_handles,
            water_material,
            fixture::source_positions(&options.scene),
            terrain_handles,
        ));
    }
    // SLICE22_END
    run.base = transform;
    commands.spawn((
        GalleryCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: if options.focus.is_some() || options.scene == "cave" {
                78f32
            } else {
                65f32
            }
            .to_radians(),
            ..default()
        }),
        Tonemapping::Reinhard,
        transform,
    ));
}

#[derive(Component)]
struct GalleryCamera;
#[derive(Resource, Default)]
struct Run {
    frame: u32,
    samples: Vec<f64>,
    control_ns: Vec<u128>,
    base: Transform,
    terrain: Handle<TerrainSurface>,
    sky: Option<Handle<SkyMaterial>>,
    writes: usize,
    // SLICE22_BEGIN
    water_material: Handle<terrain_material::WaterSurface>,
    // SLICE22_END
}
#[allow(clippy::too_many_arguments)] // Standalone renderer fixture asset stores.
fn animate(
    options: Res<Options>,
    time: Res<Time<Real>>,
    mut run: ResMut<Run>,
    mut camera: Query<&mut Transform, With<GalleryCamera>>,
    mut terrain: ResMut<Assets<TerrainSurface>>,
    mut sky: ResMut<Assets<SkyMaterial>>,
    mut exit: MessageWriter<AppExit>,
    // SLICE22_BEGIN
    mut water: ResMut<Assets<terrain_material::WaterSurface>>,
    live: Option<Res<live_water::LiveWater>>,
    // SLICE22_END
) {
    let now = std::time::Instant::now();
    run.frame += 1;
    if options.motion
        && let Ok(mut camera) = camera.single_mut()
    {
        let phase = run.frame as f32 * 0.02;
        camera.translation = run.base.translation + Vec3::new(phase.sin() * 2., 0., phase.cos());
        camera.rotation = run.base.rotation * Quat::from_rotation_y(phase.sin() * 0.3);
    }
    if !options.baseline {
        let radius = if options.cycle {
            1 + ((run.frame / 40) % 3) as i32
        } else {
            options.radius
        };
        run.writes += atmosphere::update_materials(
            AtmosphereUniform::at_time(radius, {
                let mut clock = options.clock;
                if options.advance {
                    clock.advance((time.elapsed_secs_f64() * 60.) as u32);
                }
                clock
            }),
            &run.terrain,
            run.sky.as_ref(),
            &mut terrain,
            &mut sky,
        );
    }
    // SLICE22_BEGIN
    if let Some(mut m) = water.get_mut(&run.water_material) {
        m.extension.effect.x = time.elapsed_secs();
    }
    // SLICE22_END
    let control = now.elapsed().as_nanos();
    if options.frames > 0 && run.frame > 120 && time.elapsed_secs() > 5. {
        run.samples.push(time.delta().as_secs_f64() * 1000.);
        run.control_ns.push(control);
        if run.samples.len() >= options.frames as usize {
            run.samples.sort_by(f64::total_cmp);
            run.control_ns.sort_unstable();
            let n = run.samples.len();
            println!(
                "WATER_FRAME scene={} radius={} baseline={} motion={} cycle={} frames={} p50_ms={:.4} p95_ms={:.4} p99_ms={:.4} control_p50_ns={} writes={}",
                options.scene,
                options.radius,
                options.baseline,
                options.motion,
                options.cycle,
                n,
                run.samples[n / 2],
                run.samples[n * 95 / 100],
                run.samples[n * 99 / 100],
                run.control_ns[n / 2],
                run.writes
            );
            // SLICE22_BEGIN
            if let Some(live) = live {
                println!(
                    "WATER_LIVE updates={} sim_max_us={:.3} peak_pending={} mesh_jobs={} final_water_meshes={}",
                    live.world.fluid_updates().total_processed,
                    live.sim_max_us,
                    live.peak_pending,
                    live.mesh_jobs,
                    live.handles.len()
                );
            }
            // SLICE22_END
            exit.write(AppExit::Success);
        }
    }
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
