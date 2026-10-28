struct SpriteUniforms {
    screen_size: vec2<f32>,
    zoom: f32,
    _pad: f32,
    pan: vec2<f32>,
    _pad2: vec2<f32>,
    world_light: vec4<f32>,
    fog_color: vec4<f32>,
    fog_near: f32,
    fog_far: f32,
    fog_enabled: f32,
    sharpen: f32,
    clip_near: f32,
    clip_far: f32,
    _pad4: vec2<f32>,
};

@group(0) @binding(0) var<uniform> sprite: SpriteUniforms;
@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

// Inverts ndc_z = far * (d - near) / (d * (far - near)).
fn eye_distance(ndc_z: f32) -> f32 {
    let near = sprite.clip_near;
    let far = sprite.clip_far;
    return (far * near) / max(far - ndc_z * (far - near), 1e-4);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn apply_fog(color: vec3<f32>, ndc_z: f32) -> vec3<f32> {
    if (sprite.fog_enabled <= 0.0) {
        return color;
    }
    let fog_amount = clamp(
        (eye_distance(ndc_z) - sprite.fog_near) / (sprite.fog_far - sprite.fog_near),
        0.0,
        1.0,
    );
    return mix(color, sprite.fog_color.rgb, fog_amount);
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) ndc_z: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let pos = (in.position.xy + sprite.pan) * sprite.zoom;
    let ndc_x = pos.x / sprite.screen_size.x * 2.0 - 1.0;
    let ndc_y = 1.0 - pos.y / sprite.screen_size.y * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, in.position.z, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;
    out.ndc_z = in.position.z;
    return out;
}

fn sharp_tex_coord(uv: vec2<f32>) -> vec2<f32> {
    let dims = vec2<f32>(textureDimensions(sprite_texture, 0));
    let px = uv * dims;
    let scale = max(
        vec2<f32>(
            1.0 / max(length(vec2<f32>(dpdx(px.x), dpdy(px.x))), 1e-4),
            1.0 / max(length(vec2<f32>(dpdx(px.y), dpdy(px.y))), 1e-4),
        ),
        vec2<f32>(1.0),
    );
    let offset = fract(px) - 0.5;
    let flat_region = 0.5 - 0.5 / scale;
    let snapped = (offset - clamp(offset, -flat_region, flat_region)) * scale + 0.5;
    return (floor(px) + snapped) / dims;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_coord = select(
        in.tex_coord,
        sharp_tex_coord(in.tex_coord),
        sprite.sharpen > 0.0,
    );
    let tex = textureSample(sprite_texture, sprite_sampler, tex_coord);
    if tex.a < 0.01 || in.color.a <= 0.0 {
        discard;
    }
    var color = linear_to_srgb(tex.rgb) * in.color.rgb * sprite.world_light.rgb;
    color = apply_fog(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), in.ndc_z);
    return vec4<f32>(color, tex.a * in.color.a);
}

// A weapon aura (nostalRE's own `WeaponLight::Aura`), drawn around the weapon's own shape. The quad is the weapon
// clip grown outward with its texture coordinates extended past 0..1, so the weapon keeps its size and the aura has
// room. The vertex colour carries: r the clock (phase of AURA_PERIOD seconds), g and b the inner and outer colours
// (5 bits a channel, whole numbers that survive interpolation), a the style (0 flame, 1 pulse).
const AURA_PERIOD: f32 = 100.0;
// one breath of the pulse, and the faint ring it sends outward
const PULSE_SECS: f32 = 1.6;
// how strong the pulse is off the weapon, on it, and its outgoing ring at its brightest; how fast the glow fades
// with the distance (texels)
const PULSE_GLOW: f32 = 0.6;
const PULSE_ON_BLADE: f32 = 0.45;
const PULSE_RING: f32 = 0.12;
const PULSE_FADE_TEXELS: f32 = 2.4;
// the eight directions, and the rings (texels) the pulse looks for the weapon on: at most 40 samples a pixel
const AURA_DIRS = array<vec2<f32>, 8>(
    vec2<f32>(1.0, 0.0), vec2<f32>(0.7071, 0.7071), vec2<f32>(0.0, 1.0), vec2<f32>(-0.7071, 0.7071),
    vec2<f32>(-1.0, 0.0), vec2<f32>(-0.7071, -0.7071), vec2<f32>(0.0, -1.0), vec2<f32>(0.7071, -0.7071),
);
const AURA_RADII = array<f32, 5>(1.5, 3.0, 5.0, 7.5, 10.0);

fn aura_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn aura_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(aura_hash(i), aura_hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(aura_hash(i + vec2<f32>(0.0, 1.0)), aura_hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn aura_fbm(p: vec2<f32>) -> f32 {
    return 0.5 * aura_noise(p) + 0.3 * aura_noise(p * 2.03) + 0.2 * aura_noise(p * 4.01);
}

fn aura_unpack(v: f32) -> vec3<f32> {
    let n = u32(round(v));
    return vec3<f32>(f32((n >> 10u) & 31u), f32((n >> 5u) & 31u), f32(n & 31u)) / 31.0;
}

// The weapon's opacity at `px` (texels), none outside its texture.
fn aura_weapon_alpha(px: vec2<f32>, dims: vec2<f32>) -> f32 {
    let uv = px / dims;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 0.0;
    }
    return textureSampleLevel(sprite_texture, sprite_sampler, uv, 0.0).a;
}

// Flames rising off the weapon: its shape just below each pixel, wobbling sideways, eaten into tongues by noise
// scrolling upward. Returns the flame's strength, 0..1.
fn aura_flame(px: vec2<f32>, dims: vec2<f32>, time: f32) -> f32 {
    var heat = 0.0;
    for (var k = 0; k < 8; k++) {
        let rise = f32(k) * 1.5;
        let wobble = (aura_fbm(vec2<f32>(px.x * 0.12, px.y * 0.1 + time * 2.5 + f32(k))) - 0.5) * 5.0;
        let falloff = 1.0 - f32(k) / 8.0;
        heat = max(heat, aura_weapon_alpha(px + vec2<f32>(wobble, rise), dims) * falloff);
    }
    let tongues = aura_fbm(vec2<f32>(px.x * 0.18, px.y * 0.14 + time * 4.0));
    return clamp(heat * (0.55 + tongues) - 0.35, 0.0, 1.0);
}

// A glow hugging the weapon: brightest on its edge, fading with the distance to it, breathing, with a faint ring
// sent outward every breath. Returns (how near the weapon: 0..1, strength: 0..1); discards what is out of reach.
fn aura_pulse(px: vec2<f32>, dims: vec2<f32>, time: f32) -> vec2<f32> {
    let breath = 0.8 + 0.2 * sin(time * 6.2832 / PULSE_SECS);
    if (aura_weapon_alpha(px, dims) > 0.3) {
        // on the weapon: hottest where a neighbour is clear, its edge
        var clear = 0.0;
        for (var d = 0; d < 8; d++) {
            clear = max(clear, 1.0 - aura_weapon_alpha(px + AURA_DIRS[d] * 1.5, dims));
        }
        let near = 0.35 + 0.65 * clear;
        return vec2<f32>(near, near * breath * PULSE_ON_BLADE);
    }
    // off it: the nearest ring that touches it, searched from the inside out, stopping at the first hit
    var dist = 99.0;
    for (var r = 0; r < 5 && dist > 90.0; r++) {
        for (var d = 0; d < 8; d++) {
            if (aura_weapon_alpha(px + AURA_DIRS[d] * AURA_RADII[r], dims) > 0.3) {
                dist = AURA_RADII[r];
                break;
            }
        }
    }
    if (dist > 90.0) {
        discard;
    }
    let near = exp(-dist / PULSE_FADE_TEXELS);
    let wave = fract(time / PULSE_SECS);
    let ring_at = 1.0 + wave * 9.0;
    let ring = exp(-(dist - ring_at) * (dist - ring_at) / 3.0) * (1.0 - wave) * PULSE_RING;
    return vec2<f32>(near, near * breath * PULSE_GLOW + ring);
}

@fragment
fn fs_weapon_aura(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = in.color.r * AURA_PERIOD;
    let inner = aura_unpack(in.color.g);
    let outer = aura_unpack(in.color.b);
    let dims = vec2<f32>(textureDimensions(sprite_texture, 0));
    let px = in.tex_coord * dims;
    var near = 0.0;
    var strength = 0.0;
    if (in.color.a < 0.5) {
        let flame = aura_flame(px, dims, time);
        near = flame;
        strength = flame;
    } else {
        let pulse = aura_pulse(px, dims, time);
        near = pulse.x;
        strength = pulse.y;
    }
    if (strength <= 0.005) {
        discard;
    }
    // outer at the fringe, inner where it is hottest
    let mid = mix(outer, inner, 0.5);
    let color = select(mix(mid, inner, (near - 0.5) * 2.0), mix(outer, mid, near * 2.0), near < 0.5);
    return vec4<f32>(color, clamp(strength, 0.0, 1.0));
}
