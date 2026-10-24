//! Terrain-only repeat sampling; UI and held blocks retain StandardMaterial UVs.
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
pub(crate) type TerrainSurface = ExtendedMaterial<StandardMaterial, AtlasRepeat>;
#[derive(Asset, AsBindGroup, Reflect, Clone, Debug)]
pub(crate) struct AtlasRepeat {
    #[texture(100)]
    #[sampler(101)]
    pub atlas: Handle<Image>,
}
impl MaterialExtension for AtlasRepeat {
    fn fragment_shader() -> ShaderRef {
        "embedded://rustcraft/terrain.wesl".into()
    }
}
pub(crate) struct TerrainRenderingPlugin;
impl Plugin for TerrainRenderingPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "terrain.wesl");
        app.add_plugins(MaterialPlugin::<TerrainSurface>::default());
    }
}
