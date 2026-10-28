use std::collections::HashMap;

use nostalre_effects::StrTurn;
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::str_effect::{EffectLayer, StrEffectFile};

use crate::camera::Camera;
use crate::effect_sprite::project_billboard;
use crate::mesh_batch::Z_QUAD_INDICES;
use crate::sprite::{SpriteBatch, SpriteVertex};
use crate::texture::TextureCache;

pub struct StrEffectEntry {
    pub str_file: StrEffectFile,
    /// `texture_paths[layer_idx][tex_idx]` = GRF path for that texture.
    pub texture_paths: Vec<Vec<String>>,
    /// Cloned bind groups for each texture, indexed parallel to `texture_paths`.
    /// `None` entries indicate the texture failed to load.
    pub bind_groups: Vec<Vec<Option<wgpu::BindGroup>>>,
}

/// Caches loaded STR effect files and their black-keyed textures.
/// Textures are loaded directly (not via [`TextureCache`]) so we can convert
/// the all-black background pixels into transparent ones, matching the
/// original game's additive-blend STR effect rendering.
pub struct StrEffectCache {
    entries: HashMap<String, StrEffectEntry>,
    filtering: bool,
    /// Names that failed to resolve once. Re-requesting one returns `false`
    /// without retrying the load or re-emitting the warning — stress spawns and
    /// per-frame respawns of asset-missing effects would otherwise flood the log.
    missing: std::collections::HashSet<String>,
}

impl Default for StrEffectCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StrEffectCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            filtering: true,
            missing: std::collections::HashSet::new(),
        }
    }

    /// Drops the loaded textures so the next spawn re-uploads them with the new
    /// filter. Names that failed to resolve stay remembered.
    pub fn set_filtering(&mut self, on: bool) {
        if self.filtering == on {
            return;
        }
        self.filtering = on;
        self.entries.clear();
    }

    pub fn load(
        &mut self,
        name: &str,
        aliases: &[&str],
        grf: &GrfArchive,
        texture_cache: &mut TextureCache,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> bool {
        if self.entries.contains_key(name) {
            return true;
        }
        if self.missing.contains(name) {
            return false;
        }
        let prepared = Self::prepare(
            name,
            aliases,
            grf,
            &texture_cache.bind_group_layout,
            device,
            queue,
            self.filtering,
        );
        self.install_prepared(name, prepared)
    }

    pub fn prepare(
        name: &str,
        aliases: &[&str],
        grf: &GrfArchive,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        filtering: bool,
    ) -> Option<StrEffectEntry> {
        let mut last_err: Option<String> = None;
        for candidate in std::iter::once(name).chain(aliases.iter().copied()) {
            match try_load(candidate, grf, layout, device, queue, filtering) {
                Ok(entry) => {
                    if candidate != name && nostalre_profiling::debug::trace_effects() {
                        tracing::info!("STR resolved via alias: {name} -> {candidate}");
                    }
                    return Some(entry);
                }
                Err(e) => last_err = Some(e),
            }
        }
        tracing::warn!(
            "STR file missing (tried {} name(s)): {} ({})",
            1 + aliases.len(),
            name,
            last_err.unwrap_or_default()
        );
        None
    }

    pub fn install_prepared(&mut self, name: &str, prepared: Option<StrEffectEntry>) -> bool {
        if self.entries.contains_key(name) {
            return true;
        }
        match prepared {
            Some(entry) => {
                self.entries.entry(name.to_string()).or_insert(entry);
                true
            }
            None => {
                self.missing.insert(name.to_string());
                false
            }
        }
    }

    pub fn is_resolved(&self, name: &str) -> bool {
        self.entries.contains_key(name) || self.missing.contains(name)
    }

    pub fn get(&self, name: &str) -> Option<&StrEffectEntry> {
        self.entries.get(name)
    }

    /// How long a loaded STR plays once through: its keys at its rate (the draw stops at the last key)
    pub fn seconds(&self, name: &str) -> Option<f32> {
        let file = &self.get(name)?.str_file;
        (file.fps > 0 && file.max_key > 0).then(|| file.max_key as f32 / file.fps as f32)
    }
}

fn try_load(
    name: &str,
    grf: &GrfArchive,
    layout: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    filtering: bool,
) -> Result<StrEffectEntry, String> {
    let str_path = nostalre_resources::texture::effect::str_file(name);
    let str_bytes = grf
        .read_file(&str_path)
        .map_err(|e| format!("{str_path}: {e}"))?;
    let str_file = StrEffectFile::parse(&str_bytes).map_err(|e| format!("parse: {e}"))?;

    let mut texture_paths = Vec::with_capacity(str_file.layers.len());
    let mut bind_groups = Vec::with_capacity(str_file.layers.len());
    for layer in &str_file.layers {
        let mut paths = Vec::with_capacity(layer.textures.len());
        let mut bgs: Vec<Option<wgpu::BindGroup>> = Vec::with_capacity(layer.textures.len());
        for tex_name in &layer.textures {
            let tex_path = nostalre_effects::str_texture_path(name, tex_name);
            let bg = load_str_texture(&tex_path, grf, device, queue, layout, filtering);
            bgs.push(bg);
            paths.push(tex_path);
        }
        texture_paths.push(paths);
        bind_groups.push(bgs);
    }

    Ok(StrEffectEntry {
        str_file,
        texture_paths,
        bind_groups,
    })
}

fn load_str_texture(
    path: &str,
    grf: &GrfArchive,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    filtering: bool,
) -> Option<wgpu::BindGroup> {
    let data = match grf.read_file(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("STR texture missing: {path} ({e})");
            return None;
        }
    };

    let img = match image::load_from_memory(&data) {
        Ok(i) => i,
        Err(_) => {
            let fmt = if path.ends_with(".tga") {
                image::ImageFormat::Tga
            } else if path.ends_with(".bmp") {
                image::ImageFormat::Bmp
            } else if path.ends_with(".png") {
                image::ImageFormat::Png
            } else {
                tracing::warn!("STR texture decode failed: {path}");
                return None;
            };
            match image::load_from_memory_with_format(&data, fmt) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!("STR texture decode failed: {path} ({e})");
                    return None;
                }
            }
        }
    };

    let mut rgba = img.to_rgba8();
    // STR textures use two transparency conventions:
    //   - magenta (FF00FF) for BMP color-keyed pixels (RO convention)
    //   - pure black for additive-blend layers (also key off black so the
    //     background doesn't add brightness)
    crate::texture::key_effect_texture(&mut rgba, !img.color().has_alpha());

    let filter = if filtering {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    };
    Some(crate::texture::create_texture_bind_group_from_rgba(
        device,
        queue,
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        layout,
        path,
        filter,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::AddressMode::ClampToEdge,
    ))
}

struct LayerAnim {
    visible: bool,
    texture_index: usize,
    positions: [f32; 8],
    color: [f32; 4],
    angle: f32,
    offset: [f32; 2],
    blend_src: i32,
    blend_dst: i32,
}

struct LayerBlend {
    additive: bool,
    use_vertex_alpha: bool,
}

const STR_ALPHA_REF: f32 = 15.0 / 255.0;

/// STR keyframes carry a pair of DirectX `D3DBLEND` constants: `1` ZERO,
/// `2` ONE, `3` SRCCOLOR, `4` INVSRCCOLOR, `5` SRCALPHA, `6` INVSRCALPHA,
/// `7` DESTALPHA, `12` BOTHSRCALPHA; which we remap onto our two sprite
/// pipelines. see https://github.com/ADHSoft/ro-str-viewer/blob/master/src/com/skardach/ro/graphics/BlendType.java
fn layer_blend(blend_src: i32, blend_dst: i32) -> LayerBlend {
    match (blend_src, blend_dst) {
        (2, 1) => LayerBlend {
            additive: false,
            use_vertex_alpha: false,
        },
        (2, 2) => LayerBlend {
            additive: true,
            use_vertex_alpha: false,
        },
        (5, 6) | (12, _) => LayerBlend {
            additive: false,
            use_vertex_alpha: true,
        },
        _ => LayerBlend {
            additive: true,
            use_vertex_alpha: true,
        },
    }
}

/// Vertex colour for a quad, or `None` when the alpha test rejects the whole
/// layer.
fn layer_quad_color(anim: &LayerAnim, blend: &LayerBlend) -> Option<[f32; 4]> {
    let alpha = (anim.color[3] / 255.0).clamp(0.0, 1.0);
    if alpha < STR_ALPHA_REF {
        return None;
    }
    Some([
        (anim.color[0] / 255.0).clamp(0.0, 1.0),
        (anim.color[1] / 255.0).clamp(0.0, 1.0),
        (anim.color[2] / 255.0).clamp(0.0, 1.0),
        if blend.use_vertex_alpha { alpha } else { 1.0 },
    ])
}

fn str_angle_to_radians(raw: f32) -> f32 {
    raw * std::f32::consts::TAU / 1024.0
}

fn calculate_layer_anim(layer: &EffectLayer, key_index: f32) -> LayerAnim {
    let invisible = LayerAnim {
        visible: false,
        texture_index: 0,
        positions: [0.0; 8],
        color: [1.0; 4],
        angle: 0.0,
        offset: [0.0; 2],
        blend_src: 5,
        blend_dst: 2,
    };

    let frames = &layer.frames;
    if frames.is_empty() {
        return invisible;
    }

    let mut from_id: Option<usize> = None;
    let mut to_id: Option<usize> = None;
    let mut last_frame = 0.0f32;
    let mut last_source = 0.0f32;

    for (i, f) in frames.iter().enumerate() {
        if f.frame_index as f32 <= key_index {
            if f.frame_type == 0 {
                from_id = Some(i);
            }
            if f.frame_type == 1 {
                to_id = Some(i);
            }
        }
        last_frame = last_frame.max(f.frame_index as f32);
        if f.frame_type == 0 {
            last_source = last_source.max(f.frame_index as f32);
        }
    }

    let Some(from_idx) = from_id else {
        return invisible;
    };

    if to_id.is_none() && last_frame < key_index {
        return invisible;
    }

    let from = &frames[from_idx];

    let has_morph =
        to_id.is_some_and(|ti| ti == from_idx + 1 && frames[ti].frame_index == from.frame_index);

    if !has_morph {
        if to_id.is_some() && last_source <= from.frame_index as f32 {
            return invisible;
        }

        return LayerAnim {
            visible: true,
            texture_index: from.texture_index as usize,
            positions: from.positions,
            color: from.color,
            angle: from.angle,
            offset: from.offset,
            blend_src: from.blend_src,
            blend_dst: from.blend_dst,
        };
    }

    let to = &frames[to_id.unwrap()];
    let delta = key_index - from.frame_index as f32;

    let mut color = [0f32; 4];
    for i in 0..4 {
        color[i] = from.color[i] + to.color[i] * delta;
    }

    let mut positions = [0f32; 8];
    for i in 0..8 {
        positions[i] = from.positions[i] + to.positions[i] * delta;
    }

    let angle = from.angle + to.angle * delta;
    let offset = [
        from.offset[0] + to.offset[0] * delta,
        from.offset[1] + to.offset[1] * delta,
    ];

    let tex_count = layer.textures.len().max(1);
    let anim_frame = match to.animation_mode {
        1 => (from.texture_index + to.texture_index * delta) as usize,
        2 => ((from.texture_index + to.delay * delta) as usize).min(tex_count - 1),
        3 => (from.texture_index + to.delay * delta) as usize % tex_count,
        4 => {
            let raw = (from.texture_index - to.delay * delta) as i32;
            ((raw % tex_count as i32 + tex_count as i32) % tex_count as i32) as usize
        }
        _ => 0,
    };

    LayerAnim {
        visible: true,
        texture_index: anim_frame,
        positions,
        color,
        angle,
        offset,
        blend_src: from.blend_src,
        blend_dst: from.blend_dst,
    }
}

pub struct StrEmitterInput<'a> {
    pub str_name: &'a str,
    pub position: [f32; 3],
    /// From `position` in the camera's own axes: x right, y down, z away (kRO's view-space slot offsets)
    pub view_offset: [f32; 3],
    /// The size of every quad about the STR's centre (kRO's +0x3fa8)
    pub scale: f32,
    pub anim_time: f32,
    pub repeat: bool,
    pub turn: StrTurn,
    /// Every layer's alpha times this (kRO's +0x3fac)
    pub alpha: f32,
}

impl<'a> StrEmitterInput<'a> {
    /// A whole STR at a point, at its own size
    pub fn at(str_name: &'a str, position: [f32; 3], anim_time: f32, repeat: bool) -> Self {
        Self {
            str_name,
            position,
            view_offset: [0.0; 3],
            scale: 1.0,
            anim_time,
            repeat,
            turn: StrTurn::None,
            alpha: 1.0,
        }
    }
}

/// An STR's turn on screen in degrees, 0 to the right turning clockwise (kRO's +0x3fa4); toward a point, the way
/// the camera shows from the STR to it (`Camera::screen_facing` reads 0 down, 90 right: kRO's angle is 90 less it)
fn turn_degrees(
    turn: StrTurn,
    camera: &Camera,
    view_projection: glam::Mat4,
    from: [f32; 3],
) -> f32 {
    match turn {
        StrTurn::None => 0.0,
        StrTurn::Fixed(degrees) => degrees,
        StrTurn::Toward { point, plus } => camera
            .screen_facing_in(view_projection, from, point)
            .map_or(0.0, |facing| (90.0 - facing + plus).rem_euclid(360.0)),
    }
}

/// The camera's right, down and away axes in the world, for offsets given in view space
fn view_axes(camera: &Camera) -> [glam::Vec3; 3] {
    let view = camera.view_matrix();
    let axis = |row: usize| view.row(row).truncate().normalize_or_zero();
    let y = axis(1);
    // the world's up is -Y: the screen's down leans to +Y
    let down = if y.y >= 0.0 { y } else { -y };
    [axis(0), down, -axis(2)]
}

pub fn build_str_effect_batches<'a>(
    emitters: &[StrEmitterInput<'_>],
    cache: &'a StrEffectCache,
    camera: &Camera,
    screen_w: f32,
    screen_h: f32,
    zoom: f32,
) -> Vec<SpriteBatch<'a>> {
    let mut batches = Vec::new();
    let axes = emitters
        .iter()
        .any(|e| e.view_offset != [0.0; 3])
        .then(|| view_axes(camera));
    let view_projection = camera.view_projection();

    for input in emitters {
        // faded out: nothing to draw
        if input.alpha <= 0.0 {
            continue;
        }
        let Some(entry) = cache.get(input.str_name) else {
            continue;
        };
        let str_file = &entry.str_file;

        let position = match axes {
            Some([right, down, away]) if input.view_offset != [0.0; 3] => {
                let [x, y, z] = input.view_offset;
                (glam::Vec3::from(input.position) + right * x + down * y + away * z).to_array()
            }
            _ => input.position,
        };
        let Some((anchor, depth, ppu)) = project_billboard(camera, position, screen_w, screen_h)
        else {
            continue;
        };
        // kRO turns only past 0: (cos·dx − sin·dy, sin·dx + cos·dy) about the centre, the screen's y down
        let (turn_sin, turn_cos) = match turn_degrees(input.turn, camera, view_projection, position)
        {
            degrees if degrees > 0.0 => degrees.to_radians().sin_cos(),
            _ => (0.0, 1.0),
        };

        let mut key_index = input.anim_time * str_file.fps as f32;
        if str_file.max_key > 0 && key_index >= str_file.max_key as f32 {
            if input.repeat {
                key_index %= str_file.max_key as f32;
            } else {
                continue;
            }
        }

        for (layer_idx, layer) in str_file.layers.iter().enumerate() {
            let anim = calculate_layer_anim(layer, key_index);
            if !anim.visible {
                continue;
            }

            let layer_bgs = &entry.bind_groups[layer_idx];
            if anim.texture_index >= layer_bgs.len() {
                continue;
            }

            let Some(texture) = layer_bgs[anim.texture_index].as_ref() else {
                continue;
            };

            let blend = layer_blend(anim.blend_src, anim.blend_dst);
            let Some(mut color) = layer_quad_color(&anim, &blend) else {
                continue;
            };
            if input.alpha < 1.0 {
                // a layer whose blend reads no vertex alpha fades by its colour
                let faded = if blend.use_vertex_alpha {
                    &mut color[3..]
                } else {
                    &mut color[..]
                };
                faded.iter_mut().for_each(|c| *c *= input.alpha);
            }

            let angle_rad = str_angle_to_radians(anim.angle);
            let cos_a = angle_rad.cos();
            let sin_a = angle_rad.sin();

            let scale = ppu * zoom / 75.0 * input.scale;
            let offset_x = (anim.offset[0] - 320.0) * scale;
            let offset_y = (anim.offset[1] - 320.0) * scale;

            let xy = &anim.positions;
            let corners = [
                (xy[0], xy[4]), // TL
                (xy[1], xy[5]), // TR
                (xy[3], xy[7]), // BL
                (xy[2], xy[6]), // BR
            ];
            let uvs = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

            let vertices: [SpriteVertex; 4] = std::array::from_fn(|i| {
                let (px, py) = corners[i];
                let sx = px * scale;
                let sy = py * scale;
                let rx = sx * cos_a - sy * sin_a + offset_x;
                let ry = sx * sin_a + sy * cos_a + offset_y;
                SpriteVertex {
                    position: [
                        anchor[0] + rx * turn_cos - ry * turn_sin,
                        anchor[1] + rx * turn_sin + ry * turn_cos,
                        depth - 0.001,
                    ],
                    tex_coord: uvs[i],
                    color,
                }
            });

            batches.push(SpriteBatch {
                vertices: vertices.into(),
                indices: Z_QUAD_INDICES,
                texture,
                additive: blend.additive,
                no_depth: false,
                weapon_aura: false,
            });
        }
    }
    batches
}

#[cfg(test)]
mod tests {
    use nostalre_formats::str_effect::{EffectFrame, EffectLayer};

    use super::{
        Camera, StrTurn, calculate_layer_anim, layer_blend, layer_quad_color, str_angle_to_radians,
        turn_degrees,
    };

    #[test]
    fn an_str_turns_toward_its_point_as_kro_aims_0_right_90_down() {
        let camera = Camera::default();
        let from = camera.target.to_array();
        let right = camera.right_vector();
        let toward = |dx: f32, dz: f32, plus: f32| {
            let point = [from[0] + dx, from[1], from[2] + dz];
            turn_degrees(
                StrTurn::Toward { point, plus },
                &camera,
                camera.view_projection(),
                from,
            )
        };
        let to_right = toward(right.x * 10.0, right.z * 10.0, 0.0);
        assert!(
            to_right < 1.0 || to_right > 359.0,
            "to the right is 0: {to_right}"
        );
        let eye = camera.eye();
        let down = toward(eye.x - from[0], eye.z - from[2], 0.0);
        assert!(
            (down - 90.0).abs() < 1.0,
            "toward the camera, down the screen, is 90: {down}"
        );
        let back = toward(right.x * 10.0, right.z * 10.0, 180.0);
        assert!((back - 180.0).abs() < 1.0, "plus turns it on: {back}");
        assert_eq!(
            turn_degrees(
                StrTurn::Fixed(40.0),
                &camera,
                camera.view_projection(),
                from
            ),
            40.0
        );
        assert_eq!(
            turn_degrees(StrTurn::None, &camera, camera.view_projection(), from),
            0.0
        );
    }

    fn source_frame(frame_index: i32, alpha: f32, blend: (i32, i32)) -> EffectFrame {
        EffectFrame {
            frame_index,
            frame_type: 0,
            offset: [320.0, 320.0],
            tex_coords: [0.0; 8],
            positions: [0.0; 8],
            texture_index: 0.0,
            animation_mode: 0,
            delay: 0.0,
            angle: 0.0,
            color: [255.0, 255.0, 255.0, alpha],
            blend_src: blend.0,
            blend_dst: blend.1,
            multi_texture: 0,
        }
    }

    #[test]
    fn opaque_layer_pins_alpha_and_drops_below_alpha_ref() {
        let layer = EffectLayer {
            textures: vec!["hunter_talkiebox.bmp".to_string()],
            frames: vec![
                source_frame(0, 255.0, (2, 1)),
                source_frame(10, 10.0, (2, 1)),
            ],
        };

        let anim = calculate_layer_anim(&layer, 0.0);
        let blend = layer_blend(anim.blend_src, anim.blend_dst);
        assert!(!blend.additive);
        assert_eq!(layer_quad_color(&anim, &blend).unwrap()[3], 1.0);

        let faded = calculate_layer_anim(&layer, 10.0);
        let blend = layer_blend(faded.blend_src, faded.blend_dst);
        assert!(layer_quad_color(&faded, &blend).is_none());
    }

    #[test]
    fn alpha_weighted_layers_keep_their_vertex_alpha() {
        let anim = calculate_layer_anim(
            &EffectLayer {
                textures: vec!["ring_b.bmp".to_string()],
                frames: vec![source_frame(0, 128.0, (5, 7))],
            },
            0.0,
        );
        let blend = layer_blend(anim.blend_src, anim.blend_dst);
        assert!(blend.additive);
        assert!((layer_quad_color(&anim, &blend).unwrap()[3] - 128.0 / 255.0).abs() < 1e-6);

        let anim = calculate_layer_anim(
            &EffectLayer {
                textures: vec!["alpha.bmp".to_string()],
                frames: vec![source_frame(0, 128.0, (5, 6))],
            },
            0.0,
        );
        let blend = layer_blend(anim.blend_src, anim.blend_dst);
        assert!(!blend.additive);
        assert!((layer_quad_color(&anim, &blend).unwrap()[3] - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn str_angle_decodes_brand_units_not_degrees() {
        assert!((str_angle_to_radians(1024.0) - std::f32::consts::TAU).abs() < 1e-4);
        assert!((str_angle_to_radians(256.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        let deg = |r: f32| r.to_degrees();
        assert!((deg(str_angle_to_radians(85.333)) - 30.0).abs() < 0.5);
        assert!((deg(str_angle_to_radians(-156.444)) + 55.0).abs() < 0.5);
        assert!((deg(str_angle_to_radians(-312.889)) + 110.0).abs() < 0.5);
    }
}
