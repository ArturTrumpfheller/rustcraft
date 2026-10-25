//! Matched-camera blocklight fixture: dark, one, several, removed, wall, seam.
//! Uses the production terrain material/atlas; never opens or modifies saves.
// Disable atmosphere for reproducible indoor lighting comparisons.
#[allow(dead_code)]
#[path = "../src/atmosphere.rs"]
mod atmosphere;
#[path = "../../crates/voxel_core/examples/support/lighting_fixture.rs"]
mod lighting_fixture;
#[path = "../src/terrain_material.rs"]
mod terrain_material;
#[path = "../src/terrain_texture.rs"]
mod terrain_texture;

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use terrain_material::{AtlasRepeat, TerrainRenderingPlugin, TerrainSurface};
use voxel_core::{lighting::Skylight, mesh_chunk_lit};

#[derive(Resource)]
struct Scene(String);

fn main() {
    let scene = std::env::args().nth(1).unwrap_or_else(|| "one".into());
    assert!(["dark", "one", "several", "removed", "wall", "seam"].contains(&scene.as_str()));
    let mut app = App::new();
    app.insert_resource(Scene(scene))
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustCraft Slice 20 lighting gallery".into(),
                resolution: (960, 640).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TerrainRenderingPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, capture);
    // Example crates have their own embedded namespace. Alias the unchanged
    // production shader URL so this fixture exercises the real material.
    app.world()
        .resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
        .insert_asset(
            std::path::PathBuf::new(),
            std::path::Path::new("rustcraft/terrain.wesl"),
            include_bytes!("../src/terrain.wesl"),
        );
    app.run();
}

fn setup(
    mut commands: Commands,
    scene: Res<Scene>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainSurface>>,
) {
    let w = lighting_fixture::room(&scene.0);
    let targets = w.chunks().map(|(p, _)| p).collect();
    let light = Skylight::for_chunks(&w, &targets);
    let atlas = images.add(terrain_texture::atlas_image());
    let material = materials.add(TerrainSurface {
        base: StandardMaterial {
            unlit: true,
            ..default()
        },
        extension: AtlasRepeat {
            atlas,
            atmosphere: atmosphere::AtmosphereUniform::disabled(),
        },
    });
    for (cp, _) in w.chunks() {
        let m = mesh_chunk_lit(&w, cp, &light);
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
            Transform::from_translation(Vec3::from_array(cp.0.map(|a| (a * 16) as f32))),
        ));
    }
    let (eye, target) = (Vec3::new(9., 4., 6.), Vec3::new(-2., 2., -3.));
    commands.spawn((
        Camera3d::default(),
        bevy::render::view::Tonemapping::Reinhard,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
    ));
}

fn capture(mut commands: Commands, mut frames: Local<u32>) {
    *frames += 1;
    if *frames == 60
        && let Some(path) = std::env::args().nth(2)
    {
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
