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

// The map's water, for caustics on the bed under it; strength 0 = none.
struct WaterGlobals {
    level: f32,
    time: f32,
    strength: f32,
    cell_size: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightUniforms;
@group(0) @binding(3) var<uniform> fog: FogUniforms;
@group(0) @binding(7) var<uniform> water_globals: WaterGlobals;
@group(1) @binding(0) var ground_texture: texture_2d<f32>;
@group(1) @binding(1) var ground_sampler: sampler;
@group(2) @binding(0) var lightmap_texture: texture_2d<f32>;
@group(2) @binding(1) var lightmap_sampler: sampler;

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn apply_fog(color: vec3<f32>, view_pos: vec3<f32>) -> vec3<f32> {
    if (fog.enabled <= 0.0) {
        return color;
    }
    let fog_amount = clamp(
        (length(view_pos) - fog.near) / (fog.far - fog.near),
        0.0,
        1.0,
    );
    return mix(color, fog.color.rgb, fog_amount);
}

fn caustic_hash(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// One layer of caustic light: the thin network where two Voronoi cells meet (F2 - F1 small), cell points
// drifting so the network wanders.
fn caustic_layer(p: vec2<f32>, t: f32) -> f32 {
    let ip = floor(p);
    let fp = p - ip;
    var f1 = 8.0;
    var f2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let nb = vec2<f32>(f32(x), f32(y));
            let h = caustic_hash(ip + nb);
            let o = 0.5 + 0.4 * sin(t + 6.2831 * h);
            let d = length(nb + o - fp);
            if (d < f1) {
                f2 = f1;
                f1 = d;
            } else if (d < f2) {
                f2 = d;
            }
        }
    }
    return 1.0 - smoothstep(0.0, 0.14, f2 - f1);
}

// Caustics on ground under the water, fading in from the waterline and out with depth.
fn caustics(world: vec3<f32>) -> f32 {
    let depth = (world.y - water_globals.level) / water_globals.cell_size;
    if (water_globals.strength <= 0.0 || depth <= 0.0) {
        return 0.0;
    }
    let p = world.xz / water_globals.cell_size * 0.8;
    let t = water_globals.time * 0.6;
    let a = caustic_layer(p, t);
    let b = caustic_layer(vec2<f32>(p.x * 0.866 - p.y * 0.5, p.x * 0.5 + p.y * 0.866) * 1.3 + 17.0, t * 1.3);
    let fade = smoothstep(0.0, 0.3, depth) * exp(-depth * 0.45);
    return max(a, b) * fade * water_globals.strength;
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) lightmap_coord: vec2<f32>,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) lightmap_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) world_position: vec3<f32>,
    @location(5) view_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.tex_coord = in.tex_coord;
    out.lightmap_coord = in.lightmap_coord;
    out.normal = in.normal;
    out.color = in.color;
    out.world_position = in.position;
    let view_pos = camera.view * vec4<f32>(in.position, 1.0);
    out.view_pos = view_pos.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ground_texture, ground_sampler, in.tex_coord);

    // It seems map makers fill the void around a map with cells whose UVs point at a
    // colour-keyed patch, so a cell that keys out must leave the background.
    if tex_color.a < 0.81 {
        discard;
    }

    let lightmap = textureSample(lightmap_texture, lightmap_sampler, in.lightmap_coord);

    let sunlight = light.diffuse_color.rgb;
    let ambient = light.ambient_color.rgb;

    let n_dot_l = max(dot(normalize(in.normal), normalize(light.light_dir.xyz)), 0.0);
    let shadow = lightmap.a;
    let combined_light =
        clamp(sunlight * n_dot_l + ambient, vec3<f32>(0.0), vec3<f32>(1.0)) * shadow;

    var color = clamp(
        in.color.rgb * combined_light * linear_to_srgb(tex_color.rgb) + lightmap.rgb,
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // sunlight focused by the waves, where the sun reaches the bed
    color += sunlight * n_dot_l * shadow * caustics(in.world_position);
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    color = apply_fog(color, in.view_pos);

    let alpha = tex_color.a * in.color.a;

    return vec4<f32>(color, alpha);
}
