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
struct BodySample {
    uv: vec2<f32>,
    coverage: f32,
};
fn square_body(ray: vec3<f32>, center: vec3<f32>, size: f32, right: vec3<f32>, up: vec3<f32>, forward: vec3<f32>) -> BodySample {
    let depth = dot(center, forward);
    let ray_depth = dot(ray, forward);
    if center.y < -size || depth <= 0.0 || ray_depth <= 0.0 {
        return BodySample(vec2<f32>(0.5), 0.0);
    }
    // Reuse the same camera-facing projection for coverage and pixel-art UVs.
    let offset = vec2<f32>(dot(ray,right),dot(ray,up))/ray_depth
               - vec2<f32>(dot(center,right),dot(center,up))/depth;
    let extent = max(abs(offset.x),abs(offset.y));
    let uv = vec2<f32>(0.5) + offset * vec2<f32>(0.5,-0.5) / size;
    // Fade only below the horizon before the existing center cull. Finite
    // terrain can expose this last strip; a fade avoids an abrupt pop without
    // cutting a horizontal edge through a body that is still above terrain.
    let horizon = smoothstep(-size, 0.0, center.y);
    return BodySample(uv, (1.0 - smoothstep(size * 0.96, size, extent)) * horizon);
}

// Original RustCraft 16x16 moon, MIT OR Apache-2.0. Four scene-linear shades:
// 0 off-white highlands, 1 gray plain, 2 blue-gray crater rim, 3 darker basin.
// Each row packs sixteen 2-bit indices, leftmost pixel in its lowest bits.
// The unpacked rows alongside the constants are the editable pixel-art source.
const MOON_ROWS: array<u32,16> = array<u32,16>(
    0x00555500u, // 0000111111110000
    0x05555550u, // 0011111111111100
    0x15556a54u, // 0111222111111110
    0x5555af94u, // 0112332211111111
    0x5556bfe5u, // 1123333221111111
    0x5a56bfe5u, // 1123333221112211
    0x6f95af95u, // 1112332211123321
    0x6b956a55u, // 1111222111123221
    0x5a555555u, // 1111111111112211
    0x555a9555u, // 1111111222111111
    0x556be554u, // 0111112332211111
    0x55aff950u, // 0011123333221111
    0x15aff950u, // 0011123333221110
    0x056be540u, // 0001112332211100
    0x015a9500u, // 0000111222111000
    0x00155400u, // 0000011111100000
);
const MOON_PALETTE: array<vec3<f32>,4> = array<vec3<f32>,4>(
    vec3<f32>(0.78,0.82,0.88),
    vec3<f32>(0.60,0.66,0.76),
    vec3<f32>(0.39,0.45,0.56),
    vec3<f32>(0.23,0.28,0.37),
);
fn moon_texel(uv: vec2<f32>) -> vec3<f32> {
    // Integer point lookup: no filtering, extra image/sampler or noise function.
    let cell = vec2<u32>(clamp(floor(uv * 16.0), vec2<f32>(0.0), vec2<f32>(15.0)));
    let index = (MOON_ROWS[cell.y] >> (cell.x * 2u)) & 3u;
    return MOON_PALETTE[index];
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
    color = mix(color, vec3<f32>(3.0,2.4,1.5), sun.coverage);
    // Only the visible moon's small screen footprint needs the palette lookup.
    if moon.coverage > 0.0 {
        color = mix(color, moon_texel(moon.uv), moon.coverage);
    }
    return color;
}
