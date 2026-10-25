import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_vertex_output,
    pbr_functions::main_pass_post_lighting_processing,
};
import bevy_pbr::render::mesh_view_bindings::view;
import rustcraft::atmosphere::{AtmosphereUniform, terrain_fog};
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> atmosphere: AtmosphereUniform;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var<uniform> effect: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var water: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var water_sampler: sampler;
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_vertex_output(in, front, false);
    // Quantized UV scroll retains the original pixel grid. No mesh updates.
    let scroll = floor(effect.x * 4.0) / 16.0;
    // Side UV V increases downward, so negative sampling motion flows downward.
    let uv = fract(in.uv + vec2<f32>(scroll * 0.25, -scroll));
    let texel = textureSampleLevel(water, water_sampler, uv, 0.0);
    let brightness = max(in.color.r * atmosphere.sun.w, in.color.g);
    let color = vec4<f32>(texel.rgb * brightness, in.color.a);
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, terrain_fog(atmosphere, color, in.world_position.xyz - view.world_position.xyz, in.color.rgb));
    return out;
}
