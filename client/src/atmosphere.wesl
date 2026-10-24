// Original RustCraft daylight appearance, shared by terrain and sky.
struct AtmosphereUniform {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    fog: vec4<f32>,
};
fn sky_color(atmosphere: AtmosphereUniform, direction: vec3<f32>) -> vec3<f32> {
    let height = smoothstep(0.0, atmosphere.zenith.w, max(direction.y, 0.0));
    return mix(atmosphere.horizon.rgb, atmosphere.zenith.rgb, height);
}
fn terrain_fog(atmosphere: AtmosphereUniform, color: vec4<f32>, delta: vec3<f32>, shade: vec3<f32>) -> vec4<f32> {
    if atmosphere.fog.y <= atmosphere.fog.x { return color; }
    let distance = length(delta);
    let brightness = max(shade.r, max(shade.g, shade.b));
    if distance <= atmosphere.fog.x || brightness <= atmosphere.fog.z { return color; }
    let amount = smoothstep(atmosphere.fog.x, atmosphere.fog.y, distance);
    // Existing vertex RGB is a conservative visual daylight proxy. Fully dark
    // cave faces receive no blue daylight; no additional light query or attribute.
    let daylight = smoothstep(atmosphere.fog.z, atmosphere.fog.w, brightness);
    let direction = delta / max(distance, 0.0001);
    return vec4<f32>(mix(color.rgb, sky_color(atmosphere, direction), amount * daylight), color.a);
}
