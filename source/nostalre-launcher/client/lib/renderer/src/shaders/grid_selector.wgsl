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

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> _light: LightUniforms;
@group(1) @binding(0) var grid_texture: texture_2d<f32>;
@group(1) @binding(1) var grid_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;
    return out;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(grid_texture, grid_sampler, in.tex_coord);
    let a = tex_color.a * in.color.a;
    if (a < 0.01) {
        discard;
    }
    return vec4<f32>(linear_to_srgb(tex_color.rgb) * in.color.rgb, a);
}
