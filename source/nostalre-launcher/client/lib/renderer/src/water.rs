use nostalre_formats::gnd::GndFile;
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::rsw::WaterSettings;

use crate::device::DEPTH_FORMAT;
use crate::global_uniforms::{GlobalUniforms, WaterGlobalsUniform};
use crate::texture::TextureCache;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    /// Deepest corner of the cell's floor. World Y grows downwards, so this is
    /// the largest of the four heights.
    pub floor_y: f32,
    /// `x + z` at the cell's mid-diagonal, shared by all four of its vertices:
    /// the point the wave level is sampled at to decide whether the cell is wet.
    pub phase_pos: f32,
}

impl WaterVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<WaterVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x2,
            2 => Float32,
            3 => Float32,
        ],
    };
}

/// Shared by both water shaders: the classic one reads the first eight floats.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WaterUniforms {
    wave_height: f32,
    wave_pitch_per_unit: f32,
    wave_offset: f32,
    opacity: f32,
    ambient_tint: f32,
    time: f32,
    wave_speed: f32,
    emissive: f32,
    tint: [f32; 4],
    deep: [f32; 4],
    absorb: [f32; 4],
    /// width, height, near, far
    screen: [f32; 4],
    level: f32,
    cell_size: f32,
    debug: f32,
    _padding: f32,
}

const WATER_FRAMES: usize = 32;
const WATER_TEXTURE_CELLS: f32 = 4.0;
const WATER_OPACITY: f32 = 144.0 / 255.0;
const AMBIENT_TINTED_WATER_TYPE: i32 = 4;
const WATER_TEXTURE_SETS: i32 = 6;

const WATER_TICKS_PER_FRAME: f32 = 2.0;
const WATER_FRAME_RATE: f32 = 60.0;
const WATER_TICKS_PER_SECOND: f32 = WATER_TICKS_PER_FRAME * WATER_FRAME_RATE;

/// Caustics on the bed under HD water.
const CAUSTIC_STRENGTH: f32 = 0.35;

/// How HD water colours one water type: linear colours, absorption per cell of depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterMaterial {
    /// Shallow colour; `.3` = how strongly it fills in what the water absorbed (murkiness).
    pub tint: [f32; 4],
    pub deep: [f32; 4],
    pub absorb: [f32; 4],
    /// Lava: glows, no refraction.
    pub emissive: bool,
}

/// Per water type, from the colour of its texture set (`data/texture/워터/water<type>NN.jpg`): 0 clear
/// blue, 1 deep ocean, 2 bright shallows, 3 dark dungeon water, 4 lava, 5 swamp, 6 lava crust, 7 magenta.
/// Types past those take the texture set they fall back to.
pub fn water_material(water_type: i32) -> WaterMaterial {
    // gamma colours: shallow tint, deep, absorption (r, g, b per cell), murkiness
    let (tint, deep, absorb, murk, emissive) = match water_type.rem_euclid(8) {
        0 => ([0.10, 0.44, 0.59], [0.02, 0.10, 0.20], [1.6, 0.55, 0.35], 0.55, false),
        1 => ([0.04, 0.32, 0.40], [0.01, 0.07, 0.12], [1.8, 0.70, 0.55], 0.75, false),
        2 => ([0.20, 0.59, 0.62], [0.03, 0.18, 0.24], [1.3, 0.40, 0.35], 0.45, false),
        3 => ([0.18, 0.25, 0.37], [0.02, 0.04, 0.08], [1.4, 1.00, 0.70], 0.70, false),
        5 => ([0.31, 0.28, 0.15], [0.08, 0.07, 0.03], [0.9, 1.00, 1.60], 0.90, false),
        7 => ([0.76, 0.06, 0.70], [0.20, 0.01, 0.20], [0.6, 2.00, 0.70], 0.65, false),
        4 | 6 => ([0.81, 0.16, 0.12], [0.30, 0.04, 0.02], [0.0; 3], 1.0, true),
        _ => unreachable!(),
    };
    let lin = |c: [f32; 3], a: f32| [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2]), a];
    WaterMaterial {
        tint: lin(tint, murk),
        deep: lin(deep, 1.0),
        absorb: [absorb[0], absorb[1], absorb[2], 0.0],
        emissive,
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The frame HD water refracts: a copy of the opaque scene, remade when the frame size or format changes.
struct SceneCopy {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct WaterRenderer {
    pipeline: wgpu::RenderPipeline,
    hd_pipeline: wgpu::RenderPipeline,
    scene_layout: wgpu::BindGroupLayout,
    scene_sampler: wgpu::Sampler,
    scene_copy: Option<SceneCopy>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_names: Vec<String>,
    wave_height: f32,
    wave_pitch_per_unit: f32,
    wave_speed: f32,
    anim_speed: f32,
    opacity: f32,
    ambient_tint: f32,
    level: f32,
    cell_size: f32,
    material: WaterMaterial,
    /// `NOSTALRE_WATER_DEBUG`: one term of the HD shader on screen (see water_hd.wgsl).
    debug: f32,
}

fn water_texture_name(water_type: i32, frame: usize) -> String {
    nostalre_resources::texture::water(water_type, frame)
}

/// Every texture a map with this water type may sample: its own animation, plus
/// the set it falls back to when the archive ships no textures for that type.
pub fn water_texture_candidates(water_type: i32) -> Vec<String> {
    if water_type < 0 {
        return Vec::new();
    }
    let fallback_type = water_type % WATER_TEXTURE_SETS;
    (0..WATER_FRAMES)
        .flat_map(|i| {
            let own = water_texture_name(water_type, i);
            if fallback_type == water_type {
                vec![own]
            } else {
                vec![own, water_texture_name(fallback_type, i)]
            }
        })
        .collect()
}

fn wave_offset_at(elapsed: f32, wave_speed: f32) -> f32 {
    let phase = (elapsed * wave_speed * WATER_TICKS_PER_SECOND).rem_euclid(360.0);
    if phase > 180.0 { phase - 360.0 } else { phase }
}

fn texture_frame_at(elapsed: f32, anim_speed: f32) -> usize {
    if anim_speed <= 0.0 {
        return 0;
    }
    let cycle = WATER_FRAMES as f32 * anim_speed;
    let ticks = (elapsed * WATER_TICKS_PER_SECOND).rem_euclid(cycle);
    ((ticks / anim_speed) as usize).min(WATER_FRAMES - 1)
}

/// What HD water needs from the frame it draws into.
pub struct HdWaterTarget<'a> {
    /// The finished opaque scene: copied, then drawn over. It must allow COPY_SRC.
    pub scene_texture: &'a wgpu::Texture,
    pub scene_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
}

impl WaterRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn from_water_settings(
        water: &WaterSettings,
        gnd: &GndFile,
        grf: &GrfArchive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniforms: &GlobalUniforms,
        texture_cache: &mut TextureCache,
        surface_format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let water_y = water.level?;
        let water_type = water.water_type.unwrap_or(0);
        if water_type < 0 {
            return None;
        }
        let zoom = gnd.zoom;

        let wave_height = water.wave_height.unwrap_or(1.0);
        let wave_speed = water.wave_speed.unwrap_or(2.0) % 360.0;
        let wave_pitch_per_unit = water.wave_pitch.unwrap_or(50.0) / zoom;
        let anim_speed = water.anim_speed.unwrap_or(3) as f32;
        let is_ambient_tinted = water_type == AMBIENT_TINTED_WATER_TYPE;
        let opacity = if is_ambient_tinted {
            1.0
        } else {
            WATER_OPACITY
        };
        let ambient_tint = if is_ambient_tinted { 1.0 } else { 0.0 };

        let fallback_type = water_type % WATER_TEXTURE_SETS;
        let texture_names: Vec<String> = (0..WATER_FRAMES)
            .map(|i| {
                let name = water_texture_name(water_type, i);
                if texture_cache
                    .get_or_load(&name, grf, device, queue, false)
                    .is_some()
                    || fallback_type == water_type
                {
                    return name;
                }
                let fallback = water_texture_name(fallback_type, i);
                texture_cache.get_or_load(&fallback, grf, device, queue, false);
                fallback
            })
            .collect();

        let (vertices, indices) = build_water_mesh(gnd, water_y, wave_height);
        if vertices.is_empty() {
            return None;
        }

        let vertex_buffer = create_buffer(
            device,
            "water_vertices",
            &vertices,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer =
            create_buffer(device, "water_indices", &indices, wgpu::BufferUsages::INDEX);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("water_uniforms"),
            size: std::mem::size_of::<WaterUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("water_uniforms"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water_uniforms"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let scene_layout = scene_bind_group_layout(device);
        let scene_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_scene"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let layouts = [
            &global_uniforms.bind_group_layout,
            &texture_cache.bind_group_layout,
            &uniform_bind_group_layout,
        ];
        let pipeline = create_pipeline(
            device,
            surface_format,
            "water",
            include_str!("shaders/water.wgsl"),
            &layouts,
        );
        let hd_pipeline = create_pipeline(
            device,
            surface_format,
            "water_hd",
            include_str!("shaders/water_hd.wgsl"),
            &[layouts[0], layouts[1], layouts[2], &scene_layout],
        );

        Some(Self {
            pipeline,
            hd_pipeline,
            scene_layout,
            scene_sampler,
            scene_copy: None,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            uniform_buffer,
            uniform_bind_group,
            texture_names,
            wave_height,
            wave_pitch_per_unit,
            wave_speed,
            anim_speed,
            opacity,
            ambient_tint,
            level: water_y,
            cell_size: zoom,
            material: water_material(water_type),
            debug: std::env::var("NOSTALRE_WATER_DEBUG")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
        })
    }

    /// `screen` = (width, height, near, far) of the frame it draws into.
    pub fn update(&self, queue: &wgpu::Queue, elapsed: f32, screen: [f32; 4]) {
        let m = &self.material;
        let uniforms = WaterUniforms {
            wave_height: self.wave_height,
            wave_pitch_per_unit: self.wave_pitch_per_unit,
            wave_offset: wave_offset_at(elapsed, self.wave_speed),
            opacity: self.opacity,
            ambient_tint: self.ambient_tint,
            time: elapsed,
            wave_speed: self.wave_speed,
            emissive: if m.emissive { 1.0 } else { 0.0 },
            tint: m.tint,
            deep: m.deep,
            absorb: m.absorb,
            screen,
            level: self.level,
            cell_size: self.cell_size,
            debug: self.debug,
            _padding: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// The caustics the terrain draws under this water when it is drawn HD.
    pub fn caustics(&self, elapsed: f32) -> WaterGlobalsUniform {
        WaterGlobalsUniform {
            level: self.level,
            time: elapsed,
            strength: if self.material.emissive {
                0.0
            } else {
                CAUSTIC_STRENGTH
            },
            cell_size: self.cell_size,
        }
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        global_uniforms: &'a GlobalUniforms,
        texture_cache: &'a TextureCache,
        elapsed: f32,
    ) {
        let tex_name = &self.texture_names[texture_frame_at(elapsed, self.anim_speed)];
        let tex_bg = match texture_cache.get(tex_name) {
            Some(bg) => bg,
            None => return,
        };

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &global_uniforms.bind_group, &[]);
        pass.set_bind_group(1, tex_bg, &[]);
        pass.set_bind_group(2, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// HD water: copies the finished opaque scene, then draws over it in its own pass with the depth
    /// buffer read-only, so the shader can sample both.
    pub fn render_hd(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: HdWaterTarget<'_>,
        global_uniforms: &GlobalUniforms,
        texture_cache: &TextureCache,
        elapsed: f32,
    ) {
        let tex_name = &self.texture_names[texture_frame_at(elapsed, self.anim_speed)];
        let Some(tex_bg) = texture_cache.get(tex_name) else {
            return;
        };
        let source = target.scene_texture;
        let size = source.size();
        let stale = self.scene_copy.as_ref().is_none_or(|copy| {
            copy.texture.size() != size || copy.texture.format() != source.format()
        });
        if stale {
            self.scene_copy = Some(create_scene_copy(device, source));
        }
        let copy = self.scene_copy.as_ref().expect("scene copy was just made");
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            copy.texture.as_image_copy(),
            size,
        );
        // made every frame: the depth view is replaced on resize and when the surface comes back
        let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water_scene"),
            layout: &self.scene_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&copy.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(target.depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.scene_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water_hd"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.scene_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            // read-only: the shader samples it in the same pass
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.hd_pipeline);
        pass.set_bind_group(0, &global_uniforms.bind_group, &[]);
        pass.set_bind_group(1, tex_bg, &[]);
        pass.set_bind_group(2, &self.uniform_bind_group, &[]);
        pass.set_bind_group(3, &scene_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn scene_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("water_scene"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// A texture the scene can be copied into: the same format (a copy must match), read through its
/// non-sRGB view so the shader gets the gamma-encoded values the scene was drawn with.
fn create_scene_copy(device: &wgpu::Device, source: &wgpu::Texture) -> SceneCopy {
    let format = source.format();
    let read_format = format.remove_srgb_suffix();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water_scene_copy"),
        size: source.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: if read_format == format {
            &[]
        } else {
            std::slice::from_ref(&read_format)
        },
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(read_format),
        ..Default::default()
    });
    SceneCopy { texture, view }
}

pub fn build_water_mesh(
    gnd: &GndFile,
    water_y: f32,
    wave_height: f32,
) -> (Vec<WaterVertex>, Vec<u32>) {
    let zoom = gnd.zoom;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // In native RO coords more negative = higher, so a corner "below" the water
    // surface has a larger height value. A cell gets a water quad when any of
    // its corners dips below the wave troughs (water_y - wave_height).
    let submerged = water_y - wave_height;

    for y in 0..gnd.height {
        for x in 0..gnd.width {
            let cell_idx = (y * gnd.width + x) as usize;
            let cell = &gnd.cells[cell_idx];

            if cell.surface_up < 0 {
                continue;
            }

            if cell.height_sw < submerged
                && cell.height_se < submerged
                && cell.height_nw < submerged
                && cell.height_ne < submerged
            {
                continue;
            }

            let wx = x as f32 * zoom;
            let wz = y as f32 * zoom;

            let u0 = x as f32 / WATER_TEXTURE_CELLS;
            let u1 = (x + 1) as f32 / WATER_TEXTURE_CELLS;
            let v0 = y as f32 / WATER_TEXTURE_CELLS;
            let v1 = (y + 1) as f32 / WATER_TEXTURE_CELLS;

            let floor_y = cell
                .height_sw
                .max(cell.height_se)
                .max(cell.height_nw)
                .max(cell.height_ne);
            let phase_pos = wx + wz + zoom;

            let base = vertices.len() as u32;
            vertices.push(WaterVertex {
                position: [wx, water_y, wz],
                tex_coord: [u0, v0],
                floor_y,
                phase_pos,
            });
            vertices.push(WaterVertex {
                position: [wx + zoom, water_y, wz],
                tex_coord: [u1, v0],
                floor_y,
                phase_pos,
            });
            vertices.push(WaterVertex {
                position: [wx, water_y, wz + zoom],
                tex_coord: [u0, v1],
                floor_y,
                phase_pos,
            });
            vertices.push(WaterVertex {
                position: [wx + zoom, water_y, wz + zoom],
                tex_coord: [u1, v1],
                floor_y,
                phase_pos,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
    }

    (vertices, indices)
}

fn create_buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    label: &str,
    data: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    label: &str,
    source: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &bind_group_layouts.iter().copied().map(Some).collect::<Vec<_>>(),
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[WaterVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            // never: HD water's pass holds the depth buffer read-only
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_formats::gnd::{GndCell, GndFile};

    fn make_gnd(width: i32, height: i32, cell_height: f32) -> GndFile {
        let cell_count = (width * height) as usize;
        GndFile {
            version: (1, 7),
            width,
            height,
            zoom: 10.0,
            textures: vec![],
            lightmaps: vec![],
            surfaces: vec![nostalre_formats::gnd::GndSurface {
                tex_u: [0.0; 4],
                tex_v: [0.0; 4],
                texture_id: 0,
                lightmap_id: 0,
                color_bgra: [255; 4],
            }],
            cells: (0..cell_count)
                .map(|_| GndCell {
                    height_sw: cell_height,
                    height_se: cell_height,
                    height_nw: cell_height,
                    height_ne: cell_height,
                    surface_up: 0,
                    surface_south: -1,
                    surface_east: -1,
                })
                .collect(),
        }
    }

    #[test]
    fn water_mesh_generates_quads_for_cells_below_water() {
        let gnd = make_gnd(4, 4, -5.0);
        let water_y = -10.0;
        let (vertices, indices) = build_water_mesh(&gnd, water_y, 1.0);
        assert_eq!(vertices.len(), 16 * 4);
        assert_eq!(indices.len(), 16 * 6);
    }

    #[test]
    fn water_mesh_skips_border_cells_without_surface() {
        let mut gnd = make_gnd(3, 3, -5.0);
        for y in 0..3i32 {
            for x in 0..3i32 {
                if x == 0 || x == 2 || y == 0 || y == 2 {
                    gnd.cells[(y * 3 + x) as usize].surface_up = -1;
                }
            }
        }
        let (vertices, indices) = build_water_mesh(&gnd, -10.0, 1.0);
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn water_mesh_skips_cells_entirely_above_water() {
        let gnd = make_gnd(4, 4, -100.0);
        let (vertices, indices) = build_water_mesh(&gnd, -10.0, 1.0);
        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn water_mesh_uv_is_continuous_and_tiles_every_4_cells() {
        let gnd = make_gnd(6, 1, -5.0);
        let (vertices, _) = build_water_mesh(&gnd, -10.0, 1.0);
        assert!((vertices[0].tex_coord[0] - 0.0).abs() < 0.01);
        let cell4_base = 4 * 4;
        assert!((vertices[cell4_base].tex_coord[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn water_mesh_carries_deepest_floor_and_shared_phase_anchor() {
        let mut gnd = make_gnd(2, 1, -5.0);
        gnd.cells[1].height_se = 3.0;
        let (vertices, _) = build_water_mesh(&gnd, -10.0, 1.0);

        for v in &vertices[0..4] {
            assert_eq!(v.phase_pos, 10.0);
            assert_eq!(v.floor_y, -5.0);
        }
        for v in &vertices[4..8] {
            assert_eq!(v.phase_pos, 20.0);
            assert_eq!(v.floor_y, 3.0);
        }
    }

    #[test]
    fn water_uniforms_match_the_wgsl_layout() {
        // 8 floats, 4 vec4s, 4 floats (water_hd.wgsl's WaterUniforms)
        assert_eq!(std::mem::size_of::<WaterUniforms>(), 112);
    }

    #[test]
    fn lava_glows_and_water_takes_its_set_colour() {
        assert!(water_material(4).emissive && water_material(6).emissive);
        assert!(!water_material(0).emissive);
        // clear blue water is bluer than it is red, and absorbs red fastest
        let clear = water_material(0);
        assert!(clear.tint[2] > clear.tint[0]);
        assert!(clear.absorb[0] > clear.absorb[2]);
        // the deep colour is darker than the shallow one
        assert!(clear.deep[1] < clear.tint[1]);
        // types past the known sets wrap like their textures
        assert_eq!(water_material(8), water_material(0));
    }

    #[test]
    fn water_animation_advances_two_ticks_per_frame() {
        let one_frame = 1.0 / WATER_FRAME_RATE;

        assert!((wave_offset_at(one_frame, 2.0) - 4.0).abs() < 0.001);
        assert!((wave_offset_at(1.0, 2.0) - -120.0).abs() < 0.001);

        assert_eq!(texture_frame_at(one_frame, 3.0), 0);
        assert_eq!(texture_frame_at(3.0 * one_frame, 3.0), 2);
        assert_eq!(texture_frame_at(47.0 * one_frame, 3.0), 31);
        assert_eq!(texture_frame_at(48.0 * one_frame, 3.0), 0);
    }
}
