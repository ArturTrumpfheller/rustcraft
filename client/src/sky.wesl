import bevy_pbr::render::{
    mesh_view_bindings::view,
    pbr_types::pbr_input_new,
    pbr_functions::main_pass_post_lighting_processing,
};
import rustcraft::atmosphere::{AtmosphereUniform, sky_color};
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> atmosphere: AtmosphereUniform;
struct SkyOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ray: vec3<f32>,
};
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> SkyOutput {
    // Mesh buffers are suballocated: the first vertex need not be zero. Any
    // cyclic permutation of these three corners is the same full-screen triangle.
    let corner = index % 3u;
    let uv = vec2<f32>(f32(corner & 1u), f32(corner >> 1u));
    let ndc = uv * 4.0 - vec2<f32>(1.0);
    let local_ray = view.view_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    var out: SkyOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.ray = (view.world_from_view * vec4<f32>(local_ray.xyz, 0.0)).xyz;
    return out;
}
@fragment
fn fragment(in: SkyOutput) -> @location(0) vec4<f32> {
    var pbr = pbr_input_new();
    pbr.frag_coord = in.position;
    return main_pass_post_lighting_processing(pbr, vec4<f32>(sky_color(atmosphere, normalize(in.ray)), 1.0));
}
