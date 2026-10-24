//! Retained, terrain-only daylight atmosphere. No simulation or world queries.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, CompareFunction, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
        SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

/// Scene-linear colors: both shaders apply the same view tonemapping afterwards.
#[derive(Clone, Copy, Debug, PartialEq, ShaderType, Reflect)]
pub(crate) struct AtmosphereUniform {
    /// RGB radiance; w is the elevation (ray.y) reaching the upper-sky color.
    pub zenith: Vec4,
    pub horizon: Vec4,
    /// Start/end in blocks, then low/full daylight thresholds in vertex RGB.
    pub fog: Vec4,
}
impl AtmosphereUniform {
    pub fn for_radius(radius: i32) -> Self {
        let end = radius.clamp(1, voxel_core::streaming::MAX_RADIUS) as f32
            * voxel_core::CHUNK_SIZE as f32;
        Self {
            zenith: Vec4::new(0.07, 0.28, 1.25, 0.5),
            horizon: Vec4::new(0.70, 1.20, 2.0, 1.),
            fog: Vec4::new(end * 0.55, end, 0.06, 0.28),
        }
    }
    pub fn disabled() -> Self {
        Self {
            fog: Vec4::ZERO,
            ..Self::for_radius(2)
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Clone, Debug)]
pub(crate) struct SkyMaterial {
    #[uniform(0)]
    pub atmosphere: AtmosphereUniform,
}
impl Material for SkyMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://rustcraft/sky.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://rustcraft/sky.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(false);
            depth.depth_compare = Some(CompareFunction::GreaterEqual);
        }
        Ok(())
    }
}

pub(crate) struct AtmospherePlugin;
impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "sky.wesl");
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default());
    }
}

#[derive(Component)]
pub(crate) struct Sky;
#[derive(Resource)]
pub(crate) struct SkyAssets {
    pub entity: Entity,
    pub material: Handle<SkyMaterial>,
}
/// One oversized triangle, fixed in clip space by its vertex shader. The mesh
/// transform is intentionally irrelevant; view rotation alone determines sky.
pub(crate) fn spawn_sky(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SkyMaterial>,
    atmosphere: AtmosphereUniform,
) -> SkyAssets {
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-1., -1., 0.], [3., -1., 0.], [-1., 3., 0.]],
    );
    let material = materials.add(SkyMaterial { atmosphere });
    let entity = commands
        .spawn((
            Sky,
            Name::new("Procedural sky"),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            NoFrustumCulling,
        ))
        .id();
    SkyAssets { entity, material }
}

pub(crate) fn cleanup(world: &mut World) {
    if let Some(sky) = world.remove_resource::<SkyAssets>() {
        world
            .resource_mut::<Assets<SkyMaterial>>()
            .remove(sky.material.id());
    }
    // SessionAssets already owns the mesh; SessionEntity owns the entity.
}

#[cfg(test)]
#[path = "atmosphere/tests.rs"]
mod tests;

impl Default for AtmosphereUniform {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Update retained uniforms only when appearance parameters actually change.
/// Returns the number of modified assets (zero on unchanged/moving frames).
pub(crate) fn update_materials(
    params: AtmosphereUniform,
    terrain: &Handle<crate::terrain_material::TerrainSurface>,
    sky: Option<&Handle<SkyMaterial>>,
    materials: &mut Assets<crate::terrain_material::TerrainSurface>,
    sky_materials: &mut Assets<SkyMaterial>,
) -> usize {
    let mut changed = 0;
    if materials
        .get(terrain)
        .is_some_and(|m| m.extension.atmosphere != params)
    {
        materials.get_mut(terrain).unwrap().extension.atmosphere = params;
        changed += 1;
    }
    if let Some(sky) = sky
        && sky_materials
            .get(sky)
            .is_some_and(|m| m.atmosphere != params)
    {
        sky_materials.get_mut(sky).unwrap().atmosphere = params;
        changed += 1;
    }
    changed
}
