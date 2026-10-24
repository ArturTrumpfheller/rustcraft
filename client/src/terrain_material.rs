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
    #[uniform(102)]
    pub atmosphere: crate::atmosphere::AtmosphereUniform,
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
        // Shared custom WESL imports need a retained, loaded module. Its
        // canonical namespace also works in the standalone rendering examples.
        app.world()
            .resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
            .insert_asset(
                std::path::PathBuf::new(),
                std::path::Path::new("rustcraft/atmosphere.wesl"),
                include_bytes!("atmosphere.wesl"),
            );
        let shader = app
            .world()
            .resource::<AssetServer>()
            .load("embedded://rustcraft/atmosphere.wesl");
        app.insert_resource(AtmosphereShader { _shader: shader });
        app.add_plugins(MaterialPlugin::<TerrainSurface>::default());
    }
}

#[derive(Resource)]
struct AtmosphereShader {
    _shader: Handle<Shader>,
}
