import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_vertex_output,
    pbr_functions::main_pass_post_lighting_processing,
};
import bevy_pbr::render::mesh_view_bindings::view;
import rustcraft::atmosphere::{AtmosphereUniform, terrain_fog};
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> atmosphere: AtmosphereUniform;
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var atlas_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    // Repeat in tile-local space, never across the padded atlas. Explicit LOD0
    // avoids discontinuous fract derivatives; the original image has one mip,
    // nearest filters, no anisotropy, and eight extruded texels per edge.
    let uv = in.uv_b + fract(in.uv) * vec2<f32>(16.0 / 128.0, 16.0 / 64.0);
    var pbr = pbr_input_from_vertex_output(in, front, false);
    pbr.material.base_color = textureSampleLevel(atlas, atlas_sampler, uv, 0.0) * in.color;
    var out: FragmentOutput;
    let shaded = terrain_fog(atmosphere, pbr.material.base_color, in.world_position.xyz - view.world_position.xyz, in.color.rgb);
    out.color = main_pass_post_lighting_processing(pbr, shaded);
    return out;
}
