//! Session-owned item presentation. World items survive streaming; these entities
//! and their small shared mesh cache/material are only a bounded resident view.
use super::{RenderOrigin, SceneSetup, TerrainMaterial, VoxelWorld};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::collections::BTreeMap;
use voxel_core::BlockPos;

#[derive(Resource, Default)]
pub(super) struct ItemRenders {
    meshes: BTreeMap<voxel_core::inventory::Item, Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    entities: BTreeMap<BlockPos, (Entity, voxel_core::inventory::Item)>,
}
pub(super) fn setup(mut commands: Commands) {
    commands.insert_resource(ItemRenders::default());
}
fn ensure_assets(
    renders: &mut ItemRenders,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain: &TerrainMaterial,
    item: voxel_core::inventory::Item,
) {
    if renders.meshes.contains_key(&item) {
        return;
    }
    let source = voxel_core::torch_mesh();
    let tile = voxel_core::texture::Tile::Torch.uv([0, 0]);
    let uv = source
        .uvs
        .into_iter()
        .map(|uv| [tile[0] + uv[0] * 16. / 128., tile[1] + uv[1] * 16. / 192.])
        .collect::<Vec<_>>();
    let mesh = meshes.add(
        if let Some(block) = item.build().map(|b| b.block()).filter(|b| b.is_plant()) {
            crate::viewmodel::plant_item_mesh(block)
        } else {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, source.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32(source.indices))
        },
    );
    renders.meshes.insert(item, mesh);
    if renders.material.is_none() {
        let material = materials.add(StandardMaterial {
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            double_sided: true,
            unlit: true,
            base_color_texture: Some(terrain.1.clone()),
            ..default()
        });
        renders.material = Some(material);
    }
}
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ItemAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    terrain: Res<'w, TerrainMaterial>,
}

pub(super) fn draw(
    mut commands: Commands,
    world: Res<VoxelWorld>,
    origin: Res<RenderOrigin>,
    renders: Option<ResMut<ItemRenders>>,
    mut transforms: Query<&mut Transform>,
    mut assets: ItemAssets,
) {
    let Some(mut renders) = renders else {
        return;
    };
    if world.0.dropped_items().is_empty() && renders.entities.is_empty() {
        return;
    }
    let mut desired = BTreeMap::new();
    // Resident chunk index only; no scan through saved distant items or regions.
    for (cp, _) in world.0.chunks() {
        for (p, stack) in world.0.dropped_items().in_chunk(cp) {
            if stack.item() == voxel_core::inventory::Item::Torch
                || stack.item().build().is_some_and(|b| b.block().is_plant())
            {
                desired.insert(p, stack.item());
            }
        }
    }
    for &item in desired.values() {
        ensure_assets(
            &mut renders,
            &mut assets.meshes,
            &mut assets.materials,
            &assets.terrain,
            item,
        );
    }
    renders.entities.retain(|p, &mut (e, item)| {
        if desired.get(p) == Some(&item) {
            true
        } else {
            commands.entity(e).despawn();
            false
        }
    });
    for (p, item) in &desired {
        let transform = Transform::from_translation(
            Vec3::from_array(std::array::from_fn(|a| {
                (f64::from(p.0[a]) - origin.0[a]) as f32
            })) + Vec3::new(0.25, 0.15, 0.25),
        )
        .with_scale(Vec3::splat(0.5));
        if let Some(&(e, _)) = renders.entities.get(p) {
            if let Ok(mut old) = transforms.get_mut(e) {
                *old = transform;
            }
        } else {
            let e = commands
                .spawn((
                    crate::session::SessionEntity,
                    Mesh3d(renders.meshes[item].clone()),
                    MeshMaterial3d(renders.material.as_ref().unwrap().clone()),
                    transform,
                ))
                .id();
            renders.entities.insert(*p, (e, *item));
        }
    }
}
pub(super) fn cleanup(world: &mut World) {
    if let Some(renders) = world.remove_resource::<ItemRenders>() {
        for mesh in renders.meshes.values() {
            world.resource_mut::<Assets<Mesh>>().remove(mesh);
        }
        if let Some(material) = renders.material {
            world
                .resource_mut::<Assets<StandardMaterial>>()
                .remove(&material);
        }
    }
}

pub(super) fn register(app: &mut App) {
    app.add_systems(
        Startup,
        setup.after(SceneSetup).run_if(crate::session::has_world),
    )
    .add_systems(crate::session::SessionSetup, setup.after(SceneSetup))
    .add_systems(
        PostUpdate,
        draw.before(bevy::transform::TransformSystems::Propagate)
            .run_if(crate::session::has_world),
    );
}
