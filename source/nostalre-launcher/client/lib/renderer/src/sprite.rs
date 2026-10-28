use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::{Arc, Weak};

use nostalre_formats::act::{
    ActFile, Motion, SpriteFrame, attachment_offset, head_chained_attachment_offset,
};
use nostalre_formats::imf::ImfLayerOrder;
use nostalre_formats::spr::{RgbaImageData, SpriteData};

use crate::device::DEPTH_FORMAT;
use crate::mesh_batch::{MeshBatcher, QUAD_INDICES, Vertices};
use crate::texture::create_texture_bind_group_from_rgba;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
}

impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Float32x4,
    ];

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &Self::ATTRIBS,
    };
}

static FILTERING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_filtering(on: bool) {
    FILTERING.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn sprite_filter() -> wgpu::FilterMode {
    if FILTERING.load(std::sync::atomic::Ordering::Relaxed) {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    }
}

pub struct SpriteTextures {
    pub bind_groups: Arc<Vec<wgpu::BindGroup>>,
    /// Empty for standalone images; otherwise each frame's rectangle in its atlas.
    pub uv_rects: Arc<Vec<[f32; 4]>>,
    pub sizes: Vec<(u32, u32)>,
    pub indexed_count: usize,
}

/// Owned by one renderer/archive stack. Weak entries never keep departed actors'
/// GPU textures alive. Bodies, heads, weapons, carts and shadows can be shared
/// independently even when two actors have different complete appearances.
#[derive(Default)]
pub struct SpriteTextureCache {
    sheets: RefCell<HashMap<(String, bool), (Weak<Vec<wgpu::BindGroup>>, Arc<Vec<[f32; 4]>>)>>,
}

impl SpriteTextureCache {
    pub fn upload(
        &self,
        data: &SpriteData,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> SpriteTextures {
        let key = data.texture_key.as_ref().map(|path| (
            path.clone(), FILTERING.load(std::sync::atomic::Ordering::Relaxed),
        ));
        if let Some((groups, uv_rects)) = key.as_ref().and_then(|key| self.sheets.borrow().get(key).and_then(|(groups, uv)| groups.upgrade().map(|groups| (groups, Arc::clone(uv))))) {
            return SpriteTextures {
                bind_groups: groups,
                uv_rects,
                sizes: data.images.iter().map(|image| (image.width, image.height)).collect(),
                indexed_count: data.indexed_count,
            };
        }
        // Weapon aura shaders extend coordinates beyond a whole weapon image.
        // Keep those standalone; only body/head loaders opt in to frame atlases.
        let atlas = data.texture_key.as_deref().is_some_and(|key| key.contains("|cloth=") || key.contains("|hair="));
        let textures = if atlas {
            upload_actor_atlases(&data.images, data.indexed_count, device, queue, layout)
        } else {
            upload_sprite_textures(&data.images, data.indexed_count, device, queue, layout)
        };
        if let Some(key) = key {
            let mut sheets = self.sheets.borrow_mut();
            sheets.retain(|_, (groups, _)| groups.strong_count() > 0);
            sheets.insert(key, (Arc::downgrade(&textures.bind_groups), Arc::clone(&textures.uv_rects)));
        }
        textures
    }
}

pub fn upload_sprite_textures(
    images: &[RgbaImageData],
    indexed_count: usize,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> SpriteTextures {
    upload_sprite_textures_filtered(
        images,
        indexed_count,
        device,
        queue,
        layout,
        sprite_filter(),
    )
}

/// Bitmap glyphs (damage digits, the miss/crit plates, the rank and time
/// fonts), point-sampled whatever the sprite filtering setting is: a digit is a
/// dozen texels wide and lands on a fractional pixel, so filtering it softens
/// the whole shape rather than a rim.
pub fn upload_glyph_textures(
    images: &[RgbaImageData],
    indexed_count: usize,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> SpriteTextures {
    upload_sprite_textures_filtered(
        images,
        indexed_count,
        device,
        queue,
        layout,
        wgpu::FilterMode::Nearest,
    )
}

pub fn upload_sprite_textures_filtered(
    images: &[RgbaImageData],
    indexed_count: usize,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    filter: wgpu::FilterMode,
) -> SpriteTextures {
    nostalre_profiling::profile_scope!(
        "upload_sprite_textures",
        format!("{} images", images.len())
    );
    let mut bind_groups = Vec::with_capacity(images.len());
    let mut sizes = Vec::with_capacity(images.len());

    for (i, img) in images.iter().enumerate() {
        let label = if i < indexed_count {
            format!("spr_idx_{i}")
        } else {
            format!("spr_rgba_{}", i - indexed_count)
        };
        let bg = create_texture_bind_group_from_rgba(
            device,
            queue,
            &img.data,
            img.width,
            img.height,
            layout,
            &label,
            filter,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::AddressMode::ClampToEdge,
        );
        sizes.push((img.width, img.height));
        bind_groups.push(bg);
    }

    SpriteTextures {
        bind_groups: Arc::new(bind_groups),
        uv_rects: Arc::new(Vec::new()),
        sizes,
        indexed_count,
    }
}

/// Shelf packing with a replicated one-pixel border keeps linear filtering
/// identical to clamped standalone frames. Build/upload one page at a time so
/// a large animation never duplicates all RGBA pixels in staging memory.
fn actor_atlas_plan(images: &[RgbaImageData], edge: u32) -> Vec<Vec<(usize, u32, u32)>> {
    let mut pages = Vec::new();
    let (mut page, mut x, mut y, mut row) = (Vec::new(), 0, 0, 0);
    for (i, image) in images.iter().enumerate() {
        let (w, h) = (image.width + 2, image.height + 2);
        if x + w > edge { x = 0; y += row; row = 0; }
        if y + h > edge {
            pages.push(std::mem::take(&mut page)); x = 0; y = 0; row = 0;
        }
        page.push((i, x, y)); x += w; row = row.max(h);
    }
    if !page.is_empty() { pages.push(page); }
    pages
}

fn actor_atlas_pixels(images: &[RgbaImageData], page: &[(usize, u32, u32)]) -> (Vec<u8>, u32, u32) {
    let width = page.iter().map(|&(i,x,_)| x + images[i].width + 2).max().unwrap();
    let height = page.iter().map(|&(i,_,y)| y + images[i].height + 2).max().unwrap();
    let mut pixels = vec![0; (width * height * 4) as usize];
    for &(i, x, y) in page {
        let image = &images[i];
        for py in 0..image.height + 2 {
            for px in 0..image.width + 2 {
                let sx = px.saturating_sub(1).min(image.width - 1);
                let sy = py.saturating_sub(1).min(image.height - 1);
                let src = ((sy * image.width + sx) * 4) as usize;
                let dst = (((y + py) * width + x + px) * 4) as usize;
                pixels[dst..dst+4].copy_from_slice(&image.data[src..src+4]);
            }
        }
    }
    (pixels, width, height)
}

fn upload_actor_atlases(
    images: &[RgbaImageData], indexed_count: usize, device: &wgpu::Device,
    queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout,
) -> SpriteTextures {
    const EDGE: u32 = 1024;
    if images.is_empty() || images.iter().any(|i| i.width == 0 || i.height == 0 || i.width > EDGE - 2 || i.height > EDGE - 2) {
        return upload_sprite_textures(images, indexed_count, device, queue, layout);
    }
    let mut bind_groups: Vec<Option<wgpu::BindGroup>> = vec![None; images.len()];
    let mut uv_rects = vec![[0.0; 4]; images.len()];
    for page in actor_atlas_plan(images, EDGE) {
        let (pixels, width, height) = actor_atlas_pixels(images, &page);
        for &(i, x, y) in &page {
            let image = &images[i];
            uv_rects[i] = [(x+1) as f32 / width as f32, (y+1) as f32 / height as f32,
                (x+1+image.width) as f32 / width as f32, (y+1+image.height) as f32 / height as f32];
        }
        let group = create_texture_bind_group_from_rgba(device, queue, &pixels, width, height,
            layout, "actor_frame_atlas", sprite_filter(), wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::AddressMode::ClampToEdge);
        for (i, _, _) in page { bind_groups[i] = Some(group.clone()); }
    }
    SpriteTextures { bind_groups: Arc::new(bind_groups.into_iter().map(Option::unwrap).collect()),
        uv_rects: Arc::new(uv_rects), sizes: images.iter().map(|i| (i.width, i.height)).collect(), indexed_count }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteUniforms {
    pub screen_size: [f32; 2],
    pub zoom: f32,
    pub _pad: f32,
    pub pan: [f32; 2],
    pub _pad2: [f32; 2],
    pub world_light: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_enabled: f32,
    /// Whether the fragment shader snaps the sample point to the texel grid, so
    /// filtering only softens the seam between two texels.
    pub sharpen: f32,
    /// The camera planes the vertex depth was projected with. A sprite quad
    /// carries an NDC depth and no world position, so the shader inverts the
    /// projection to get the eye distance the fog ramp needs.
    pub clip_near: f32,
    pub clip_far: f32,
    pub _pad4: [f32; 2],
}

impl Default for SpriteUniforms {
    fn default() -> Self {
        Self {
            screen_size: [1.0, 1.0],
            zoom: 1.0,
            _pad: 0.0,
            pan: [0.0, 0.0],
            _pad2: [0.0, 0.0],
            world_light: [1.0; 4],
            fog_color: [0.0; 4],
            fog_near: 0.0,
            fog_far: 1.0,
            fog_enabled: 0.0,
            sharpen: 0.0,
            clip_near: 1.0,
            clip_far: 1.0,
            _pad4: [0.0, 0.0],
        }
    }
}

const INITIAL_VERTEX_CAPACITY: usize = 1024;
const INITIAL_INDEX_CAPACITY: usize = 2048;

#[derive(Clone, Copy, PartialEq)]
enum SpriteDepth {
    None,
    Test {
        write: bool,
    },
    Overlay,
    /// Depth-write only, no colour. Stamps the opaque body silhouette so later
    /// passes (effects) occlude against it, while the colour pass itself writes
    /// no depth — coplanar body layers/copies can never reject each other.
    DepthOnly,
}

pub struct SpriteRenderer {
    pub pipelines: SpritePipelines,
    uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    meshes: MeshBatcher<SpriteVertex>,
    silhouette_meshes: MeshBatcher<SpriteVertex>,
    /// For [`Self::render_second`], made on its first use
    second_meshes: Option<MeshBatcher<SpriteVertex>>,
    depth_write: bool,
    uniforms: SpriteUniforms,
}

/// Every pipeline a sprite renderer draws with, built from one shader: at start and on a shader reload alike.
pub struct SpritePipelines {
    pub normal: wgpu::RenderPipeline,
    pub no_depth: wgpu::RenderPipeline,
    pub additive: wgpu::RenderPipeline,
    pub additive_no_depth: wgpu::RenderPipeline,
    pub overlay: wgpu::RenderPipeline,
    pub additive_overlay: wgpu::RenderPipeline,
    pub depth_only: wgpu::RenderPipeline,
    /// A weapon's aura (`fs_weapon_aura`), additive, testing depth without writing it
    pub weapon_aura: wgpu::RenderPipeline,
    pub weapon_aura_no_depth: wgpu::RenderPipeline,
}

impl SpritePipelines {
    fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
        shader_source: &str,
        depth_write: bool,
    ) -> Self {
        let alpha = wgpu::BlendState::ALPHA_BLENDING;
        let additive_component = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let additive = wgpu::BlendState {
            color: additive_component,
            alpha: additive_component,
        };
        let test = SpriteDepth::Test { write: depth_write };
        let build = |entry: &str, blend: wgpu::BlendState, depth: SpriteDepth| {
            SpriteRenderer::create_pipeline(
                device,
                surface_format,
                uniform_layout,
                texture_layout,
                shader_source,
                entry,
                blend,
                depth,
            )
        };
        Self {
            normal: build("fs_main", alpha, test),
            no_depth: build("fs_main", alpha, SpriteDepth::None),
            additive: build("fs_main", additive, test),
            additive_no_depth: build("fs_main", additive, SpriteDepth::None),
            overlay: build("fs_main", alpha, SpriteDepth::Overlay),
            additive_overlay: build("fs_main", additive, SpriteDepth::Overlay),
            depth_only: build("fs_main", alpha, SpriteDepth::DepthOnly),
            weapon_aura: build(
                "fs_weapon_aura",
                additive,
                SpriteDepth::Test { write: false },
            ),
            weapon_aura_no_depth: build("fs_weapon_aura", additive, SpriteDepth::None),
        }
    }
}

impl SpriteRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        logical_width: f32,
        logical_height: f32,
        shader_source: &str,
        depth_write: bool,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let uniform_data = SpriteUniforms {
            screen_size: [logical_width, logical_height],
            ..Default::default()
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_uniforms"),
            contents: bytemuck::cast_slice(&[uniform_data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite_uniforms"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_uniforms"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipelines = SpritePipelines::new(
            device,
            surface_format,
            &uniform_bind_group_layout,
            texture_bind_group_layout,
            shader_source,
            depth_write,
        );

        Self {
            pipelines,
            uniform_buffer,
            uniform_bind_group,
            uniform_bind_group_layout,
            meshes: MeshBatcher::new(
                device,
                "sprite",
                INITIAL_VERTEX_CAPACITY,
                INITIAL_INDEX_CAPACITY,
            ),
            silhouette_meshes: MeshBatcher::new(
                device,
                "sprite_silhouette",
                INITIAL_VERTEX_CAPACITY,
                INITIAL_INDEX_CAPACITY,
            ),
            second_meshes: None,
            depth_write,
            uniforms: uniform_data,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
        shader_source: &str,
        fragment_entry: &str,
        blend: wgpu::BlendState,
        depth: SpriteDepth,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(uniform_layout), Some(texture_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SpriteVertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fragment_entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(blend),
                    write_mask: if depth == SpriteDepth::DepthOnly {
                        wgpu::ColorWrites::empty()
                    } else {
                        wgpu::ColorWrites::ALL
                    },
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: match depth {
                SpriteDepth::None => None,
                SpriteDepth::Test { write } => Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                SpriteDepth::Overlay => Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                SpriteDepth::DepthOnly => Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
            },
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn recreate_pipeline(
        &mut self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        texture_layout: &wgpu::BindGroupLayout,
        shader_source: &str,
    ) {
        self.pipelines = SpritePipelines::new(
            device,
            surface_format,
            &self.uniform_bind_group_layout,
            texture_layout,
            shader_source,
            self.depth_write,
        );
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &SpriteUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, logical_width: f32, logical_height: f32) {
        self.uniforms.screen_size = [logical_width, logical_height];
        self.uniforms.zoom = 1.0;
        self.uniforms.pan = [0.0, 0.0];
        self.update_uniforms(queue, &self.uniforms);
    }

    pub fn set_sharpen(&mut self, queue: &wgpu::Queue, on: bool) {
        self.uniforms.sharpen = if on { 1.0 } else { 0.0 };
        self.update_uniforms(queue, &self.uniforms);
    }

    pub fn set_world_light(&mut self, queue: &wgpu::Queue, light: [f32; 3]) {
        self.uniforms.world_light = [light[0], light[1], light[2], 1.0];
        self.update_uniforms(queue, &self.uniforms);
    }

    pub fn set_fog(
        &mut self,
        queue: &wgpu::Queue,
        fog: &crate::FogUniform,
        clip_near: f32,
        clip_far: f32,
    ) {
        self.uniforms.fog_color = fog.color;
        self.uniforms.fog_near = fog.near;
        self.uniforms.fog_far = fog.far;
        self.uniforms.fog_enabled = fog.enabled;
        self.uniforms.clip_near = clip_near;
        self.uniforms.clip_far = clip_far;
        self.update_uniforms(queue, &self.uniforms);
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: Option<&wgpu::TextureView>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        clear_color: Option<wgpu::Color>,
        batches: &[SpriteBatch],
    ) {
        let target = PassTarget {
            encoder,
            view: target_view,
            depth_view,
            clear_color,
        };
        draw_batches(
            &self.pipelines,
            &self.uniform_bind_group,
            &mut self.meshes,
            target,
            device,
            queue,
            batches,
        );
    }

    /// [`Self::render`] for a second set of batches in the same frame (the effects drawn before the sprites), from a
    /// mesh batcher of their own: one upload would otherwise overwrite the other before the frame is submitted
    pub fn render_second(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: Option<&wgpu::TextureView>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batches: &[SpriteBatch],
    ) {
        let meshes = self.second_meshes.get_or_insert_with(|| {
            MeshBatcher::new(
                device,
                "sprite_second",
                INITIAL_VERTEX_CAPACITY,
                INITIAL_INDEX_CAPACITY,
            )
        });
        let target = PassTarget {
            encoder,
            view: target_view,
            depth_view,
            clear_color: None,
        };
        draw_batches(
            &self.pipelines,
            &self.uniform_bind_group,
            meshes,
            target,
            device,
            queue,
            batches,
        );
    }

    /// Stamp a depth-only body silhouette (colour masked off) so effects drawn
    /// afterward occlude against the body. Call this *after* the colour `render`,
    /// with batches whose `z` is the flat feet/anchor depth (no gradient): that
    /// far, uniform depth lets effects above the feet (buffs, casts, auras) pass
    /// the later depth test and draw on top, while ground effects at the feet are
    /// occluded — the ordering the game had before per-pixel gradient depth.
    pub fn render_silhouette(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batches: &[SpriteBatch],
    ) {
        pack(&mut self.silhouette_meshes, batches);
        if self.silhouette_meshes.is_empty() {
            return;
        }
        self.silhouette_meshes.upload(device, queue);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite_silhouette"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        pass.set_pipeline(&self.pipelines.depth_only);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        self.silhouette_meshes.bind(&mut pass);
        for run in self.silhouette_meshes.runs() {
            pass.set_bind_group(1, batches[run.first].texture, &[]);
            pass.draw_indexed(run.indices.clone(), 0, 0..1);
        }
    }
}

/// Lays the batches out in `meshes` in order, neighbours of one texture and pipeline joined into one draw.
fn pack(meshes: &mut MeshBatcher<SpriteVertex>, batches: &[SpriteBatch]) {
    meshes.clear();
    for (i, batch) in batches.iter().enumerate() {
        meshes.push(i, &batch.vertices, batch.indices, |first| {
            batches[first].draws_with(batch)
        });
    }
}

/// A sprite batch's vertices: a quad inline, or any mesh.
pub type SpriteVertices = Vertices<SpriteVertex>;

pub struct SpriteBatch<'a> {
    pub vertices: SpriteVertices,
    pub indices: &'a [u32],
    pub texture: &'a wgpu::BindGroup,
    pub additive: bool,
    /// Skip the depth test entirely (the original's `RF_NODEPTHCHECK`).
    pub no_depth: bool,
    /// Drawn by the weapon aura shader (`fs_weapon_aura`, additive) instead of as a sprite.
    pub weapon_aura: bool,
}

impl SpriteBatch<'_> {
    /// Whether `other` draws with the same texture and pipeline, so both fit in one draw.
    fn draws_with(&self, other: &SpriteBatch) -> bool {
        std::ptr::eq(self.texture, other.texture)
            && (self.additive, self.no_depth, self.weapon_aura)
                == (other.additive, other.no_depth, other.weapon_aura)
    }
}

/// A sprite's images as the layer code reads them: their sizes, and how many are palette-indexed (an RGBA clip's
/// index counts from there). The game's sheets are on the GPU ([`SpriteTextures`]); a picture composed on the CPU
/// keeps the pixels ([`crate::portrait::CpuSheet`]).
pub trait SpriteSheet {
    fn uv_rect(&self, _index: usize) -> [f32; 4] { [0.0, 0.0, 1.0, 1.0] }
    fn indexed_count(&self) -> usize;
    fn image_size(&self, index: usize) -> Option<(u32, u32)>;
    /// How many images a clip can draw.
    fn drawable_count(&self) -> usize;
}

impl SpriteSheet for SpriteTextures {
    fn uv_rect(&self, index: usize) -> [f32; 4] {
        self.uv_rects.get(index).copied().unwrap_or([0.0, 0.0, 1.0, 1.0])
    }
    fn indexed_count(&self) -> usize {
        self.indexed_count
    }

    fn image_size(&self, index: usize) -> Option<(u32, u32)> {
        self.sizes.get(index).copied()
    }

    fn drawable_count(&self) -> usize {
        self.bind_groups.len()
    }
}

fn clip_texture_index(clip: &SpriteFrame, textures: &impl SpriteSheet) -> Option<usize> {
    if clip.sprite_index < 0 {
        return None;
    }
    let index = if clip.sprite_type == 0 {
        clip.sprite_index as usize
    } else {
        textures.indexed_count() + clip.sprite_index as usize
    };
    textures.image_size(index).map(|_| index)
}

/// Unscaled sprite pixels from a clip's anchor down to the bottom edge of its
/// quad. Clips are centred on the anchor, so this is where the sprite rests.
pub fn clip_bottom_offset(clip: &SpriteFrame, textures: &SpriteTextures) -> f32 {
    let Some(tex_index) = clip_texture_index(clip, textures) else {
        return 0.0;
    };
    let height = match clip.height {
        Some(h) if h > 0 => h as f32,
        _ => textures.sizes[tex_index].1 as f32,
    };
    clip.y as f32 + height * clip.zoom_y / 2.0
}

pub fn build_clip_quad(
    clip: &SpriteFrame,
    textures: &SpriteTextures,
    screen_anchor: [f32; 2],
    depth: f32,
    offset: [f32; 2],
) -> Option<ClipQuad> {
    sheet_clip_quad(clip, textures, screen_anchor, depth, offset)
}

/// [`build_clip_quad`] on any sheet.
fn sheet_clip_quad(
    clip: &SpriteFrame,
    textures: &impl SpriteSheet,
    screen_anchor: [f32; 2],
    depth: f32,
    offset: [f32; 2],
) -> Option<ClipQuad> {
    let tex_index = clip_texture_index(clip, textures)?;
    let (tex_w, tex_h) = textures.image_size(tex_index)?;
    let (w, h) = match (clip.width, clip.height) {
        (Some(cw), Some(ch)) if cw > 0 && ch > 0 => (cw as f32, ch as f32),
        _ => (tex_w as f32, tex_h as f32),
    };

    let scaled_w = w * clip.zoom_x;
    let scaled_h = h * clip.zoom_y;
    let half_w = scaled_w / 2.0;
    let half_h = scaled_h / 2.0;

    let cx = screen_anchor[0] + clip.x as f32 + offset[0];
    let cy = screen_anchor[1] + clip.y as f32 + offset[1];

    let [left, top, right, bottom] = textures.uv_rect(tex_index);
    let (u0, u1) = if clip.mirror != 0 {
        (right, left)
    } else {
        (left, right)
    };
    let (v0, v1) = (top, bottom);

    let color = [
        clip.color[0] as f32 / 255.0,
        clip.color[1] as f32 / 255.0,
        clip.color[2] as f32 / 255.0,
        clip.color[3] as f32 / 255.0,
    ];

    let corners = [
        [-half_w, -half_h],
        [half_w, -half_h],
        [half_w, half_h],
        [-half_w, half_h],
    ];

    let angle = (clip.angle as f32).to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];

    let vertices = std::array::from_fn(|i| {
        let [x, y] = corners[i];
        let rx = x * cos_a - y * sin_a;
        let ry = x * sin_a + y * cos_a;
        SpriteVertex {
            position: [cx + rx, cy + ry, depth],
            tex_coord: uvs[i],
            color,
        }
    });

    Some((vertices, tex_index))
}

/// Same as [`build_clip_quad`], but grows the quad about `screen_anchor` so the
/// sprite tracks the entity's on-screen size (`sprite_scale`) instead of staying
/// a fixed pixel size.
pub fn build_clip_quad_scaled(
    clip: &SpriteFrame,
    textures: &SpriteTextures,
    screen_anchor: [f32; 2],
    depth: f32,
    offset: [f32; 2],
    scale: f32,
) -> Option<ClipQuad> {
    let (mut vertices, tex_index) = build_clip_quad(clip, textures, screen_anchor, depth, offset)?;
    if scale != 1.0 {
        for v in &mut vertices {
            v.position[0] = screen_anchor[0] + (v.position[0] - screen_anchor[0]) * scale;
            v.position[1] = screen_anchor[1] + (v.position[1] - screen_anchor[1]) * scale;
        }
    }
    Some((vertices, tex_index))
}

/// A clip's quad (corners in [`QUAD_INDICES`] order) and its texture index.
pub type ClipQuad = ([SpriteVertex; 4], usize);

pub struct CompositeClips {
    pub body: Vec<ClipQuad>,
    pub head: Vec<ClipQuad>,
    pub headgear_bottom: Vec<ClipQuad>,
    pub headgear_mid: Vec<ClipQuad>,
    pub headgear_top: Vec<ClipQuad>,
    pub weapon: Vec<ClipQuad>,
    pub weapon_trail: Vec<ClipQuad>,
    pub shield: Vec<ClipQuad>,
    pub robe: Vec<ClipQuad>,
    pub robe_behind: bool,
    pub body_motion_idx: usize,
}

/// Which motion of a body-part action to render. For idle/sit the whole actor is
/// posed by the head-turn (doridori) frame, so every part uses `head_dir`; other
/// actions play their normal animation via `motion_idx`. A part with no frame at
/// `motion_idx` is not drawn.
pub(crate) fn part_motion_index(
    is_idle_or_sit: bool,
    head_dir: u8,
    motion_idx: usize,
    len: usize,
) -> Option<usize> {
    if len == 0 {
        None
    } else if is_idle_or_sit {
        Some(head_dir as usize % len)
    } else if motion_idx < len {
        Some(motion_idx)
    } else {
        None
    }
}

/// Action and motion of a headgear layer. A headgear with a whole multiple of
/// the head's frames animates at that multiple of the head's frame, and that
/// takes precedence over a substituted head pose.
pub(crate) fn headgear_pose(
    action_idx: usize,
    head: Option<(usize, usize)>,
    head_override: Option<usize>,
    is_idle_or_sit: bool,
    head_dir: u8,
    motion_idx: usize,
    len: usize,
) -> Option<(usize, usize)> {
    match head {
        Some((head_idx, head_len)) if len > head_len && len % head_len == 0 => {
            Some((action_idx, head_idx * (len / head_len)))
        }
        _ => match head_override {
            Some(action) => Some((action, 0)),
            None => Some((
                action_idx,
                part_motion_index(is_idle_or_sit, head_dir, motion_idx, len)?,
            )),
        },
    }
}

/// Frames of the TaeKwon kick and cast that pose the head apart from the body:
/// they draw the head and its headgear from the first frame of an idle
/// direction instead of the action's own frame. `sex` is 1 for male.
pub(crate) fn taekwon_head_action(action_idx: usize, motion_idx: usize, sex: u8) -> Option<usize> {
    let dir = action_idx % 8;
    match action_idx / 8 {
        11 => {
            let (first, second) = if sex == 1 { (3, 4) } else { (4, 5) };
            let turned = match dir {
                0 | 1 => (6, 5),
                2 | 3 => (6, 7),
                4 | 5 => (2, 1),
                _ => (2, 3),
            };
            if motion_idx == first {
                Some(turned.0)
            } else if motion_idx == second {
                Some(turned.1)
            } else {
                None
            }
        }
        12 if motion_idx == 1 => Some(match dir {
            0 | 1 => 0,
            2 | 3 => 6,
            4 | 5 => 4,
            _ => 2,
        }),
        _ => None,
    }
}

/// The clips of a part drawn on the body's anchor (weapon, its trail, shield, garment), at the pose
/// `part_motion_idx` picks for its frame count; none when the part has no such action or frame.
fn part_clips(
    act: &ActFile,
    textures: &impl SpriteSheet,
    action_idx: usize,
    part_motion_idx: &impl Fn(usize) -> Option<usize>,
    body_motion: &Motion,
    screen_anchor: [f32; 2],
    depth: f32,
) -> Vec<ClipQuad> {
    let Some(motion) = act
        .actions
        .get(action_idx)
        .and_then(|action| action.motions.get(part_motion_idx(action.motions.len())?))
    else {
        return Vec::new();
    };
    let (off_x, off_y) = attachment_offset(body_motion, motion);
    motion
        .clips
        .iter()
        .filter_map(|clip| {
            sheet_clip_quad(
                clip,
                textures,
                screen_anchor,
                depth,
                [off_x as f32, off_y as f32],
            )
            .filter(|(_, tex_idx)| *tex_idx < textures.drawable_count())
        })
        .collect()
}

pub fn build_composite_clips<T: SpriteSheet>(
    entity: &EntitySprite<T>,
    action_idx: usize,
    motion_idx: usize,
    head_dir: u8,
    screen_anchor: [f32; 2],
    depth: f32,
) -> Option<CompositeClips> {
    let body_action = &entity.body_act.actions[action_idx];
    if body_action.motions.is_empty() {
        return None;
    }

    let base_action = action_idx / 8;
    let is_idle_or_sit = (base_action == 0 || base_action == 2) && entity.head_act.is_some();

    let part_motion_idx = |len: usize| part_motion_index(is_idle_or_sit, head_dir, motion_idx, len);
    let body_motion_idx = part_motion_idx(body_action.motions.len())?;
    let body_motion = &body_action.motions[body_motion_idx];

    let mut body = Vec::new();
    for clip in &body_motion.clips {
        if let Some((vertices, tex_idx)) = sheet_clip_quad(
            clip,
            &entity.body_textures,
            screen_anchor,
            depth,
            [0.0, 0.0],
        ) && tex_idx < entity.body_textures.drawable_count()
        {
            body.push((vertices, tex_idx));
        }
    }

    let head_override = entity
        .taekwon_sex
        .and_then(|sex| taekwon_head_action(action_idx, motion_idx, sex));

    let mut head = Vec::new();
    let mut head_anchor: Option<(&Motion, [f32; 2])> = None;
    let mut head_layer = None;
    if let (Some(head_act), Some(head_tex)) = (&entity.head_act, &entity.head_textures) {
        head_layer = head_act
            .actions
            .get(action_idx)
            .and_then(|a| part_motion_idx(a.motions.len()).map(|m| (m, a.motions.len())));
        let head_pose = match head_override {
            Some(action) => Some((action, 0)),
            None => head_layer.map(|(m, _)| (action_idx, m)),
        };
        if let Some((head_action_idx, head_motion_idx)) = head_pose
            && let Some(head_motion) = head_act
                .actions
                .get(head_action_idx)
                .and_then(|a| a.motions.get(head_motion_idx))
        {
            let (off_x, off_y) = attachment_offset(body_motion, head_motion);
            let head_zoom = head_motion
                .clips
                .first()
                .map_or([1.0, 1.0], |c| [c.zoom_x, c.zoom_y]);
            head_anchor = Some((head_motion, head_zoom));
            for clip in &head_motion.clips {
                if let Some((vertices, tex_idx)) = sheet_clip_quad(
                    clip,
                    head_tex,
                    screen_anchor,
                    depth,
                    [off_x as f32, off_y as f32],
                ) && tex_idx < head_tex.drawable_count()
                {
                    head.push((vertices, tex_idx));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_headgear_clips(
        act: Option<&ActFile>,
        tex: Option<&impl SpriteSheet>,
        action_idx: usize,
        motion_idx: usize,
        head_dir: u8,
        is_idle_or_sit: bool,
        body_motion: &Motion,
        head_anchor: Option<(&Motion, [f32; 2])>,
        head_layer: Option<(usize, usize)>,
        head_override: Option<usize>,
        screen_anchor: [f32; 2],
        depth: f32,
    ) -> Vec<ClipQuad> {
        let mut clips = Vec::new();
        if let (Some(act), Some(tex)) = (act, tex) {
            if let Some(hg_base) = act.actions.get(action_idx)
                && let Some((hg_action_idx, hg_motion_idx)) = headgear_pose(
                    action_idx,
                    head_layer,
                    head_override,
                    is_idle_or_sit,
                    head_dir,
                    motion_idx,
                    hg_base.motions.len(),
                )
                && let Some(hg_motion) = act
                    .actions
                    .get(hg_action_idx)
                    .and_then(|a| a.motions.get(hg_motion_idx))
            {
                let body_offset = attachment_offset(body_motion, hg_motion);
                for clip in &hg_motion.clips {
                    let offset = match head_anchor {
                        Some((head_motion, head_zoom)) => {
                            let (x, y) = head_chained_attachment_offset(
                                body_motion,
                                head_motion,
                                head_zoom,
                                hg_motion,
                                [clip.zoom_x, clip.zoom_y],
                            );
                            [x, y]
                        }
                        None => [body_offset.0 as f32, body_offset.1 as f32],
                    };
                    if let Some((vertices, tex_idx)) =
                        sheet_clip_quad(clip, tex, screen_anchor, depth, offset)
                        && tex_idx < tex.drawable_count()
                    {
                        clips.push((vertices, tex_idx));
                    }
                }
            }
        }
        clips
    }

    let headgear_bottom = build_headgear_clips(
        entity.headgear_bottom_act.as_ref(),
        entity.headgear_bottom_textures.as_ref(),
        action_idx,
        motion_idx,
        head_dir,
        is_idle_or_sit,
        body_motion,
        head_anchor,
        head_layer,
        head_override,
        screen_anchor,
        depth,
    );
    let headgear_mid = build_headgear_clips(
        entity.headgear_mid_act.as_ref(),
        entity.headgear_mid_textures.as_ref(),
        action_idx,
        motion_idx,
        head_dir,
        is_idle_or_sit,
        body_motion,
        head_anchor,
        head_layer,
        head_override,
        screen_anchor,
        depth,
    );
    let headgear_top = build_headgear_clips(
        entity.headgear_top_act.as_ref(),
        entity.headgear_top_textures.as_ref(),
        action_idx,
        motion_idx,
        head_dir,
        is_idle_or_sit,
        body_motion,
        head_anchor,
        head_layer,
        head_override,
        screen_anchor,
        depth,
    );

    let part = |layer: Option<(&ActFile, &T)>| {
        layer.map_or_else(Vec::new, |(act, textures)| {
            part_clips(
                act,
                textures,
                action_idx,
                &part_motion_idx,
                body_motion,
                screen_anchor,
                depth,
            )
        })
    };
    let weapon = part(
        entity
            .weapon_act
            .as_ref()
            .zip(entity.weapon_textures.as_ref()),
    );
    let weapon_trail = part(
        entity
            .weapon_trail_act
            .as_ref()
            .zip(entity.weapon_trail_textures.as_ref()),
    );
    let shield = part(
        entity
            .shield_act
            .as_ref()
            .zip(entity.shield_textures.as_ref()),
    );
    let robe = part(
        entity
            .robe
            .as_ref()
            .map(|layer| (&layer.act, &layer.textures)),
    );
    let robe_behind = entity
        .robe
        .as_ref()
        .is_some_and(|layer| layer.is_behind(action_idx, body_motion_idx));

    Some(CompositeClips {
        body,
        head,
        headgear_bottom,
        headgear_mid,
        headgear_top,
        weapon,
        weapon_trail,
        shield,
        robe,
        robe_behind,
        body_motion_idx,
    })
}

pub fn scale_clip_vertices(
    vertices: &mut [SpriteVertex],
    center: [f32; 2],
    scale: f32,
    depth_gradient: [f32; 2],
) {
    for v in vertices {
        v.position[0] = center[0] + (v.position[0] - center[0]) * scale;
        v.position[1] = center[1] + (v.position[1] - center[1]) * scale;
        v.position[2] += depth_gradient[0] * (v.position[0] - center[0])
            + depth_gradient[1] * (v.position[1] - center[1]);
    }
}

pub fn rotate_sprite_vertices(vertices: &mut [SpriteVertex], center: [f32; 2], angle: f32) {
    let (sin, cos) = angle.sin_cos();
    for v in vertices {
        let dx = v.position[0] - center[0];
        let dy = v.position[1] - center[1];
        v.position[0] = center[0] + dx * cos - dy * sin;
        v.position[1] = center[1] + dx * sin + dy * cos;
    }
}

/// An actor's layers. `T` holds each part's images: on the GPU for the game ([`SpriteTextures`]), or as pixels for a
/// picture composed on the CPU ([`crate::portrait::CpuSheet`]).
pub struct EntitySprite<T = SpriteTextures> {
    pub body_textures: T,
    pub body_act: ActFile,
    pub head_textures: Option<T>,
    pub head_act: Option<ActFile>,
    pub weapon_textures: Option<T>,
    pub weapon_act: Option<ActFile>,
    pub weapon_trail_textures: Option<T>,
    pub weapon_trail_act: Option<ActFile>,
    pub headgear_top_textures: Option<T>,
    pub headgear_top_act: Option<ActFile>,
    pub headgear_mid_textures: Option<T>,
    pub headgear_mid_act: Option<ActFile>,
    pub headgear_bottom_textures: Option<T>,
    pub headgear_bottom_act: Option<ActFile>,
    pub shield_textures: Option<T>,
    pub shield_act: Option<ActFile>,
    pub shadow_textures: Option<T>,
    pub shadow_act: Option<ActFile>,
    pub robe: Option<RobeLayer<T>>,
    pub layer_order: Option<ImfLayerOrder>,
    /// Set to the actor's sex on the TaeKwon line, whose kick and cast frames
    /// pose the head apart from the body.
    pub taekwon_sex: Option<u8>,
}

/// A garment: drawn over everything but on the frames `behind` lists per action, where it goes behind the body.
pub struct RobeLayer<T = SpriteTextures> {
    pub textures: T,
    pub act: ActFile,
    pub behind: HashMap<u16, Vec<u16>>,
}

impl<T> RobeLayer<T> {
    fn is_behind(&self, action_idx: usize, motion_idx: usize) -> bool {
        self.behind
            .get(&(action_idx as u16))
            .is_some_and(|frames| frames.contains(&(motion_idx as u16)))
    }
}

impl<T> EntitySprite<T> {
    pub fn with_robe(mut self, robe: Option<RobeLayer<T>>) -> Self {
        self.robe = robe;
        self
    }

    pub fn with_layer_order(mut self, layer_order: Option<ImfLayerOrder>) -> Self {
        self.layer_order = layer_order;
        self
    }

    pub fn with_taekwon_sex(mut self, sex: Option<u8>) -> Self {
        self.taekwon_sex = sex;
        self
    }

    fn body_layer_count(&self) -> u8 {
        1 + [
            self.head_act.is_some(),
            self.headgear_top_act.is_some(),
            self.headgear_mid_act.is_some(),
            self.headgear_bottom_act.is_some(),
        ]
        .into_iter()
        .filter(|&present| present)
        .count() as u8
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_entity_sprite(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    body: SpriteData,
    head: Option<SpriteData>,
    weapon: Option<SpriteData>,
    weapon_trail: Option<SpriteData>,
    headgear_top: Option<SpriteData>,
    headgear_mid: Option<SpriteData>,
    headgear_bottom: Option<SpriteData>,
    shield: Option<SpriteData>,
    shadow: Option<SpriteData>,
) -> EntitySprite {
    nostalre_profiling::profile_function!();
    EntitySprite::from_parts(
        body,
        head,
        weapon,
        weapon_trail,
        headgear_top,
        headgear_mid,
        headgear_bottom,
        shield,
        shadow,
        |data| {
            let textures =
                upload_sprite_textures(&data.images, data.indexed_count, device, queue, layout);
            (textures, data.act)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_entity_sprite_cached(
    device: &wgpu::Device, queue: &wgpu::Queue, cache: &crate::texture::TextureCache,
    body: SpriteData, head: Option<SpriteData>, weapon: Option<SpriteData>,
    weapon_trail: Option<SpriteData>, headgear_top: Option<SpriteData>,
    headgear_mid: Option<SpriteData>, headgear_bottom: Option<SpriteData>,
    shield: Option<SpriteData>, shadow: Option<SpriteData>,
) -> EntitySprite {
    EntitySprite::from_parts(body, head, weapon, weapon_trail, headgear_top,
        headgear_mid, headgear_bottom, shield, shadow, |data| {
            let textures = cache.sprite_sheets.upload(&data, device, queue, &cache.bind_group_layout);
            (textures, data.act)
        })
}

impl<T> EntitySprite<T> {
    /// An actor from its parts' sprites, each part's images made into a sheet by `sheet`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        body: SpriteData,
        head: Option<SpriteData>,
        weapon: Option<SpriteData>,
        weapon_trail: Option<SpriteData>,
        headgear_top: Option<SpriteData>,
        headgear_mid: Option<SpriteData>,
        headgear_bottom: Option<SpriteData>,
        shield: Option<SpriteData>,
        shadow: Option<SpriteData>,
        mut sheet: impl FnMut(SpriteData) -> (T, ActFile),
    ) -> Self {
        let (body_textures, body_act) = sheet(body);
        let mut part = |data: Option<SpriteData>| data.map(&mut sheet).unzip();
        let (head_textures, head_act) = part(head);
        let (weapon_textures, weapon_act) = part(weapon);
        let (weapon_trail_textures, weapon_trail_act) = part(weapon_trail);
        let (headgear_top_textures, headgear_top_act) = part(headgear_top);
        let (headgear_mid_textures, headgear_mid_act) = part(headgear_mid);
        let (headgear_bottom_textures, headgear_bottom_act) = part(headgear_bottom);
        let (shield_textures, shield_act) = part(shield);
        let (shadow_textures, shadow_act) = part(shadow);
        EntitySprite {
            body_textures,
            body_act,
            head_textures,
            head_act,
            weapon_textures,
            weapon_act,
            weapon_trail_textures,
            weapon_trail_act,
            headgear_top_textures,
            headgear_top_act,
            headgear_mid_textures,
            headgear_mid_act,
            headgear_bottom_textures,
            headgear_bottom_act,
            shield_textures,
            shield_act,
            shadow_textures,
            shadow_act,
            robe: None,
            layer_order: None,
            taekwon_sex: None,
        }
    }

    /// The sheet `part`'s clips are drawn from.
    pub fn sheet(&self, part: SpritePart) -> Option<&T> {
        match part {
            SpritePart::Body => Some(&self.body_textures),
            SpritePart::Head => self.head_textures.as_ref(),
            SpritePart::HeadgearBottom => self.headgear_bottom_textures.as_ref(),
            SpritePart::HeadgearMid => self.headgear_mid_textures.as_ref(),
            SpritePart::HeadgearTop => self.headgear_top_textures.as_ref(),
            SpritePart::Weapon => self.weapon_textures.as_ref(),
            SpritePart::WeaponTrail => self.weapon_trail_textures.as_ref(),
            SpritePart::Shield => self.shield_textures.as_ref(),
            SpritePart::Robe => self.robe.as_ref().map(|robe| &robe.textures),
        }
    }
}

/// A layer of a composed actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpritePart {
    Body,
    Head,
    HeadgearBottom,
    HeadgearMid,
    HeadgearTop,
    Weapon,
    WeaponTrail,
    Shield,
    Robe,
}

impl SpritePart {
    /// What the hands hold and the back wears: a body copy of only the body's layers leaves them out.
    fn is_gear(self) -> bool {
        matches!(
            self,
            Self::Weapon | Self::WeaponTrail | Self::Shield | Self::Robe
        )
    }
}

impl CompositeClips {
    /// The clips back to front, in the order the game draws them for `action_idx` (its direction puts the shield
    /// behind or in front; `layer_order`, the job's imf, may put the body over the head).
    pub fn into_draw_order(
        self,
        action_idx: usize,
        layer_order: Option<&ImfLayerOrder>,
    ) -> Vec<(SpritePart, ClipQuad)> {
        let dir = action_idx % 8;
        let shield_behind = dir > 1 && dir < 6;
        let body_over_head =
            layer_order.is_some_and(|order| order.body_over_head(action_idx, self.body_motion_idx));
        let (first, second) = if body_over_head {
            ((SpritePart::Head, self.head), (SpritePart::Body, self.body))
        } else {
            ((SpritePart::Body, self.body), (SpritePart::Head, self.head))
        };
        let shield = (SpritePart::Shield, self.shield);
        let robe = (SpritePart::Robe, self.robe);
        let (shield_back, shield_front) = if shield_behind {
            (Some(shield), None)
        } else {
            (None, Some(shield))
        };
        let (robe_back, robe_front) = if self.robe_behind {
            (Some(robe), None)
        } else {
            (None, Some(robe))
        };
        shield_back
            .into_iter()
            .chain(robe_back)
            .chain([
                first,
                second,
                (SpritePart::HeadgearBottom, self.headgear_bottom),
                (SpritePart::HeadgearTop, self.headgear_top),
                (SpritePart::HeadgearMid, self.headgear_mid),
                (SpritePart::Weapon, self.weapon),
                (SpritePart::WeaponTrail, self.weapon_trail),
            ])
            .chain(shield_front)
            // over the body, the robe goes last: kRO ranks it above every headgear (`tb_layer_priority`'s
            // Default_Robe)
            .chain(robe_front)
            .flat_map(|(part, quads)| quads.into_iter().map(move |quad| (part, quad)))
            .collect()
    }
}

impl EntitySprite {
    pub fn compute_pick_bounds(
        &self,
        animation: &nostalre_formats::act::SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        screen_anchor: [f32; 2],
        depth: f32,
        scale: f32,
        screen_w: f32,
    ) -> [f32; 4] {
        let action_idx = match camera_dir {
            Some(dir) => animation.action_index(&self.body_act, dir),
            None => animation.flat_action_index(&self.body_act),
        };

        let clips = build_composite_clips(
            self,
            action_idx,
            animation.motion_index(),
            head_dir,
            screen_anchor,
            depth,
        );

        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut has_vertices = false;

        if let Some(clips) = clips {
            for group in [&clips.body, &clips.head] {
                for (vertices, _) in group {
                    for v in vertices {
                        let sx = screen_anchor[0] + (v.position[0] - screen_anchor[0]) * scale;
                        let sy = screen_anchor[1] + (v.position[1] - screen_anchor[1]) * scale;
                        min_x = min_x.min(sx);
                        min_y = min_y.min(sy);
                        max_x = max_x.max(sx);
                        max_y = max_y.max(sy);
                        has_vertices = true;
                    }
                }
            }
        }

        if !has_vertices {
            let half = 20.0 * scale;
            return [
                screen_anchor[0] - half,
                screen_anchor[1] - half * 2.0,
                screen_anchor[0] + half,
                screen_anchor[1],
            ];
        }

        crate::sprite_projection::pick_bounds_from_drawn(
            [min_x, min_y, max_x, max_y],
            screen_anchor,
            screen_w,
        )
    }

    pub fn compute_head_offset(
        &self,
        animation: &nostalre_formats::act::SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        screen_anchor: [f32; 2],
        depth: f32,
        scale: f32,
    ) -> f32 {
        let action_count = self.body_act.actions.len();
        if action_count == 0 {
            return 40.0 * scale;
        }
        let idle_action_idx = match camera_dir {
            Some(dir) => (dir as usize + 12 - animation.direction()) % 8 % action_count,
            None => animation.direction() % action_count,
        };

        let Some(clips) =
            build_composite_clips(self, idle_action_idx, 0, head_dir, screen_anchor, depth)
        else {
            return 40.0 * scale;
        };

        let all_groups: [&Vec<ClipQuad>; 7] = [
            &clips.body,
            &clips.head,
            &clips.headgear_bottom,
            &clips.headgear_mid,
            &clips.headgear_top,
            &clips.weapon,
            &clips.shield,
        ];
        let mut min_y = f32::MAX;
        let mut has_vertices = false;
        for group in all_groups {
            for (vertices, _) in group {
                for v in vertices {
                    let sy = screen_anchor[1] + (v.position[1] - screen_anchor[1]) * scale;
                    if sy < min_y {
                        min_y = sy;
                    }
                    has_vertices = true;
                }
            }
        }

        if !has_vertices {
            return 40.0 * scale;
        }
        (screen_anchor[1] - min_y).max(0.0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_batches(
        &self,
        animation: &nostalre_formats::act::SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        screen_anchor: [f32; 2],
        depth: f32,
        scale: f32,
        depth_gradient: [f32; 2],
    ) -> Vec<SpriteBatch<'_>> {
        self.build_layers(
            animation,
            camera_dir,
            head_dir,
            screen_anchor,
            depth,
            scale,
            depth_gradient,
            nostalre_effects::WeaponLight::None,
            false,
        )
    }

    /// `build_batches`, with the weapon layer optionally lit and the weapon and
    /// shield optionally dropped. A lit weapon keeps its slot in the layer order.
    #[allow(clippy::too_many_arguments)]
    fn build_layers(
        &self,
        animation: &nostalre_formats::act::SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        screen_anchor: [f32; 2],
        depth: f32,
        scale: f32,
        depth_gradient: [f32; 2],
        weapon_light: nostalre_effects::WeaponLight,
        body_layers_only: bool,
    ) -> Vec<SpriteBatch<'_>> {
        let action_idx = match camera_dir {
            Some(dir) => animation.action_index(&self.body_act, dir),
            None => animation.flat_action_index(&self.body_act),
        };

        let Some(clips) = build_composite_clips(
            self,
            action_idx,
            animation.motion_index(),
            head_dir,
            screen_anchor,
            depth,
        ) else {
            return Vec::new();
        };

        let mut batches = Vec::new();
        for (part, (mut vertices, tex_idx)) in
            clips.into_draw_order(action_idx, self.layer_order.as_ref())
        {
            if body_layers_only && part.is_gear() {
                continue;
            }
            let Some(sheet) = self.sheet(part) else {
                continue;
            };
            scale_clip_vertices(&mut vertices, screen_anchor, scale, depth_gradient);
            let texture = &sheet.bind_groups[tex_idx];
            let batch = |vertices: [SpriteVertex; 4], additive: bool| SpriteBatch {
                vertices: vertices.into(),
                indices: QUAD_INDICES,
                texture,
                additive,
                no_depth: false,
                weapon_aura: false,
            };
            if part != SpritePart::Weapon {
                batches.push(batch(vertices, false));
                continue;
            }
            use nostalre_effects::WeaponLight;
            let aura_quad = match weapon_light {
                WeaponLight::Aura(aura) => Some(weapon_aura_quad(&vertices, &aura, scale)),
                _ => None,
            };
            if weapon_light == WeaponLight::Spark {
                batches.push(batch(vertices, false));
            }
            // an aura goes around the weapon, drawn as it is
            batches.push(batch(
                vertices,
                matches!(weapon_light, WeaponLight::Spark | WeaponLight::Glow),
            ));
            if let Some(aura_quad) = aura_quad {
                batches.push(SpriteBatch {
                    vertices: aura_quad.into(),
                    indices: QUAD_INDICES,
                    texture,
                    additive: true,
                    no_depth: false,
                    weapon_aura: true,
                });
            }
        }
        batches
    }

    /// Composes only the head (+ headgear) layers, auto-fitted into a square of
    /// side `target_px` centered on `center`. Used for the face icons shown in
    /// member/roster list rows.
    pub fn build_head_batches(
        &self,
        animation: &nostalre_formats::act::SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        center: [f32; 2],
        target_px: f32,
        depth: f32,
    ) -> Vec<SpriteBatch<'_>> {
        let action_idx = match camera_dir {
            Some(dir) => animation.action_index(&self.body_act, dir),
            None => animation.flat_action_index(&self.body_act),
        };
        let Some(clips) = build_composite_clips(
            self,
            action_idx,
            animation.motion_index(),
            head_dir,
            [0.0, 0.0],
            depth,
        ) else {
            return Vec::new();
        };

        let mut groups: Vec<(&SpriteTextures, Vec<ClipQuad>)> = Vec::new();
        if let Some(t) = &self.head_textures {
            groups.push((t, clips.head));
        }
        if let Some(t) = &self.headgear_bottom_textures {
            groups.push((t, clips.headgear_bottom));
        }
        if let Some(t) = &self.headgear_top_textures {
            groups.push((t, clips.headgear_top));
        }
        if let Some(t) = &self.headgear_mid_textures {
            groups.push((t, clips.headgear_mid));
        }

        let mut min = [f32::MAX, f32::MAX];
        let mut max = [f32::MIN, f32::MIN];
        for (_, quads) in &groups {
            for (verts, _) in quads {
                for v in verts {
                    min[0] = min[0].min(v.position[0]);
                    min[1] = min[1].min(v.position[1]);
                    max[0] = max[0].max(v.position[0]);
                    max[1] = max[1].max(v.position[1]);
                }
            }
        }
        if min[0] > max[0] {
            return Vec::new();
        }
        let span = (max[0] - min[0]).max(max[1] - min[1]).max(1.0);
        let fit = target_px / span;
        let cx = (min[0] + max[0]) / 2.0;
        let cy = (min[1] + max[1]) / 2.0;

        let mut batches = Vec::new();
        for (tex, quads) in groups {
            for (mut verts, tex_idx) in quads {
                if tex_idx >= tex.bind_groups.len() {
                    continue;
                }
                for v in &mut verts {
                    v.position[0] = (v.position[0] - cx) * fit + center[0];
                    v.position[1] = (v.position[1] - cy) * fit + center[1];
                }
                batches.push(SpriteBatch {
                    vertices: verts.into(),
                    indices: QUAD_INDICES,
                    texture: &tex.bind_groups[tex_idx],
                    additive: false,
                    no_depth: false,
                    weapon_aura: false,
                });
            }
        }
        batches
    }

    pub fn build_shadow_batches(
        &self,
        screen_anchor: [f32; 2],
        depth: f32,
        scale: f32,
        depth_gradient: [f32; 2],
    ) -> Vec<SpriteBatch<'_>> {
        match (&self.shadow_act, &self.shadow_textures) {
            (Some(act), Some(tex)) => {
                build_shadow_batches(act, tex, screen_anchor, depth, scale, depth_gradient)
            }
            _ => Vec::new(),
        }
    }
}

/// Shadow blob under an actor or a floor item: action 0 motion 0 of `shadow.act`,
/// scaled about the ground anchor.
pub fn build_shadow_batches<'a>(
    shadow_act: &ActFile,
    shadow_tex: &'a SpriteTextures,
    screen_anchor: [f32; 2],
    depth: f32,
    scale: f32,
    depth_gradient: [f32; 2],
) -> Vec<SpriteBatch<'a>> {
    let mut batches = Vec::new();
    if shadow_act.actions.is_empty() || shadow_act.actions[0].motions.is_empty() {
        return batches;
    }
    for clip in &shadow_act.actions[0].motions[0].clips {
        if let Some((mut vertices, tex_idx)) =
            build_clip_quad(clip, shadow_tex, screen_anchor, depth, [0.0, 0.0])
            && tex_idx < shadow_tex.bind_groups.len()
        {
            scale_clip_vertices(&mut vertices, screen_anchor, scale, depth_gradient);
            batches.push(SpriteBatch {
                vertices: vertices.into(),
                indices: QUAD_INDICES,
                texture: &shadow_tex.bind_groups[tex_idx],
                additive: false,
                no_depth: false,
                weapon_aura: false,
            });
        }
    }
    batches
}

#[derive(Clone, Debug)]
pub struct BodyChannels {
    /// Per-edge jitter in screen pixels: top, bottom, left, right.
    pub edge_jitter: [f32; 4],
    pub tint: Option<[u8; 3]>,
    /// Ground-lightmap intensity of the cell the actor stands on.
    pub light: [f32; 3],
    pub scale: f32,
    pub yaw: f32,
    pub alpha: f32,
    pub lift_px: f32,
    pub angle: f32,
    pub squeeze: f32,
    pub additive: bool,
    pub weapon_light: nostalre_effects::WeaponLight,
    /// Screen pixels per `BodyCopy::margin_px` unit — `400 / camera distance`,
    /// so a copy's outline keeps its share of the sprite at any zoom.
    pub copy_margin_scale: f32,
    pub copies: Vec<nostalre_effects::BodyCopy>,
}

impl Default for BodyChannels {
    fn default() -> Self {
        Self {
            edge_jitter: [0.0; 4],
            tint: None,
            light: [1.0; 3],
            scale: 1.0,
            yaw: 0.0,
            alpha: 1.0,
            lift_px: 0.0,
            angle: 0.0,
            squeeze: 1.0,
            additive: false,
            weapon_light: nostalre_effects::WeaponLight::None,
            copy_margin_scale: 1.0,
            copies: Vec::new(),
        }
    }
}

pub fn transform_batch_vertices(
    batches: &mut [SpriteBatch],
    anchor: [f32; 2],
    radians: f32,
    scale: [f32; 2],
) {
    if radians == 0.0 && scale == [1.0, 1.0] {
        return;
    }
    let (sin, cos) = radians.sin_cos();
    for batch in batches {
        for v in &mut batch.vertices {
            let dx = (v.position[0] - anchor[0]) * scale[0];
            let dy = (v.position[1] - anchor[1]) * scale[1];
            v.position[0] = anchor[0] + dx * cos - dy * sin;
            v.position[1] = anchor[1] + dx * sin + dy * cos;
        }
    }
}

/// Like `transform_batch_vertices`, but also re-evaluates each vertex's depth for
/// the screen-space move it made. `build_batches` encodes z as an affine function
/// of screen position (the depth gradient); scaling a body copy without updating z
/// leaves it stale, so the copy's silhouette would write a wrong depth in the
/// prepass (e.g. its enlarged bottom edge carrying the nearer z of its original
/// position). Adding `grad · Δpos` keeps the copy on the same depth plane as the
/// live body, so the prepass stamps one consistent silhouette.
pub fn transform_batch_vertices_with_depth(
    batches: &mut [SpriteBatch],
    anchor: [f32; 2],
    radians: f32,
    scale: [f32; 2],
    depth_gradient: [f32; 2],
) {
    if radians == 0.0 && scale == [1.0, 1.0] {
        return;
    }
    let (sin, cos) = radians.sin_cos();
    for batch in batches {
        for v in &mut batch.vertices {
            let (ox, oy) = (v.position[0], v.position[1]);
            let dx = (ox - anchor[0]) * scale[0];
            let dy = (oy - anchor[1]) * scale[1];
            let nx = anchor[0] + dx * cos - dy * sin;
            let ny = anchor[1] + dx * sin + dy * cos;
            v.position[0] = nx;
            v.position[1] = ny;
            v.position[2] += depth_gradient[0] * (nx - ox) + depth_gradient[1] * (ny - oy);
        }
    }
}

/// How far a weapon aura's quad reaches past the weapon clip on every side, in screen pixels at scale 1
const WEAPON_AURA_MARGIN_PX: f32 = 10.0;
/// The aura shader's clock wraps every this many seconds (`AURA_PERIOD` in `sprite.wgsl`)
const WEAPON_AURA_PERIOD_SECS: f32 = 100.0;

/// A colour as the aura shader reads it from a vertex: 5 bits a channel in one whole number, which interpolation
/// across the quad leaves exact
fn pack_aura_colour([r, g, b]: [u8; 3]) -> f32 {
    (((r as u32 >> 3) << 10) | ((g as u32 >> 3) << 5) | (b as u32 >> 3)) as f32
}

/// The quad the weapon aura shader (`fs_weapon_aura`) draws an aura around a weapon clip in (`WeaponLight::Aura`):
/// the clip grown by `WEAPON_AURA_MARGIN_PX` along its own axes, its texture coordinates extended past 0..1 to
/// match, so the weapon's texels stay where they are and the aura gets room around them. The vertex colour carries
/// the clock, the two colours and the style.
fn weapon_aura_quad(
    clip: &[SpriteVertex; 4],
    aura: &nostalre_effects::WeaponAura,
    scale: f32,
) -> [SpriteVertex; 4] {
    let [a, b, _, d] = clip;
    let sub = |p: [f32; 3], q: [f32; 3]| [p[0] - q[0], p[1] - q[1]];
    // the clip's own axes: across (a to b) and down (a to d), however it is turned
    let (across, down) = (sub(b.position, a.position), sub(d.position, a.position));
    let len = |v: [f32; 2]| v[0].hypot(v[1]).max(1e-3);
    let margin = WEAPON_AURA_MARGIN_PX * scale;
    let grow = [
        (len(across) + 2.0 * margin) / len(across),
        (len(down) + 2.0 * margin) / len(down),
    ];
    let n = 4.0;
    let centre = clip.iter().fold([0.0f32; 2], |c, v| {
        [c[0] + v.position[0] / n, c[1] + v.position[1] / n]
    });
    let colour = [
        (aura.frame as f32 / 60.0 / WEAPON_AURA_PERIOD_SECS).fract(),
        pack_aura_colour(aura.inner),
        pack_aura_colour(aura.outer),
        match aura.style {
            nostalre_effects::WeaponAuraStyle::Flame => 0.0,
            nostalre_effects::WeaponAuraStyle::Pulse => 1.0,
        },
    ];
    clip.map(|v| {
        let off = [v.position[0] - centre[0], v.position[1] - centre[1]];
        let along = |axis: [f32; 2]| (off[0] * axis[0] + off[1] * axis[1]) / len(axis).powi(2);
        let (s, t) = (along(across) * grow[0], along(down) * grow[1]);
        let mut out = v;
        out.position[0] = centre[0] + across[0] * s + down[0] * t;
        out.position[1] = centre[1] + across[1] * s + down[1] * t;
        out.tex_coord = [
            0.5 + (v.tex_coord[0] - 0.5) * grow[0],
            0.5 + (v.tex_coord[1] - 0.5) * grow[1],
        ];
        out.color = colour;
        out
    })
}

fn apply_tint_alpha(batches: &mut [SpriteBatch], tint: Option<[u8; 3]>, alpha: f32) {
    let tint = tint.map(|t| {
        [
            t[0] as f32 / 255.0,
            t[1] as f32 / 255.0,
            t[2] as f32 / 255.0,
        ]
    });
    for batch in batches {
        for v in &mut batch.vertices {
            if let Some([tr, tg, tb]) = tint {
                v.color[0] *= tr;
                v.color[1] *= tg;
                v.color[2] *= tb;
            }
            if alpha != 1.0 {
                v.color[3] *= alpha;
            }
        }
    }
}

/// Pushes every edge of each clip out by `margin` screen pixels, so the whole
/// silhouette gains an even outline instead of stretching about a shared centre.
fn grow_clips(batches: &mut [SpriteBatch], margin: f32, depth_gradient: [f32; 2]) {
    if margin == 0.0 {
        return;
    }
    for batch in batches {
        grow_clip(&mut batch.vertices, margin, depth_gradient);
    }
}

fn grow_clip(vertices: &mut [SpriteVertex], margin: f32, depth_gradient: [f32; 2]) {
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for v in vertices.iter() {
        for axis in 0..2 {
            min[axis] = min[axis].min(v.position[axis]);
            max[axis] = max[axis].max(v.position[axis]);
        }
    }
    let (w, h) = (max[0] - min[0], max[1] - min[1]);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (cx, cy) = ((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5);
    let (sx, sy) = ((w + 2.0 * margin) / w, (h + 2.0 * margin) / h);
    for v in vertices {
        let x = cx + (v.position[0] - cx) * sx;
        let y = cy + (v.position[1] - cy) * sy;
        v.position[2] +=
            depth_gradient[0] * (x - v.position[0]) + depth_gradient[1] * (y - v.position[1]);
        v.position[0] = x;
        v.position[1] = y;
    }
}

/// Moves the four edges of the composed body independently, remapping every
/// vertex into the jittered box. Equal offsets on opposite edges translate;
/// unequal ones stretch.
fn apply_edge_jitter(
    batches: &mut [SpriteBatch],
    jitter: [f32; 4],
    depth_gradient: [f32; 2],
) -> Option<()> {
    let (min, max) = batches_bbox(batches)?;
    let (w, h) = (max[0] - min[0], max[1] - min[1]);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let [top, bottom, left, right] = jitter;
    let (sx, sy) = ((w + right - left) / w, (h + bottom - top) / h);
    for batch in batches {
        for v in &mut batch.vertices {
            let x = min[0] + left + (v.position[0] - min[0]) * sx;
            let y = min[1] + top + (v.position[1] - min[1]) * sy;
            v.position[2] +=
                depth_gradient[0] * (x - v.position[0]) + depth_gradient[1] * (y - v.position[1]);
            v.position[0] = x;
            v.position[1] = y;
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
pub fn compose_actor_batches<'a>(
    sprite: &'a EntitySprite,
    animation: &nostalre_formats::act::SpriteAnimationState,
    camera_dir: u8,
    head_dir: u8,
    screen_anchor: [f32; 2],
    depth: f32,
    base_scale: f32,
    depth_gradient: [f32; 2],
    channels: &BodyChannels,
) -> Vec<SpriteBatch<'a>> {
    let dir = if channels.yaw != 0.0 {
        let steps = (channels.yaw / (std::f32::consts::TAU / 8.0)).round() as i32;
        (((camera_dir as i32 + steps) % 8 + 8) % 8) as u8
    } else {
        camera_dir
    };
    let anchor = [screen_anchor[0], screen_anchor[1] - channels.lift_px];
    let scale = base_scale * channels.scale;

    let mut live = sprite.build_layers(
        animation,
        Some(dir),
        head_dir,
        anchor,
        depth,
        scale,
        depth_gradient,
        channels.weapon_light,
        false,
    );
    let (body_center, body_w, body_h) = batches_bbox(&live)
        .map(|(min, max)| {
            (
                [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5],
                (max[0] - min[0]).max(1.0),
                (max[1] - min[1]).max(1.0),
            )
        })
        .unwrap_or((anchor, 1.0, 1.0));

    let build_copy = |copy: &nostalre_effects::BodyCopy| {
        let mut batches = sprite.build_layers(
            animation,
            Some(dir),
            head_dir,
            anchor,
            depth,
            scale,
            depth_gradient,
            nostalre_effects::WeaponLight::None,
            copy.body_layers_only,
        );
        let margin_px = copy
            .glow
            .map_or(copy.margin_px, |g| g.margin_px(sprite.body_layer_count()));
        if margin_px != 0.0 {
            grow_clips(
                &mut batches,
                margin_px * channels.copy_margin_scale,
                depth_gradient,
            );
        } else {
            let scale_xy = if (copy.scale[0] - copy.scale[1]).abs() < 1e-6 && copy.scale[1] != 1.0 {
                let margin = (copy.scale[1] - 1.0) * body_h * 0.5;
                [(body_w + 2.0 * margin) / body_w, copy.scale[1]]
            } else {
                copy.scale
            };
            transform_batch_vertices_with_depth(
                &mut batches,
                body_center,
                0.0,
                scale_xy,
                depth_gradient,
            );
        }
        if copy.offset_px != [0.0, 0.0] {
            let offset_dz =
                depth_gradient[0] * copy.offset_px[0] + depth_gradient[1] * copy.offset_px[1];
            for b in &mut batches {
                for v in &mut b.vertices {
                    v.position[0] += copy.offset_px[0];
                    v.position[1] += copy.offset_px[1];
                    v.position[2] += offset_dz;
                }
            }
        }
        if copy.glow.is_some() {
            apply_tint_alpha(&mut batches, channels.tint, channels.alpha);
        }
        apply_tint_alpha(&mut batches, Some(copy.tint), copy.alpha);
        for b in &mut batches {
            b.additive = copy.additive;
        }
        batches
    };

    let mut out = Vec::new();
    for copy in channels.copies.iter().filter(|c| c.behind) {
        out.append(&mut build_copy(copy));
    }

    if channels.squeeze != 1.0 {
        transform_batch_vertices_with_depth(
            &mut live,
            anchor,
            0.0,
            [1.0, channels.squeeze],
            depth_gradient,
        );
    }
    if channels.angle != 0.0 {
        transform_batch_vertices_with_depth(
            &mut live,
            body_center,
            channels.angle,
            [1.0, 1.0],
            depth_gradient,
        );
    }
    if channels.edge_jitter != [0.0; 4] {
        apply_edge_jitter(&mut live, channels.edge_jitter, depth_gradient);
    }
    apply_tint_alpha(&mut live, channels.tint, channels.alpha);
    if channels.additive {
        for b in &mut live {
            b.additive = true;
        }
    }
    out.append(&mut live);

    for copy in channels.copies.iter().filter(|c| !c.behind) {
        out.append(&mut build_copy(copy));
    }

    if channels.light != [1.0; 3] {
        for batch in &mut out {
            for v in &mut batch.vertices {
                v.color[0] *= channels.light[0];
                v.color[1] *= channels.light[1];
                v.color[2] *= channels.light[2];
            }
        }
    }
    out
}

fn batches_bbox(batches: &[SpriteBatch]) -> Option<([f32; 2], [f32; 2])> {
    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    let mut any = false;
    for batch in batches {
        for v in &batch.vertices {
            any = true;
            min_x = min_x.min(v.position[0]);
            min_y = min_y.min(v.position[1]);
            max_x = max_x.max(v.position[0]);
            max_y = max_y.max(v.position[1]);
        }
    }
    any.then_some(([min_x, min_y], [max_x, max_y]))
}

/// Where a sprite pass draws: the frame's encoder, its colour target, the depth it tests against, and whether it clears
struct PassTarget<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    depth_view: Option<&'a wgpu::TextureView>,
    clear_color: Option<wgpu::Color>,
}

/// One sprite pass: `batches` packed into `meshes`, uploaded and drawn under the pipeline each batch asks for
fn draw_batches(
    pipelines: &SpritePipelines,
    uniform_bind_group: &wgpu::BindGroup,
    meshes: &mut MeshBatcher<SpriteVertex>,
    target: PassTarget<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batches: &[SpriteBatch],
) {
    pack(meshes, batches);
    // nothing to draw still clears when asked to
    if meshes.is_empty() && target.clear_color.is_none() {
        return;
    }
    meshes.upload(device, queue);

    {
        let mut pass = target
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sprite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match target.clear_color {
                            Some(color) => wgpu::LoadOp::Clear(color),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: target.depth_view.map(|dv| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: dv,
                        depth_ops: Some(wgpu::Operations {
                            load: match target.clear_color {
                                Some(_) => wgpu::LoadOp::Clear(1.0),
                                None => wgpu::LoadOp::Load,
                            },
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                ..Default::default()
            });

        let has_depth = target.depth_view.is_some();
        pass.set_bind_group(0, uniform_bind_group, &[]);
        meshes.bind(&mut pass);

        let pipeline_for = |(additive, no_depth, weapon_aura): (bool, bool, bool)| match (
            weapon_aura,
            additive,
            has_depth,
            no_depth,
        ) {
            (true, _, true, _) => &pipelines.weapon_aura,
            (true, _, false, _) => &pipelines.weapon_aura_no_depth,
            (false, false, true, false) => &pipelines.normal,
            (false, false, true, true) => &pipelines.overlay,
            (false, false, false, _) => &pipelines.no_depth,
            (false, true, true, false) => &pipelines.additive,
            (false, true, true, true) => &pipelines.additive_overlay,
            (false, true, false, _) => &pipelines.additive_no_depth,
        };
        let mut current = (false, false, false);
        pass.set_pipeline(pipeline_for(current));

        for run in meshes.runs() {
            let batch = &batches[run.first];
            let wanted = (batch.additive, batch.no_depth, batch.weapon_aura);
            if wanted != current {
                current = wanted;
                pass.set_pipeline(pipeline_for(current));
            }
            pass.set_bind_group(1, batch.texture, &[]);
            pass.draw_indexed(run.indices.clone(), 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_atlas_preserves_pixels_edges_pages_and_mirrored_clips() {
        let images = (0..700).map(|i| RgbaImageData {
            width: 40, height: 55, data: vec![(i % 251) as u8; 40 * 55 * 4],
        }).collect::<Vec<_>>();
        let pages = actor_atlas_plan(&images, 1024);
        assert!(pages.len() > 1 && pages.len() < 10);
        let mut seen = vec![false; images.len()];
        for page in &pages {
            let (pixels, width, height) = actor_atlas_pixels(&images, page);
            assert!(width <= 1024 && height <= 1024);
            let mut occupied = vec![false; (width * height) as usize];
            for &(i,x,y) in page {
                assert!(!seen[i]); seen[i] = true;
                for py in 0..images[i].height + 2 {
                    for px in 0..images[i].width + 2 {
                        let offset = ((y+py)*width + x+px) as usize;
                        assert!(!occupied[offset]); occupied[offset] = true;
                        assert_eq!(&pixels[offset*4..offset*4+4], &[(i % 251) as u8; 4]);
                    }
                }
            }
        }
        assert!(seen.into_iter().all(|v| v));
        let (device, queue) = crate::test_support::device();
        let cache = crate::texture::TextureCache::new(&device);
        let textures = upload_actor_atlases(&images[..2], 1, &device, &queue, &cache.bind_group_layout);
        assert_eq!(textures.bind_groups[0], textures.bind_groups[1], "two frames share one allocation");
        assert_ne!(textures.uv_rects[0], textures.uv_rects[1]);
        let mut clip = SpriteFrame { x:0, y:0, sprite_index:0, mirror:0, color:[255;4],
            zoom_x:1.0, zoom_y:1.0, angle:0, sprite_type:1, width:None, height:None };
        let (normal,index) = build_clip_quad(&clip,&textures,[10.0,20.0],0.0,[0.0;2]).unwrap();
        assert_eq!(index,1,"RGBA indexing retains the indexed-frame offset");
        assert_eq!(normal[0].tex_coord, [textures.uv_rects[1][0],textures.uv_rects[1][1]]);
        clip.mirror=1;
        let (mirrored,_) = build_clip_quad(&clip,&textures,[10.0,20.0],0.0,[0.0;2]).unwrap();
        assert_eq!(mirrored[0].tex_coord,normal[1].tex_coord);
        assert_eq!(mirrored[0].position,normal[0].position);
    }

    #[test]
    fn population_bodies_share_gpu_frames() {
        let (device, queue) = crate::test_support::device();
        let cache = crate::texture::TextureCache::new(&device);
        let body = || SpriteData {
            texture_key: Some("test/body.spr".into()),
            images: vec![RgbaImageData { width: 2, height: 2, data: vec![255; 16] }],
            indexed_count: 1,
            act: ActFile { version: (2, 5), actions: vec![], events: vec![], delays: vec![] },
        };
        let first = build_entity_sprite_cached(&device, &queue, &cache, body(), None, None, None, None, None, None, None, None);
        let second = build_entity_sprite_cached(&device, &queue, &cache, body(), None, None, None, None, None, None, None, None);
        assert_eq!(first.body_textures.bind_groups[0], second.body_textures.bind_groups[0], "identical bot bodies must reuse the uploaded GPU frames");
        let retained = Arc::downgrade(&first.body_textures.bind_groups);
        drop(first);
        assert!(retained.upgrade().is_some(), "the second bot still needs the shared frames");
        drop(second);
        assert!(retained.upgrade().is_none(), "the cache must not pin departed bots' GPU memory");
        let mut dyed = body();
        dyed.texture_key = Some("test/body.spr|cloth=2".into());
        dyed.images[0].data[0] = 0;
        let plain = build_entity_sprite_cached(&device, &queue, &cache, body(), None, None, None, None, None, None, None, None);
        let dyed = build_entity_sprite_cached(&device, &queue, &cache, dyed, None, None, None, None, None, None, None, None);
        assert_ne!(plain.body_textures.bind_groups[0], dyed.body_textures.bind_groups[0], "different palettes must keep distinct pixels");
        let other_renderer = crate::texture::TextureCache::new(&device);
        let isolated = build_entity_sprite_cached(&device, &queue, &other_renderer, body(), None, None, None, None, None, None, None, None);
        assert_ne!(plain.body_textures.bind_groups[0], isolated.body_textures.bind_groups[0], "archive stacks and renderer lifetimes must stay isolated");
    }

    fn batch<'a>(
        vertices: SpriteVertices,
        texture: &'a wgpu::BindGroup,
        additive: bool,
    ) -> SpriteBatch<'a> {
        let indices = if vertices.len() == 4 {
            QUAD_INDICES
        } else {
            &[0, 1, 2]
        };
        SpriteBatch {
            vertices,
            indices,
            texture,
            additive,
            no_depth: false,
            weapon_aura: false,
        }
    }

    fn vertex() -> SpriteVertex {
        SpriteVertex {
            position: [0.0; 3],
            tex_coord: [0.0; 2],
            color: [1.0; 4],
        }
    }

    #[test]
    fn batches_sharing_texture_and_pipeline_pack_into_one_draw() {
        let (device, _) = crate::test_support::device();
        let t = crate::test_support::bind_groups();
        let v = vertex();
        let batches = [
            batch([v; 4].into(), &t[0], false),
            batch(vec![v; 3].into(), &t[0], false),
            batch([v; 4].into(), &t[0], true),
            batch([v; 4].into(), &t[1], true),
        ];
        let mut meshes = MeshBatcher::new(&device, "test", 4, 6);
        // a pass before leaves its lists behind; the next pack starts them over
        pack(&mut meshes, &batches[2..]);
        pack(&mut meshes, &batches);
        let runs: Vec<_> = meshes
            .runs()
            .iter()
            .map(|r| (r.first, r.indices.clone()))
            .collect();
        assert_eq!(runs, [(0, 0..9), (2, 9..15), (3, 15..21)]);
        assert_eq!(meshes.vertices().len(), 15);
        assert_eq!(
            &meshes.indices()[6..9],
            &[4, 5, 6],
            "a mesh's indices follow the quad's vertices"
        );
    }

    /// Every pass on wgpu's stub device, which validates each draw's index range against the bound buffers.
    #[test]
    fn passes_record_valid_draws_on_the_stub_device() {
        let (device, queue) = crate::test_support::device();
        let textures = crate::texture::TextureCache::new(&device);
        let white = crate::texture::create_texture_bind_group(
            &device,
            &queue,
            &image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])),
            &textures.bind_group_layout,
            "white",
        );
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = SpriteRenderer::new(
            &device,
            format,
            &textures.bind_group_layout,
            64.0,
            64.0,
            include_str!("shaders/sprite.wgsl"),
            false,
        );
        let target = |format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: 64,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = target(format, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth = target(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let v = vertex();
        // more than the buffers start with, so the upload grows them
        let batches: Vec<_> = (0..INITIAL_VERTEX_CAPACITY)
            .map(|i| batch([v; 4].into(), &white, i % 3 == 0))
            .chain([batch(vec![v; 3].into(), &white, false)])
            .collect();
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.render(
            &mut encoder,
            &color,
            Some(&depth),
            &device,
            &queue,
            None,
            &batches,
        );
        renderer.render_silhouette(&mut encoder, &color, &depth, &device, &queue, &batches);
        renderer.render(
            &mut encoder,
            &color,
            None,
            &device,
            &queue,
            Some(wgpu::Color::BLACK),
            &[],
        );
        queue.submit([encoder.finish()]);
        assert!(
            renderer.meshes.is_empty(),
            "the last pass, a clear only, had nothing to draw"
        );
        assert_eq!(
            renderer.silhouette_meshes.vertices().len(),
            INITIAL_VERTEX_CAPACITY * 4 + 3
        );
    }

    #[test]
    fn a_weapon_auras_quad_grows_around_it_keeping_its_texels_in_place() {
        use nostalre_effects::{WeaponAura, WeaponAuraStyle};
        let v = |x: f32, y: f32, u: f32, w: f32| SpriteVertex {
            position: [x, y, 0.5],
            tex_coord: [u, w],
            color: [1.0; 4],
        };
        // a 40 px clip, mirrored (u runs right to left)
        let clip = [
            v(0.0, 60.0, 1.0, 0.0),
            v(40.0, 60.0, 0.0, 0.0),
            v(40.0, 100.0, 0.0, 1.0),
            v(0.0, 100.0, 1.0, 1.0),
        ];
        let aura = |frame| WeaponAura {
            style: WeaponAuraStyle::Pulse,
            inner: [255, 255, 255],
            outer: [8, 16, 248],
            frame,
        };
        let quad = weapon_aura_quad(&clip, &aura(0), 1.0);
        let m = WEAPON_AURA_MARGIN_PX;
        assert_eq!(quad[0].position[..2], [-m, 60.0 - m]);
        assert_eq!(quad[2].position[..2], [40.0 + m, 100.0 + m]);
        // a texel keeps its place: the grown corner maps a margin's worth past the texture's edge
        let past = m / 40.0;
        assert!((quad[0].tex_coord[0] - (1.0 + past)).abs() < 1e-5);
        assert!((quad[2].tex_coord[1] - (1.0 + past)).abs() < 1e-5);
        assert_eq!(quad[0].position[2], 0.5, "the weapon's depth");
        // the colours, 5 bits a channel, and the style
        assert_eq!(quad[0].color[1], 32767.0);
        assert_eq!(quad[0].color[2], ((1 << 10) | (2 << 5) | 31) as f32);
        assert_eq!(quad[0].color[3], 1.0);
        assert_ne!(
            weapon_aura_quad(&clip, &aura(60), 1.0)[0].color[0],
            quad[0].color[0],
            "the clock moves"
        );
    }

    use nostalre_formats::act::SpriteFrame;

    fn dummy_textures() -> SpriteTextures {
        SpriteTextures {
            bind_groups: Arc::new(Vec::new()),
            uv_rects: Arc::new(Vec::new()),
            sizes: vec![(24, 24), (32, 32), (48, 48)],
            indexed_count: 2,
        }
    }

    fn idle_motion(attach: (i32, i32)) -> Motion {
        Motion {
            range1: [0; 4],
            range2: [0; 4],
            clips: Vec::new(),
            event_id: -1,
            attach_points: vec![nostalre_formats::act::AnchorPoint {
                ignored: 0,
                x: attach.0,
                y: attach.1,
                attribute: 0,
            }],
        }
    }

    #[test]
    fn layers_draw_back_to_front_with_shield_and_robe_by_direction() {
        let quad = |tex: usize| ([vertex(); 4], tex);
        let clips = |robe_behind: bool| CompositeClips {
            body: vec![quad(0)],
            head: vec![quad(1)],
            headgear_bottom: vec![quad(2)],
            headgear_mid: vec![quad(3)],
            headgear_top: vec![quad(4)],
            weapon: vec![quad(5)],
            weapon_trail: vec![quad(6)],
            shield: vec![quad(7)],
            robe: vec![quad(8), quad(9)],
            robe_behind,
            body_motion_idx: 0,
        };
        let order = |clips: CompositeClips, action_idx: usize| -> Vec<(SpritePart, usize)> {
            clips
                .into_draw_order(action_idx, None)
                .into_iter()
                .map(|(part, (_, tex))| (part, tex))
                .collect()
        };
        use SpritePart::*;
        // facing the camera: the shield in front, the robe over everything
        assert_eq!(
            order(clips(false), 0),
            [
                (Body, 0),
                (Head, 1),
                (HeadgearBottom, 2),
                (HeadgearTop, 4),
                (HeadgearMid, 3),
                (Weapon, 5),
                (WeaponTrail, 6),
                (Shield, 7),
                (Robe, 8),
                (Robe, 9),
            ]
        );
        // turned away: the shield behind, and a robe frame listed behind goes under the body
        let away = order(clips(true), 8 + 3);
        assert_eq!(away[..4], [(Shield, 7), (Robe, 8), (Robe, 9), (Body, 0)]);
        assert_eq!(away.last(), Some(&(WeaponTrail, 6)));
    }

    #[test]
    fn a_robe_goes_behind_the_body_only_on_the_listed_frames() {
        let robe = RobeLayer {
            textures: dummy_textures(),
            act: ActFile {
                version: (2, 5),
                actions: Vec::new(),
                events: Vec::new(),
                delays: Vec::new(),
            },
            behind: HashMap::from([(0, vec![0, 2]), (12, vec![1])]),
        };
        assert!(robe.is_behind(0, 0));
        assert!(!robe.is_behind(0, 1));
        assert!(robe.is_behind(0, 2));
        assert!(robe.is_behind(12, 1));
        // an action without rows draws it over the body
        assert!(!robe.is_behind(4, 0));
    }

    #[test]
    fn taekwon_kick_and_cast_turn_the_head_out_of_the_action() {
        const MALE: u8 = 1;
        const FEMALE: u8 = 0;

        assert_eq!(taekwon_head_action(88, 3, MALE), Some(6));
        assert_eq!(taekwon_head_action(88, 4, MALE), Some(5));
        assert_eq!(taekwon_head_action(88, 5, MALE), None);
        assert_eq!(taekwon_head_action(88, 3, FEMALE), None);
        assert_eq!(taekwon_head_action(88, 4, FEMALE), Some(6));
        assert_eq!(taekwon_head_action(88, 5, FEMALE), Some(5));

        let turned: Vec<_> = (88..96)
            .map(|a| taekwon_head_action(a, 3, MALE).unwrap())
            .collect();
        assert_eq!(turned, [6, 6, 6, 6, 2, 2, 2, 2]);
        let turned: Vec<_> = (88..96)
            .map(|a| taekwon_head_action(a, 4, MALE).unwrap())
            .collect();
        assert_eq!(turned, [5, 5, 7, 7, 1, 1, 3, 3]);

        let cast: Vec<_> = (96..104)
            .map(|a| taekwon_head_action(a, 1, MALE).unwrap())
            .collect();
        assert_eq!(cast, [0, 0, 6, 6, 4, 4, 2, 2]);
        assert_eq!(taekwon_head_action(96, 0, MALE), None);

        // The swing's other group and every non-kick frame keep their own pose.
        assert_eq!(taekwon_head_action(80, 3, MALE), None);
        assert_eq!(taekwon_head_action(88, 0, MALE), None);
    }

    // Real dir=2 idle attach points from data.grf (초보자_남 / 1_남): each idle
    // action has 3 doridori-pose motions whose neck anchor differs by ~17px.
    #[test]
    fn doridori_keeps_head_attached_to_body() {
        let body = [
            idle_motion((-4, -57)),
            idle_motion((11, -74)),
            idle_motion((-3, -57)),
        ];
        let head = [
            idle_motion((-6, -57)),
            idle_motion((10, -73)),
            idle_motion((-5, -56)),
        ];

        // Idle: every part follows head_dir, not the (0) animation frame.
        assert_eq!(part_motion_index(true, 1, 0, 3), Some(1));
        // A moving action ignores head_dir and plays its own frame.
        assert_eq!(part_motion_index(false, 1, 2, 3), Some(2));
        // A part short of frames is skipped rather than wrapped.
        assert_eq!(part_motion_index(false, 1, 5, 3), None);
        // A headgear with a whole multiple of the head's frames runs at that
        // multiple; any other count follows the plain rule.
        let pose =
            |head, over, motion_idx, len| headgear_pose(88, head, over, false, 0, motion_idx, len);
        assert_eq!(pose(Some((2, 3)), None, 2, 6), Some((88, 4)));
        assert_eq!(pose(Some((2, 3)), None, 2, 5), Some((88, 2)));
        assert_eq!(pose(Some((2, 3)), None, 2, 3), Some((88, 2)));
        assert_eq!(pose(None, None, 7, 6), None);
        // A substituted head pose carries the headgear with it, but the
        // multiple wins over it.
        assert_eq!(pose(Some((3, 8)), Some(6), 3, 8), Some((6, 0)));
        assert_eq!(pose(Some((3, 8)), Some(6), 3, 16), Some((88, 6)));

        // With both parts on the head_dir pose the neck anchors line up.
        for head_dir in 0u8..3 {
            let bi = part_motion_index(true, head_dir, 0, 3).unwrap();
            let hi = part_motion_index(true, head_dir, 0, 3).unwrap();
            let (ox, oy) = attachment_offset(&body[bi], &head[hi]);
            assert!(
                ox.abs() <= 2 && oy.abs() <= 2,
                "head_dir {head_dir}: ({ox},{oy})"
            );
        }

        // The old bug: body stuck on frame 0 while the head turned -> big gap.
        let (ox, oy) = attachment_offset(&body[0], &head[1]);
        assert!(ox.abs() > 10 || oy.abs() > 10);
    }

    /// The halo copies of a body grow clip by clip, so a small head sprite and a
    /// tall body sprite each gain the same outline instead of the whole rig
    /// being stretched about one shared centre.
    #[test]
    fn body_copy_margin_outlines_each_clip_by_the_same_pixels() {
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32| {
            [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
                .map(|(x, y)| SpriteVertex {
                    position: [x, y, 0.0],
                    tex_coord: [0.0, 0.0],
                    color: [1.0; 4],
                })
                .to_vec()
        };
        let bounds = |v: &[SpriteVertex]| {
            let xs: Vec<f32> = v.iter().map(|v| v.position[0]).collect();
            let ys: Vec<f32> = v.iter().map(|v| v.position[1]).collect();
            [
                xs.iter().cloned().fold(f32::MAX, f32::min),
                ys.iter().cloned().fold(f32::MAX, f32::min),
                xs.iter().cloned().fold(f32::MIN, f32::max),
                ys.iter().cloned().fold(f32::MIN, f32::max),
            ]
        };

        let mut head = quad(90.0, 10.0, 110.0, 30.0);
        let mut body = quad(80.0, 30.0, 120.0, 130.0);
        for clip in [&mut head, &mut body] {
            grow_clip(clip, 5.0, [0.0, 0.0]);
        }

        assert_eq!(bounds(&head), [85.0, 5.0, 115.0, 35.0]);
        assert_eq!(bounds(&body), [75.0, 25.0, 125.0, 135.0]);
    }

    #[test]
    fn build_clip_quad_basic_indexed_sprite() {
        let clip = SpriteFrame {
            x: 0,
            y: 0,
            sprite_index: 0,
            mirror: 0,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let (verts, tex_idx) =
            build_clip_quad(&clip, &textures, [100.0, 100.0], 0.5, [0.0, 0.0]).unwrap();

        assert_eq!(tex_idx, 0);
        assert!((verts[0].position[0] - 88.0).abs() < 0.01);
        assert!((verts[0].position[1] - 88.0).abs() < 0.01);
        assert!((verts[2].position[0] - 112.0).abs() < 0.01);
        assert!((verts[2].position[1] - 112.0).abs() < 0.01);
        assert!((verts[0].position[2] - 0.5).abs() < 0.001);
    }

    /// Screen space is y-down, so a positive clip angle turns clockwise, and a
    /// mirrored clip is the same quad with its texture columns swapped — the
    /// corner that carries `u = 0` moves to the other side of the rotated quad.
    #[test]
    fn rotated_clip_turns_clockwise_and_mirrors_across_the_rotated_axis() {
        let make = |angle: i32, mirror: u32| SpriteFrame {
            x: 0,
            y: 0,
            sprite_index: 0,
            mirror,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let anchor = [100.0, 100.0];
        let corner = |verts: &[SpriteVertex], u: f32, v: f32| -> [f32; 2] {
            let c = verts
                .iter()
                .find(|c| c.tex_coord == [u, v])
                .expect("corner present");
            [c.position[0], c.position[1]]
        };

        let (flat, _) = build_clip_quad(&make(0, 0), &textures, anchor, 0.0, [0.0, 0.0]).unwrap();
        assert_eq!(corner(&flat, 0.0, 0.0), [88.0, 88.0]);

        // +90° puts the texture's top-left corner where its bottom-left was.
        let (spun, _) = build_clip_quad(&make(90, 0), &textures, anchor, 0.0, [0.0, 0.0]).unwrap();
        let tl = corner(&spun, 0.0, 0.0);
        assert!(
            (tl[0] - 112.0).abs() < 0.01 && (tl[1] - 88.0).abs() < 0.01,
            "{tl:?}"
        );

        // Mirroring swaps which rotated corner carries u = 0, and leaves the
        // quad's four positions untouched.
        let (mirrored, _) =
            build_clip_quad(&make(90, 1), &textures, anchor, 0.0, [0.0, 0.0]).unwrap();
        assert_eq!(corner(&mirrored, 0.0, 0.0), corner(&spun, 1.0, 0.0));
        assert_eq!(corner(&mirrored, 1.0, 0.0), corner(&spun, 0.0, 0.0));
        let mut spun_pos: Vec<[f32; 2]> = spun
            .iter()
            .map(|v| [v.position[0], v.position[1]])
            .collect();
        let mut mirror_pos: Vec<[f32; 2]> = mirrored
            .iter()
            .map(|v| [v.position[0], v.position[1]])
            .collect();
        for p in spun_pos.iter_mut().chain(mirror_pos.iter_mut()) {
            *p = [p[0].round(), p[1].round()];
        }
        spun_pos.sort_by(|a, b| a.partial_cmp(b).unwrap());
        mirror_pos.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(spun_pos, mirror_pos);
    }

    #[test]
    fn build_clip_quad_scaled_grows_size_and_offset_about_anchor() {
        let clip = SpriteFrame {
            x: 10,
            y: 0,
            sprite_index: 0,
            mirror: 0,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let (verts, _) =
            build_clip_quad_scaled(&clip, &textures, [100.0, 100.0], 0.0, [0.0, 0.0], 2.0).unwrap();

        let width = verts[2].position[0] - verts[0].position[0];
        let center_x = (verts[0].position[0] + verts[2].position[0]) / 2.0;
        assert!((width - 48.0).abs() < 0.01, "24px sprite doubles to 48px");
        assert!(
            (center_x - 120.0).abs() < 0.01,
            "clip offset scales about anchor"
        );
    }

    #[test]
    fn build_clip_quad_rgba_sprite_with_offset() {
        let clip = SpriteFrame {
            x: 10,
            y: -5,
            sprite_index: 0,
            mirror: 0,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 1,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let (_, tex_idx) =
            build_clip_quad(&clip, &textures, [200.0, 200.0], 0.0, [0.0, 0.0]).unwrap();
        assert_eq!(tex_idx, 2);
    }

    #[test]
    fn build_clip_quad_mirrored_flips_uvs() {
        let clip = SpriteFrame {
            x: 0,
            y: 0,
            sprite_index: 0,
            mirror: 1,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let (verts, _) =
            build_clip_quad(&clip, &textures, [100.0, 100.0], 0.0, [0.0, 0.0]).unwrap();
        assert!((verts[0].tex_coord[0] - 1.0).abs() < 0.01);
        assert!((verts[1].tex_coord[0] - 0.0).abs() < 0.01);
    }

    #[test]
    fn build_clip_quad_negative_index_returns_none() {
        let clip = SpriteFrame {
            x: 0,
            y: 0,
            sprite_index: -1,
            mirror: 0,
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        assert!(build_clip_quad(&clip, &textures, [100.0, 100.0], 0.0, [0.0, 0.0]).is_none());
    }

    #[test]
    fn build_clip_quad_with_zoom_scales_dimensions() {
        let clip = SpriteFrame {
            x: 0,
            y: 0,
            sprite_index: 0,
            mirror: 0,
            color: [255, 255, 255, 255],
            zoom_x: 2.0,
            zoom_y: 0.5,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        };
        let textures = dummy_textures();
        let (verts, _) =
            build_clip_quad(&clip, &textures, [100.0, 100.0], 0.0, [0.0, 0.0]).unwrap();
        let w = verts[1].position[0] - verts[0].position[0];
        let h = verts[3].position[1] - verts[0].position[1];
        assert!((w - 48.0).abs() < 0.01);
        assert!((h - 12.0).abs() < 0.01);
    }
}
