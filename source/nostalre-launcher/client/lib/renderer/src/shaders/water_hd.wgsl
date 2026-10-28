// HD water. The surface is flat at the RSW level; the waves live in the normal, built from layered value
// noise and its analytic gradient. It refracts a copy of the finished opaque scene and reads the depth
// buffer for how much water lies in front of what it shows: that depth drives absorption, colour, wave
// size, reflectivity and the shore. Emissive types (lava) glow instead.
//
// World space: Y grows downwards (up is -Y), lengths in RO units; the shading works in cells.
// Colours: the scene is gamma encoded like the rest of the renderer; the water works in linear and
// encodes its result again.

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
};

struct LightUniforms {
    light_dir: vec4<f32>,
    diffuse_color: vec4<f32>,
    ambient_color: vec4<f32>,
    shadow_strength: f32,
};

struct FogUniforms {
    color: vec4<f32>,
    near: f32,
    far: f32,
    enabled: f32,
};

struct WaterUniforms {
    wave_height: f32,
    wave_pitch_per_unit: f32,
    wave_offset: f32,
    opacity: f32,
    ambient_tint: f32,
    time: f32,
    wave_speed: f32,
    emissive: f32,
    // linear colours; tint.a = how strongly the water's own colour fills in what it absorbed
    tint: vec4<f32>,
    deep: vec4<f32>,
    // absorption per cell of water, per channel
    absorb: vec4<f32>,
    // width, height, near, far
    screen: vec4<f32>,
    level: f32,
    cell_size: f32,
    // NOSTALRE_WATER_DEBUG: 0 off, 1 depth, 2 normal, 3 fresnel, 4 refracted, 5 reflected, 6 glitter
    debug: f32,
    pad1: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightUniforms;
@group(0) @binding(3) var<uniform> fog: FogUniforms;
@group(1) @binding(0) var water_texture: texture_2d<f32>;
@group(1) @binding(1) var water_sampler: sampler;
@group(2) @binding(0) var<uniform> water: WaterUniforms;
@group(3) @binding(0) var scene_color: texture_2d<f32>;
@group(3) @binding(1) var scene_depth: texture_depth_2d;
@group(3) @binding(2) var scene_sampler: sampler;

const UP: vec3<f32> = vec3<f32>(0.0, -1.0, 0.0);

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn apply_fog(color: vec3<f32>, view_pos: vec3<f32>) -> vec3<f32> {
    if (fog.enabled <= 0.0) {
        return color;
    }
    let fog_amount = clamp((length(view_pos) - fog.near) / (fog.far - fog.near), 0.0, 1.0);
    return mix(color, fog.color.rgb, fog_amount);
}

// Distance along the view axis of a depth buffer value (wgpu's 0..1 depth, perspective_rh).
fn view_depth(d: f32) -> f32 {
    let n = water.screen.z;
    let f = water.screen.w;
    return n * f / (f - d * (f - n));
}

// A sin-free hash: stable on mobile GPUs at large inputs.
fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise with a quintic fade: (value, d/dx, d/dy).
fn noised(p: vec2<f32>) -> vec3<f32> {
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    let k1 = b - a;
    let k2 = c - a;
    let k4 = a - b - c + d;
    let value = a + k1 * u.x + k2 * u.y + k4 * u.x * u.y;
    let grad = du * vec2<f32>(k1 + k4 * u.y, k2 + k4 * u.x);
    return vec3<f32>(value, grad);
}

// The sky the surface reflects: the map's fog colour when it has fog, a pale blue otherwise, deeper
// toward the zenith, with a glow around the sun. `up` is how far the ray points up (0 horizon, 1 zenith).
fn sky_colour(up: f32, toward_sun: f32, lit: vec3<f32>) -> vec3<f32> {
    let horizon = select(vec3<f32>(0.62, 0.74, 0.86), srgb_to_linear(fog.color.rgb), fog.enabled > 0.0);
    let zenith = horizon * vec3<f32>(0.55, 0.72, 1.0);
    let sun = srgb_to_linear(light.diffuse_color.rgb) * pow(max(toward_sun, 0.0), 24.0) * 0.5;
    return mix(horizon, zenith, up) * lit + sun;
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) floor_y: f32,
    @location(3) phase_pos: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) view_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var pos = in.position;
    pos.y = water.level;
    out.clip_position = camera.view_proj * vec4<f32>(pos, 1.0);
    out.tex_coord = in.tex_coord;
    out.world = pos;
    out.view_pos = (camera.view * vec4<f32>(pos, 1.0)).xyz;
    return out;
}

// Lava: the animated texture, brightened, with slow flowing hot veins; it ignores the scene light.
fn lava(in: VertexOutput, p: vec2<f32>, t: f32) -> vec4<f32> {
    let tex = textureSample(water_texture, water_sampler, in.tex_coord).rgb;
    let flow = noised(p * 0.35 + vec2<f32>(t * 0.04, t * 0.025)).x;
    let flow2 = noised(p * 0.9 + vec2<f32>(-t * 0.03, t * 0.05) + 57.0).x;
    let pulse = 0.9 + 0.1 * sin(t * 1.4);
    var col = tex * 1.25 * pulse;
    col += vec3<f32>(1.0, 0.42, 0.05) * pow(flow, 3.0) * 0.7;
    col += vec3<f32>(0.8, 0.25, 0.02) * pow(flow2, 3.0) * 0.4;
    let out = apply_fog(linear_to_srgb(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), in.view_pos);
    return vec4<f32>(out, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cell = water.cell_size;
    let t = water.time;
    let p = in.world.xz / cell;
    let screen = water.screen.xy;
    let uv = in.clip_position.xy / screen;
    let pixel = vec2<i32>(in.clip_position.xy);

    // ---- waves: three octaves for the shape, the finest also breaks up refraction and the sun path,
    // a fourth only modulates the glitter
    let speed = 0.12 + 0.18 * log(1.0 + clamp(water.wave_speed, 0.0, 8.0)) / log(9.0);
    let n1 = noised(p * 0.55 + vec2<f32>(t * speed * 0.9, t * speed * 0.6));
    let n2 = noised(p * 1.35 + vec2<f32>(-t * speed * 0.7, t * speed * 0.8) + 37.0);
    let n3 = noised(p * 3.1 + vec2<f32>(t * speed * 1.4, -t * speed * 1.1) + 91.0);
    let n4 = noised(p * 6.5 + vec2<f32>(t * speed * 2.0, -t * speed * 1.6) + 173.0);

    // what lies behind the surface, straight through
    let lin_water = view_depth(in.clip_position.z);
    let depth_straight = max(0.0, view_depth(textureLoad(scene_depth, pixel, 0)) - lin_water) / cell;

    // shallow water cannot carry big waves: the swell grows with depth
    let calm = smoothstep(0.0, 1.2, depth_straight);
    let amp = mix(0.08, 0.30, calm);
    let slope = (n1.yz * 0.55 + n2.yz * 1.35 * 0.35 + n3.yz * 3.1 * 0.12) * amp;
    let n = normalize(vec3<f32>(-slope.x, -1.0, -slope.y));
    let ripple = n3.yz * 3.1 * 0.05;

    let v = normalize(camera.eye_pos.xyz - in.world);
    let l = normalize(light.light_dir.xyz);
    let sun_up = max(dot(l, UP), 0.0);
    let n_dot_v = max(dot(n, v), 0.0);

    // ---- sun glitter, before any branch: fwidth needs uniform control flow
    let n_spec = normalize(n + vec3<f32>(ripple.x, 0.0, ripple.y) * calm);
    let h = normalize(v + l);
    // sub-pixel normal variation is roughness: widen the lobe instead of point-sampling it
    let normal_var = fwidth(n_spec.x) + fwidth(n_spec.z);
    let spec_exp = mix(110.0, 8.0, clamp(normal_var * 6.0, 0.0, 1.0));
    let sheen = pow(max(dot(n_spec, h), 0.0), spec_exp);
    let glint_aa = max(fwidth(n4.x), 0.001);
    let sparkle = smoothstep(0.45 - glint_aa, 0.9 + glint_aa, n4.x);

    if (water.emissive > 0.5) {
        return lava(in, p, t);
    }

    // ---- refraction: offset by the wave, unless that lands on something in front of the water
    let refract_uv = clamp(uv + n.xz * 0.025 * smoothstep(0.0, 0.5, depth_straight), vec2<f32>(0.001), vec2<f32>(0.999));
    let refract_pixel = vec2<i32>(refract_uv * screen);
    let depth_refract = max(0.0, view_depth(textureLoad(scene_depth, refract_pixel, 0)) - lin_water) / cell;
    let use_refract = depth_refract > 0.05;
    let scene_uv = select(uv, refract_uv, use_refract);
    let depth = select(depth_straight, depth_refract, use_refract);
    let scene = srgb_to_linear(textureSampleLevel(scene_color, scene_sampler, scene_uv, 0.0).rgb);

    let shore = smoothstep(0.0, 0.12, depth);
    // the scene light the water's own colours take (the refracted scene is lit already)
    let lit = srgb_to_linear(clamp(light.ambient_color.rgb + light.diffuse_color.rgb * sun_up, vec3<f32>(0.0), vec3<f32>(1.0)));

    // ---- body: Beer-Lambert absorption, then the water's colour scattering in where light was lost;
    // shallow water keeps its tint, deep water turns to the deep colour
    let absorbed = scene * exp(-water.absorb.rgb * depth);
    let body = mix(water.tint.rgb, water.deep.rgb, smoothstep(0.0, 2.5, depth)) * lit;
    let scatter = (1.0 - exp(-depth * 0.9)) * water.tint.a;
    let refracted = mix(absorbed, body, scatter);

    // ---- reflection: RO's camera never gets near grazing (NdotV stays around 0.45..0.9), so the
    // reachable band is spread over a useful range rather than read off the bottom of a physical curve;
    // shallow water reflects less so the bed shows through
    let fresnel = mix(0.10, 0.45, smoothstep(0.92, 0.45, n_dot_v)) * mix(0.35, 1.0, smoothstep(0.0, 2.0, depth));
    let r = reflect(-v, n);
    let sky = sky_colour(clamp(dot(r, UP), 0.0, 1.0), dot(r, l), lit);
    // a reflection belongs to the water: it takes on some of the body colour
    let reflected = mix(sky, sky * (water.tint.rgb * 0.5 + 0.5), 0.35);
    var col = mix(refracted, reflected, fresnel);

    // ---- wave shading: faces toward the sun lighter, steep faces darker, centred on 1
    let flat_n_dot_l = sun_up;
    let face = dot(n, l) - flat_n_dot_l;
    let steep = 1.0 - max(dot(n, UP), 0.0);
    col *= clamp(1.0 + face * 1.1 - steep * 1.6, 0.6, 1.4);

    // ---- sun glitter: the swell picks the sun path, the fine layer breaks it into sparkles, which only
    // deeper water has; reflectivity gates it like any reflection
    let glitter = min(sheen * mix(1.0, 0.2 + 0.8 * sparkle, calm), 0.6) * clamp(fresnel / 0.45, 0.0, 1.0);
    col += srgb_to_linear(light.diffuse_color.rgb) * glitter * smoothstep(0.0, 0.25, sun_up) * 1.6;

    // ---- shore foam: patchy, drifting, only where the ground rises to the surface
    let foam_band = 1.0 - smoothstep(0.05, 0.45, depth);
    let foam_noise = noised(p * 2.2 + vec2<f32>(t * 0.05, -t * 0.04)).x * 0.65
        + noised(p * 5.3 + vec2<f32>(-t * 0.08, t * 0.06) + 11.0).x * 0.35;
    let foam = foam_band * smoothstep(0.42, 0.72, foam_noise);
    col = mix(col, vec3<f32>(0.86, 0.91, 0.96) * lit, foam * 0.55);

    let debug = i32(water.debug);
    if (debug > 0) {
        var shown = vec3<f32>(depth / 4.0);
        switch debug {
            case 2: { shown = n * vec3<f32>(0.5, -0.5, 0.5) + 0.5; }
            case 3: { shown = vec3<f32>(fresnel); }
            case 4: { shown = linear_to_srgb(refracted); }
            case 5: { shown = linear_to_srgb(reflected); }
            case 6: { shown = vec3<f32>(glitter); }
            default: {}
        }
        return vec4<f32>(shown, 1.0);
    }

    let out = apply_fog(linear_to_srgb(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), in.view_pos);
    return vec4<f32>(out, shore);
}
