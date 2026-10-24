//! Fixed-camera renderer fixtures. Run with flat, structure, room or trees.
//! Uses the production terrain material/atlas; never opens or modifies saves.
#[path = "../src/terrain_material.rs"]
mod terrain_material;
#[path = "../src/terrain_texture.rs"]
mod terrain_texture;

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use terrain_material::{AtlasRepeat, TerrainRenderingPlugin, TerrainSurface};
use voxel_core::{Block, BlockPos, World, lighting::Skylight, mesh_chunk_lit};

#[derive(Resource)]
struct Scene(String);

fn main() {
    let scene = std::env::args().nth(1).unwrap_or_else(|| "flat".into());
    assert!(["flat", "structure", "room", "trees"].contains(&scene.as_str()));
    let mut app = App::new();
    app.insert_resource(Scene(scene))
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustCraft AO gallery".into(),
                resolution: (960, 640).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TerrainRenderingPlugin)
        .add_systems(Startup, setup);
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
    let mut w = World::default();
    let mut put = |p, b| {
        w.set_block(BlockPos(p), b).unwrap();
    };
    for x in -12..12 {
        for z in -12..12 {
            put(
                [x, 0, z],
                if scene.0 == "trees" || scene.0 == "flat" {
                    Block::Grass
                } else {
                    Block::Stone
                },
            );
        }
    }
    match scene.0.as_str() {
        "flat" => {
            for p in [[0, 1, 0], [1, 1, 0], [1, 2, 0], [-3, 1, 2], [0, 1, -1]] {
                put(p, Block::Stone);
            }
        }
        "structure" => {
            // Two intersecting walls and a stepped overhang across X/Z=0.
            for y in 1..5 {
                for a in -5..5 {
                    put([a, y, -3], Block::Stone);
                    put([-5, y, a], Block::Stone);
                }
            }
            for x in -5..0 {
                for z in -3..1 {
                    put([x, 5, z], Block::Stone);
                }
            }
            for p in [[0, 1, 0], [0, 1, 1], [0, 2, 0], [1, 1, 0]] {
                put(p, Block::Stone);
            }
        }
        "room" => {
            // Closed back/sides/roof, open front and one skylight aperture.
            for x in -5..6 {
                for z in -6..7 {
                    if [x, z] != [1, 1] {
                        put([x, 6, z], Block::Stone);
                    }
                    for y in 1..6 {
                        if x == -5 || x == 5 || z == -6 {
                            put([x, y, z], Block::Stone);
                        }
                    }
                }
            }
            for p in [[-4, 1, -5], [-4, 2, -5], [-3, 1, -5], [2, 1, -4]] {
                put(p, Block::Stone);
            }
        }
        "trees" => {
            for [tx, tz] in [[0, 0], [-5, -4], [4, -5]] {
                for y in 1..7 {
                    put([tx, y, tz], Block::Wood);
                }
                for y in 4..8 {
                    for x in tx - 2..=tx + 2 {
                        for z in tz - 2..=tz + 2 {
                            if x != tx || z != tz || y >= 7 {
                                put([x, y, z], Block::Leaves);
                            }
                        }
                    }
                }
            }
        }
        _ => unreachable!(),
    }
    let targets = w.chunks().map(|(p, _)| p).collect();
    let light = Skylight::for_chunks(&w, &targets);
    let atlas = images.add(terrain_texture::atlas_image());
    let material = materials.add(TerrainSurface {
        base: StandardMaterial {
            unlit: true,
            ..default()
        },
        extension: AtlasRepeat { atlas },
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
    let (eye, target) = match scene.0.as_str() {
        "room" => (Vec3::new(3.5, 3.5, 6.), Vec3::new(-1., 2., -3.)),
        "trees" => (Vec3::new(11., 8., 14.), Vec3::new(-1., 3., -1.)),
        _ => (Vec3::new(9., 7., 12.), Vec3::new(-1., 1.5, -1.)),
    };
    commands.spawn((
        Camera3d::default(),
        bevy::render::view::Tonemapping::Reinhard,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
    ));
}
