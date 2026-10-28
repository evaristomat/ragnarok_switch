struct UiUniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> ui: UiUniforms;
@group(1) @binding(0) var ui_texture: texture_2d<f32>;
@group(1) @binding(1) var ui_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
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
    let ndc_x = in.position.x / ui.screen_size.x * 2.0 - 1.0;
    let ndc_y = 1.0 - in.position.y / ui.screen_size.y * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;
    return out;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn load_gamma(coord: vec2<i32>, dims: vec2<i32>) -> vec4<f32> {
    let c = clamp(coord, vec2<i32>(0), dims - vec2<i32>(1));
    let t = textureLoad(ui_texture, c, 0);
    return vec4<f32>(linear_to_srgb(t.rgb), t.a);
}

/// Bilinear over gamma bytes rather than the sampler's linear blend: a 1px
/// feature half covering a pixel has to land on 128, not 188, or thin strokes
/// wash out. Magnified texels also get their interior pinned to one value so a
/// fractional UI scale keeps hard edges.
fn sample_sharp_gamma(uv: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(ui_texture, 0));
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

    let t = floor(px) + snapped - 0.5;
    let base = floor(t);
    let w = t - base;
    let idims = vec2<i32>(textureDimensions(ui_texture, 0));
    let i0 = vec2<i32>(base);
    let c00 = load_gamma(i0, idims);
    let c10 = load_gamma(i0 + vec2<i32>(1, 0), idims);
    let c01 = load_gamma(i0 + vec2<i32>(0, 1), idims);
    let c11 = load_gamma(i0 + vec2<i32>(1, 1), idims);
    return mix(mix(c00, c10, w.x), mix(c01, c11, w.x), w.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = sample_sharp_gamma(in.tex_coord);
    return vec4<f32>(tex.rgb * in.color.rgb, tex.a * in.color.a);
}
