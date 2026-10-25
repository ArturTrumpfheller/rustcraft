// Original RustCraft daylight appearance, shared by terrain and sky.
struct AtmosphereUniform {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    fog: vec4<f32>,
    sun: vec4<f32>,
    celestial: vec4<f32>,
};
fn sky_color(atmosphere: AtmosphereUniform, direction: vec3<f32>) -> vec3<f32> {
    let height = smoothstep(0.0, atmosphere.zenith.w, max(direction.y, 0.0));
    return mix(atmosphere.horizon.rgb, atmosphere.zenith.rgb, height);
}
fn terrain_fog(atmosphere: AtmosphereUniform, color: vec4<f32>, delta: vec3<f32>, shade: vec3<f32>) -> vec4<f32> {
    if atmosphere.fog.y <= atmosphere.fog.x { return color; }
    let distance = length(delta);
    let brightness = shade.b;
    if distance <= atmosphere.fog.x || brightness <= atmosphere.fog.z { return color; }
    let amount = smoothstep(atmosphere.fog.x, atmosphere.fog.y, distance);
    // Geometric sky exposure is independent of time and torch light.
    // Sealed caves never receive exterior fog even beside a bright torch.
    let daylight = smoothstep(atmosphere.fog.z, atmosphere.fog.w, brightness);
    let direction = delta / max(distance, 0.0001);
    return vec4<f32>(mix(color.rgb, sky_color(atmosphere, direction), amount * daylight), color.a);
}

// Angular square billboards in world direction space. Translation, render-origin
// shifts and terrain depth never move celestial objects.
fn square_body(ray: vec3<f32>, center: vec3<f32>, size: f32, right: vec3<f32>, up: vec3<f32>, forward: vec3<f32>) -> f32 {
    let depth = dot(center, forward);
    let ray_depth = dot(ray, forward);
    if center.y < -size || depth <= 0.0 || ray_depth <= 0.0 { return 0.0; }
    // Camera-facing square with a world-space center and fixed angular size.
    let offset = vec2<f32>(dot(ray,right),dot(ray,up))/ray_depth
               - vec2<f32>(dot(center,right),dot(center,up))/depth;
    let extent = max(abs(offset.x),abs(offset.y));
    return 1.0 - smoothstep(size * 0.96, size, extent);
}
fn celestial_sky(a: AtmosphereUniform, ray: vec3<f32>, right: vec3<f32>, up: vec3<f32>, forward: vec3<f32>) -> vec3<f32> {
    var color = sky_color(a, ray);
    let sun = square_body(ray, a.sun.xyz, a.celestial.y, right, up, forward);
    let moon = square_body(ray, -a.sun.xyz, a.celestial.z, right, up, forward);
    // Stable sparse direction-space cells; no per-frame random values or assets.
    let cell = floor(ray * 170.0);
    let hash = fract(sin(dot(cell, vec3<f32>(127.1,311.7,74.7))) * 43758.5453);
    let offset = fract(ray * 170.0) - vec3<f32>(0.5);
    let star = select(0.0, 1.0, hash > 0.997 && dot(offset,offset) < 0.12);
    color += vec3<f32>(0.85,0.90,1.0) * star * a.celestial.x * smoothstep(0.0,0.12,ray.y);
    color = mix(color, vec3<f32>(3.0,2.4,1.5), sun);
    return mix(color, vec3<f32>(0.60,0.68,0.82), moon);
}
