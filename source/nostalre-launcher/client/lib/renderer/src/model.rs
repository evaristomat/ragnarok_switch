use std::collections::HashMap;

use nostalre_formats::gnd::GndFile;
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::rsm::{RsmFile, RsmNode};
use nostalre_formats::rsw::{RswFile, RswObject};

use crate::device::DEPTH_FORMAT;
use crate::global_uniforms::GlobalUniforms;
use crate::matrix_texture::MatrixTexture;
use crate::rsm_anim;
use crate::texture::TextureCache;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    pub alpha: f32,
    pub lit_scale: f32,
    pub unlit: f32,
}

impl ModelVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ModelVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32,
            4 => Float32,
            5 => Float32,
        ],
    };
}

#[derive(Clone)]
struct DrawBatch {
    bounds: Option<([f32;3],[f32;3])>,
    visible: bool,
    texture_name: String,
    start_index: u32,
    index_count: u32,
}

pub struct ModelRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    batches: Vec<DrawBatch>,
}

struct BoundingBox {
    min: glam::Vec3,
    max: glam::Vec3,
    center: glam::Vec3,
}

impl BoundingBox {
    fn new() -> Self {
        Self {
            min: glam::Vec3::splat(f32::INFINITY),
            max: glam::Vec3::splat(f32::NEG_INFINITY),
            center: glam::Vec3::ZERO,
        }
    }

    fn extend(&mut self, point: glam::Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    fn finalize(&mut self) {
        let range = (self.max - self.min) * 0.5;
        self.center = self.min + range;
    }
}

/// A map's props, split by whether they need a per-frame animation tick.
pub struct RswModelRenderers {
    pub static_models: Option<ModelRenderer>,
    pub animated_models: Option<AnimatedModelRenderer>,
}

impl ModelRenderer {
    pub fn update_visibility(&mut self, camera: &crate::camera::Camera) {
        if self.batches.iter().all(|b| b.bounds.is_none()) { return; }
        let vp = camera.view_projection();
        let originals = self.batches.iter().filter(|b| b.bounds.is_none()).count();
        let visible = self.batches.iter().filter(|b| b.bounds.is_some_and(|(lo,hi)| bounds_visible(vp,lo,hi))).count();
        // Splitting can increase draw calls in an open view. Retain the original
        // texture batches and use sectors only when fewer draws are needed.
        let use_sectors = visible < originals;
        for batch in &mut self.batches {
            batch.visible = match batch.bounds {
                Some((lo,hi)) => use_sectors && bounds_visible(vp,lo,hi),
                None => !use_sectors,
            };
        }
    }
    pub fn from_rsw(
        rsw: &RswFile,
        gnd: &GndFile,
        grf: &GrfArchive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniforms: &GlobalUniforms,
        texture_cache: &mut TextureCache,
        surface_format: wgpu::TextureFormat,
    ) -> RswModelRenderers {
        let rsw_models: Vec<_> = rsw
            .objects
            .iter()
            .filter_map(|obj| {
                if let RswObject::Model(m) = obj {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();

        if rsw_models.is_empty() {
            return RswModelRenderers {
                static_models: None,
                animated_models: None,
            };
        }

        let start = std::time::Instant::now();

        let (rsm_cache, static_instances, animated_instances) =
            load_rsw_instances(&rsw_models, gnd, grf);

        let (vertices, indices, batches) = build_mesh(
            &rsm_cache,
            &static_instances,
            grf,
            texture_cache,
            device,
            queue,
        );

        let (indices, batches) = sector_batches(&vertices, &indices, batches);

        let animated_models = AnimatedModelRenderer::build(
            &rsm_cache,
            &animated_instances,
            grf,
            device,
            queue,
            global_uniforms,
            texture_cache,
            surface_format,
        );

        tracing::info!(
            "Loaded {} model instances ({} verts, {} batches) in {:.1}s; {} animated instances",
            rsw_models.len(),
            vertices.len(),
            batches.len(),
            start.elapsed().as_secs_f32(),
            animated_models
                .as_ref()
                .map(|a| a.instances.len())
                .unwrap_or(0),
        );

        if vertices.is_empty() {
            return RswModelRenderers {
                static_models: None,
                animated_models,
            };
        }

        let vertex_buffer = create_buffer(
            device,
            "model_vertices",
            &vertices,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer =
            create_buffer(device, "model_indices", &indices, wgpu::BufferUsages::INDEX);

        let pipeline = create_pipeline(
            device,
            surface_format,
            &global_uniforms.bind_group_layout,
            &texture_cache.bind_group_layout,
        );

        RswModelRenderers {
            static_models: Some(Self {
                pipeline,
                vertex_buffer,
                index_buffer,
                batches,
            }),
            animated_models,
        }
    }

    /// Build a renderer for a single standalone RSM (no RSW/GND world context),
    /// centred at the origin via the same node compilation the map path uses.
    /// Returns the renderer plus the centred model's bounding-box `center` and
    /// `size` (world units) so a caller can frame a camera around it.
    pub fn from_rsm(
        rsm: &RsmFile,
        grf: &GrfArchive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniforms: &GlobalUniforms,
        texture_cache: &mut TextureCache,
        surface_format: wgpu::TextureFormat,
    ) -> Option<(Self, [f32; 3], [f32; 3])> {
        if rsm.nodes.is_empty() {
            return None;
        }

        preload_rsm_textures(rsm, grf, texture_cache, device, queue);

        let is_only = rsm.nodes.len() == 1;
        let (bbox, node_matrices) = calc_bounding_box(rsm);
        let alpha = rsm.alpha.map(|a| a as f32 / 255.0).unwrap_or(1.0);
        let instance_matrix = glam::Mat4::IDENTITY;

        let mut texture_quads: HashMap<String, (Vec<ModelVertex>, Vec<u32>)> = HashMap::new();
        for (node_idx, node) in rsm.nodes.iter().enumerate() {
            compile_node(
                node,
                rsm,
                &node_matrices[node_idx],
                &bbox,
                &instance_matrix,
                is_only,
                alpha,
                &mut texture_quads,
            );
        }

        let (all_vertices, all_indices, batches) = flatten_texture_batches(texture_quads);

        if all_vertices.is_empty() {
            return None;
        }

        let vertex_buffer = create_buffer(
            device,
            "model_vertices",
            &all_vertices,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer = create_buffer(
            device,
            "model_indices",
            &all_indices,
            wgpu::BufferUsages::INDEX,
        );
        let pipeline = create_pipeline(
            device,
            surface_format,
            &global_uniforms.bind_group_layout,
            &texture_cache.bind_group_layout,
        );

        // where `compile_node` puts the box: RSM1 centred on x/z with its top at y=0, RSM2 turned y-down
        let size = bbox.max - bbox.min;
        let root = model_root_matrix(rsm, &bbox);
        let center =
            ((root.transform_point3(bbox.min) + root.transform_point3(bbox.max)) * 0.5).to_array();

        Some((
            Self {
                pipeline,
                vertex_buffer,
                index_buffer,
                batches,
            },
            center,
            [size.x, size.y, size.z],
        ))
    }

    /// Build a renderer for a single RSM placed at a world position, scaled by
    /// the map's `scale_factor` (`zoom / 10`) so it matches props baked from the
    /// RSW. Used for skill-unit models (traps) placed at runtime.
    pub fn from_rsm_at(
        rsm: &RsmFile,
        grf: &GrfArchive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniforms: &GlobalUniforms,
        texture_cache: &mut TextureCache,
        surface_format: wgpu::TextureFormat,
        world_pos: [f32; 3],
        scale_factor: f32,
    ) -> Option<Self> {
        if rsm.nodes.is_empty() {
            return None;
        }

        preload_rsm_textures(rsm, grf, texture_cache, device, queue);

        let is_only = rsm.nodes.len() == 1;
        let (bbox, node_matrices) = calc_bounding_box(rsm);
        let alpha = rsm.alpha.map(|a| a as f32 / 255.0).unwrap_or(1.0);
        let instance_matrix = glam::Mat4::from_translation(glam::Vec3::from_array(world_pos))
            * glam::Mat4::from_scale(glam::Vec3::splat(scale_factor));

        let mut texture_quads: HashMap<String, (Vec<ModelVertex>, Vec<u32>)> = HashMap::new();
        for (node_idx, node) in rsm.nodes.iter().enumerate() {
            compile_node(
                node,
                rsm,
                &node_matrices[node_idx],
                &bbox,
                &instance_matrix,
                is_only,
                alpha,
                &mut texture_quads,
            );
        }

        let (all_vertices, all_indices, batches) = flatten_texture_batches(texture_quads);

        if all_vertices.is_empty() {
            return None;
        }

        let vertex_buffer = create_buffer(
            device,
            "model_vertices",
            &all_vertices,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer = create_buffer(
            device,
            "model_indices",
            &all_indices,
            wgpu::BufferUsages::INDEX,
        );
        let pipeline = create_pipeline(
            device,
            surface_format,
            &global_uniforms.bind_group_layout,
            &texture_cache.bind_group_layout,
        );

        Some(Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            batches,
        })
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        global_uniforms: &'a GlobalUniforms,
        texture_cache: &'a TextureCache,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &global_uniforms.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for batch in &self.batches {
            if !batch.visible { continue; }
            if let Some(tex_bg) = texture_cache.get(&batch.texture_name) {
                pass.set_bind_group(1, tex_bg, &[]);
                pass.draw_indexed(
                    batch.start_index..batch.start_index + batch.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimatedModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    pub alpha: f32,
    /// Index into the per-frame node matrix array.
    pub node_slot: u32,
    pub lit_scale: f32,
    pub unlit: f32,
}

impl AnimatedModelVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<AnimatedModelVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32,
            4 => Uint32,
            5 => Float32,
            6 => Float32,
        ],
    };
}

/// An animated RSM kept resident so its node tracks can be re-evaluated.
struct AnimatedModel {
    rsm: RsmFile,
    /// `node_post_matrix` per node — constant across frames.
    post: Vec<glam::Mat4>,
}

struct AnimatedInstance {
    model: usize,
    /// `instance_matrix * bbox_shift` — constant across frames.
    pre: glam::Mat4,
    anim_type: i32,
    anim_speed: f32,
    cur_motion: f32,
    /// First node matrix slot owned by this instance.
    slot_base: usize,
    pose_elapsed: f32,
}

/// Map props with a moving node track. Vertices stay in node-local space and one
/// matrix per (instance, node) is re-uploaded each frame, so per-frame cost
/// scales with node count rather than vertex count.
pub struct AnimatedModelRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    batches: Vec<DrawBatch>,
    node_texture: MatrixTexture,
    node_bind_group: wgpu::BindGroup,
    models: Vec<AnimatedModel>,
    instances: Vec<AnimatedInstance>,
    matrices: Vec<[f32; 16]>,
    accum_scratch: Vec<glam::Mat4>,
    visited_scratch: Vec<bool>,
}

impl AnimatedModelRenderer {
    #[allow(clippy::too_many_arguments)]
    fn build(
        rsm_cache: &HashMap<String, RsmFile>,
        instances_by_model: &HashMap<String, Vec<ModelInstance>>,
        grf: &GrfArchive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniforms: &GlobalUniforms,
        texture_cache: &mut TextureCache,
        surface_format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let mut models: Vec<AnimatedModel> = Vec::new();
        let mut instances: Vec<AnimatedInstance> = Vec::new();
        let mut texture_quads: HashMap<String, (Vec<AnimatedModelVertex>, Vec<u32>)> =
            HashMap::new();
        let mut slot_count = 0usize;

        for (rsm_path, model_instances) in instances_by_model {
            let Some(rsm) = rsm_cache.get(rsm_path) else {
                continue;
            };
            if rsm.nodes.is_empty() {
                continue;
            }

            preload_rsm_textures(rsm, grf, texture_cache, device, queue);

            let is_only = rsm.nodes.len() == 1;
            let (bbox, node_matrices) = calc_bounding_box(rsm);
            let shift = model_root_matrix(rsm, &bbox);
            let post: Vec<glam::Mat4> = rsm
                .nodes
                .iter()
                .map(|node| node_post_matrix(rsm, node, is_only))
                .collect();

            let model_idx = models.len();

            for inst in model_instances {
                let pre = inst.instance_matrix * shift;
                let slot_base = slot_count;
                slot_count += rsm.nodes.len();

                for (node_idx, node) in rsm.nodes.iter().enumerate() {
                    // Frame 0 pose decides the winding sign; rotation over the
                    // animation cannot flip it.
                    let frame0 = pre * node_matrices[node_idx] * post[node_idx];
                    compile_animated_node(
                        node,
                        rsm,
                        (slot_base + node_idx) as u32,
                        frame0.determinant(),
                        inst.alpha,
                        &mut texture_quads,
                    );
                }

                instances.push(AnimatedInstance {
                    model: model_idx,
                    pre,
                    anim_type: inst.anim_type,
                    anim_speed: inst.anim_speed,
                    cur_motion: 0.0,
                    slot_base,
                    pose_elapsed: 0.0,
                });
            }

            models.push(AnimatedModel {
                rsm: clone_anim_skeleton(rsm),
                post,
            });
        }

        let (vertices, indices, batches) = flatten_texture_batches(texture_quads);
        if vertices.is_empty() || slot_count == 0 {
            return None;
        }

        let vertex_buffer = create_buffer(
            device,
            "animated_model_vertices",
            &vertices,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer = create_buffer(
            device,
            "animated_model_indices",
            &indices,
            wgpu::BufferUsages::INDEX,
        );

        // the node matrices as a texture (see matrix_texture), its padding identity: uploaded as is every frame
        let node_texture = MatrixTexture::new(device, "animated_model_nodes", slot_count);
        let matrices = vec![glam::Mat4::IDENTITY.to_cols_array(); node_texture.capacity()];
        let node_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("animated_model_nodes"),
            entries: &[MatrixTexture::layout_entry(0)],
        });
        let node_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("animated_model_nodes"),
            layout: &node_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(node_texture.view()),
            }],
        });

        let pipeline = create_animated_pipeline(
            device,
            surface_format,
            &global_uniforms.bind_group_layout,
            &texture_cache.bind_group_layout,
            &node_layout,
        );

        let max_nodes = models.iter().map(|m| m.rsm.nodes.len()).max().unwrap_or(0);

        let renderer = Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            batches,
            node_texture,
            node_bind_group,
            models,
            instances,
            matrices,
            accum_scratch: vec![glam::Mat4::IDENTITY; max_nodes],
            visited_scratch: vec![false; max_nodes],
        };
        renderer.write_matrices(queue);
        Some(renderer)
    }

    /// Poses every instance at its current motion, then advances it by `dt`
    /// seconds.
    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32, camera: &crate::camera::Camera) {
        nostalre_profiling::profile_function!();
        let mut dirty = false;
        for inst in &mut self.instances {
            let model = &self.models[inst.model];
            inst.cur_motion = rsm_anim::advance_motion(inst.cur_motion, inst.anim_type, inst.anim_speed, model.rsm.anim_length, dt);
            inst.pose_elapsed += dt;
            let position = inst.pre.transform_point3(glam::Vec3::ZERO);
            let interval = if camera.is_world_pos_visible(position.x,position.y,position.z,1.0) { 1.0/30.0 } else { 0.1 };
            if inst.pose_elapsed < interval { continue; }
            inst.pose_elapsed %= interval;
            dirty = true;
            let node_count = model.rsm.nodes.len();

            node_accum_matrices_into(
                &model.rsm,
                inst.cur_motion,
                &mut self.accum_scratch[..node_count],
                &mut self.visited_scratch[..node_count],
            );

            for node_idx in 0..node_count {
                self.matrices[inst.slot_base + node_idx] =
                    (inst.pre * self.accum_scratch[node_idx] * model.post[node_idx])
                        .to_cols_array();
            }


        }
        if dirty { self.write_matrices(queue); }
    }

    fn write_matrices(&self, queue: &wgpu::Queue) {
        self.node_texture.write(queue, &self.matrices);
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        global_uniforms: &'a GlobalUniforms,
        texture_cache: &'a TextureCache,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &global_uniforms.bind_group, &[]);
        pass.set_bind_group(2, &self.node_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for batch in &self.batches {
            if let Some(tex_bg) = texture_cache.get(&batch.texture_name) {
                pass.set_bind_group(1, tex_bg, &[]);
                pass.draw_indexed(
                    batch.start_index..batch.start_index + batch.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

/// Keeps only what per-frame track evaluation needs; the mesh already lives in
/// the vertex buffer.
fn clone_anim_skeleton(rsm: &RsmFile) -> RsmFile {
    // RSM2 counts keys in frames at its own fps, RSM1 in the units the motion counter advances by.
    let scale = if is_rsm2(rsm) {
        1000.0 / rsm.fps.filter(|f| *f > 0.0).unwrap_or(60.0)
    } else {
        1.0
    };
    let frame = |f: i32| (f as f32 * scale).floor() as i32;
    RsmFile {
        version: rsm.version,
        anim_length: (rsm.anim_length as f32 * scale).floor() as u32,
        shade_type: rsm.shade_type,
        alpha: rsm.alpha,
        fps: rsm.fps,
        textures: Vec::new(),
        root_node_names: rsm.root_node_names.clone(),
        nodes: rsm
            .nodes
            .iter()
            .map(|n| RsmNode {
                name: n.name.clone(),
                parent_name: n.parent_name.clone(),
                texture_ids: Vec::new(),
                texture_names: Vec::new(),
                local_transform: n.local_transform,
                translation1: n.translation1,
                translation2: n.translation2,
                rotation_angle: n.rotation_angle,
                rotation_axis: n.rotation_axis,
                scale: n.scale,
                vertices: Vec::new(),
                tex_vertices: Vec::new(),
                faces: Vec::new(),
                scale_keyframes: n
                    .scale_keyframes
                    .iter()
                    .map(|k| nostalre_formats::rsm::ScaleKeyframe {
                        frame: frame(k.frame),
                        scale: k.scale,
                        _reserved: k._reserved,
                    })
                    .collect(),
                rot_keyframes: n
                    .rot_keyframes
                    .iter()
                    .map(|k| nostalre_formats::rsm::RotKeyframe {
                        frame: frame(k.frame),
                        quaternion: k.quaternion,
                    })
                    .collect(),
                translation_keyframes: n
                    .translation_keyframes
                    .iter()
                    .map(|k| nostalre_formats::rsm::PosKeyframe {
                        frame: frame(k.frame),
                        position: k.position,
                        _reserved: k._reserved,
                    })
                    .collect(),
                textures_keyframes: Vec::new(),
            })
            .collect(),
    }
}

fn compile_animated_node(
    node: &RsmNode,
    rsm: &RsmFile,
    node_slot: u32,
    determinant: f32,
    alpha: f32,
    texture_quads: &mut HashMap<String, (Vec<AnimatedModelVertex>, Vec<u32>)>,
) {
    let normal_sign = if determinant < 0.0 { -1.0 } else { 1.0 };
    let local_verts: Vec<glam::Vec3> = node.vertices.iter().map(vec3_from_arr).collect();
    let vert_count = local_verts.len();
    let normals = corner_normals(
        node,
        &local_verts,
        normal_sign,
        rsm.shade_type == SHADE_SMOOTH,
    );
    let unlit = if rsm.shade_type == SHADE_NONE {
        1.0
    } else {
        0.0
    };

    for (face_index, face) in node.faces.iter().enumerate() {
        let v0_idx = face.vertex_ids[0] as usize;
        let v1_idx = face.vertex_ids[1] as usize;
        let v2_idx = face.vertex_ids[2] as usize;
        if v0_idx >= vert_count || v1_idx >= vert_count || v2_idx >= vert_count {
            continue;
        }

        let tex_name = resolve_texture_name(rsm, node, face.texture_index);
        let tex_path = nostalre_resources::texture::named(&tex_name);

        let entry = texture_quads
            .entry(tex_path)
            .or_insert_with(|| (Vec::new(), Vec::new()));
        let base = entry.0.len() as u32;

        for i in 0..3 {
            let vid = face.vertex_ids[i] as usize;
            let tid = face.tex_vertex_ids[i] as usize;
            let pos = local_verts[vid];
            let normal = normals[face_index * 3 + i];
            let (u, v) = if tid < node.tex_vertices.len() {
                (node.tex_vertices[tid].u, node.tex_vertices[tid].v)
            } else {
                (0.0, 0.0)
            };

            entry.0.push(AnimatedModelVertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                tex_coord: [u, v],
                alpha,
                node_slot,
                lit_scale: tex_vertex_lit_scale(node, tid),
                unlit,
            });
        }

        entry.1.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

struct ModelInstance {
    instance_matrix: glam::Mat4,
    alpha: f32,
    anim_type: i32,
    anim_speed: f32,
}

/// Groups a map's model objects by RSM, splitting off the ones whose RSW entry
/// asks for animation and whose RSM actually has a moving track.
fn load_rsw_instances(
    rsw_models: &[&nostalre_formats::rsw::RswModel],
    gnd: &GndFile,
    grf: &GrfArchive,
) -> (
    HashMap<String, RsmFile>,
    HashMap<String, Vec<ModelInstance>>,
    HashMap<String, Vec<ModelInstance>>,
) {
    let zoom = gnd.zoom;
    let scale_factor = zoom / 10.0;
    let center_x = gnd.width as f32 * zoom / 2.0;
    let center_z = gnd.height as f32 * zoom / 2.0;

    let mut rsm_cache: HashMap<String, Option<RsmFile>> = HashMap::new();
    let mut static_instances: HashMap<String, Vec<ModelInstance>> = HashMap::new();
    let mut animated_instances: HashMap<String, Vec<ModelInstance>> = HashMap::new();

    for rsw_model in rsw_models {
        let rsm_path = format!("data\\model\\{}", rsw_model.model_name);

        if !rsm_cache.contains_key(&rsm_path) {
            let rsm = match grf.read_file(&rsm_path) {
                Ok(data) => match RsmFile::parse(&data) {
                    Ok(rsm) => Some(rsm),
                    Err(e) => {
                        tracing::warn!("Failed to parse RSM {rsm_path}: {e}");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read RSM {rsm_path}: {e}");
                    None
                }
            };
            rsm_cache.insert(rsm_path.clone(), rsm);
        }

        let Some(rsm) = rsm_cache.get(&rsm_path).and_then(|r| r.as_ref()) else {
            continue;
        };

        let anim_type = rsw_model.anim_type.unwrap_or(rsm_anim::DEFAULT_ANIM_TYPE);
        let animates =
            anim_type != rsm_anim::ANIM_TYPE_STATIC && rsm_anim::model_has_moving_track(rsm);

        let instance = ModelInstance {
            instance_matrix: build_instance_matrix(rsw_model, scale_factor, center_x, center_z),
            alpha: rsm.alpha.map(|a| a as f32 / 255.0).unwrap_or(1.0),
            anim_type,
            anim_speed: rsw_model.anim_speed.unwrap_or(rsm_anim::DEFAULT_ANIM_SPEED),
        };

        if animates {
            animated_instances
                .entry(rsm_path)
                .or_default()
                .push(instance);
        } else {
            static_instances.entry(rsm_path).or_default().push(instance);
        }
    }

    let loaded = rsm_cache
        .into_iter()
        .filter_map(|(k, v)| v.map(|rsm| (k, rsm)))
        .collect();
    (loaded, static_instances, animated_instances)
}

fn build_mesh(
    rsm_cache: &HashMap<String, RsmFile>,
    instances_by_model: &HashMap<String, Vec<ModelInstance>>,
    grf: &GrfArchive,
    texture_cache: &mut TextureCache,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (Vec<ModelVertex>, Vec<u32>, Vec<DrawBatch>) {
    let mut texture_quads: HashMap<String, (Vec<ModelVertex>, Vec<u32>)> = HashMap::new();

    for (rsm_path, instances) in instances_by_model {
        let Some(rsm) = rsm_cache.get(rsm_path) else {
            continue;
        };

        if rsm.nodes.is_empty() {
            continue;
        }

        preload_rsm_textures(rsm, grf, texture_cache, device, queue);

        let is_only = rsm.nodes.len() == 1;

        let (bbox, node_matrices) = calc_bounding_box(rsm);

        for inst in instances {
            for (node_idx, node) in rsm.nodes.iter().enumerate() {
                compile_node(
                    node,
                    rsm,
                    &node_matrices[node_idx],
                    &bbox,
                    &inst.instance_matrix,
                    is_only,
                    inst.alpha,
                    &mut texture_quads,
                );
            }
        }
    }

    flatten_texture_batches(texture_quads)
}

fn flatten_texture_batches<V: Copy>(
    texture_quads: HashMap<String, (Vec<V>, Vec<u32>)>,
) -> (Vec<V>, Vec<u32>, Vec<DrawBatch>) {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    let mut batches = Vec::new();

    for (tex_name, (verts, idxs)) in texture_quads {
        let vertex_offset = all_vertices.len() as u32;
        let start_index = all_indices.len() as u32;
        all_vertices.extend_from_slice(&verts);
        all_indices.extend(idxs.iter().map(|i| i + vertex_offset));
        batches.push(DrawBatch {
            bounds: None, visible: true,
            texture_name: tex_name,
            start_index,
            index_count: idxs.len() as u32,
        });
    }

    (all_vertices, all_indices, batches)
}

fn build_instance_matrix(
    model: &nostalre_formats::rsw::RswModel,
    scale_factor: f32,
    center_x: f32,
    center_z: f32,
) -> glam::Mat4 {
    let pos = glam::Vec3::new(
        model.position[0] * scale_factor + center_x,
        model.position[1] * scale_factor,
        model.position[2] * scale_factor + center_z,
    );

    let rot_x = model.rotation[0].to_radians();
    let rot_y = model.rotation[1].to_radians();
    let rot_z = model.rotation[2].to_radians();

    let scale = glam::Vec3::new(
        model.scale[0] * scale_factor,
        model.scale[1] * scale_factor,
        model.scale[2] * scale_factor,
    );

    glam::Mat4::from_translation(pos)
        * glam::Mat4::from_rotation_z(rot_z)
        * glam::Mat4::from_rotation_x(rot_x)
        * glam::Mat4::from_rotation_y(rot_y)
        * glam::Mat4::from_scale(scale)
}

pub(crate) fn mat3_to_mat4(m: &nostalre_formats::Mat3) -> glam::Mat4 {
    glam::Mat4::from_cols(
        glam::Vec4::new(m[0][0], m[0][1], m[0][2], 0.0),
        glam::Vec4::new(m[1][0], m[1][1], m[1][2], 0.0),
        glam::Vec4::new(m[2][0], m[2][1], m[2][2], 0.0),
        glam::Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

fn vec3_from_arr(v: &nostalre_formats::Vec3) -> glam::Vec3 {
    glam::Vec3::new(v[0], v[1], v[2])
}

/// RSM 2.2 and later (`.rsm2`): stored y-up, where RSM1 and the map are y-down, with node transforms of their own
/// (Korangar's `calculate_matrices_rsm2`).
fn is_rsm2(rsm: &RsmFile) -> bool {
    rsm.version >= (2, 2)
}

/// What a model's node space goes through before its instance matrix: RSM1 is recentred on x/z with its top face
/// at y=0; RSM2 is only turned y-down, its origin already its base.
fn model_root_matrix(rsm: &RsmFile, bbox: &BoundingBox) -> glam::Mat4 {
    if is_rsm2(rsm) {
        glam::Mat4::from_scale(glam::Vec3::new(1.0, -1.0, 1.0))
    } else {
        glam::Mat4::from_translation(glam::Vec3::new(-bbox.center.x, -bbox.max.y, -bbox.center.z))
    }
}

/// The constant part of a node's transform, applied after the animated
/// parent chain. RSM2 nodes carry theirs in the chain itself.
fn node_post_matrix(rsm: &RsmFile, node: &RsmNode, is_only: bool) -> glam::Mat4 {
    if is_rsm2(rsm) {
        return glam::Mat4::IDENTITY;
    }
    let mut matrix = glam::Mat4::IDENTITY;
    if !is_only && let Some(offset) = node.translation1 {
        matrix = glam::Mat4::from_translation(vec3_from_arr(&offset));
    }
    matrix * mat3_to_mat4(&node.local_transform)
}

/// Accumulated parent-chain matrix for every node at `frame`, plus which nodes
/// are reachable from the model's roots.
fn node_accum_matrices(rsm: &RsmFile, frame: f32) -> (Vec<glam::Mat4>, Vec<bool>) {
    let mut node_matrices = vec![glam::Mat4::IDENTITY; rsm.nodes.len()];
    let mut visited = vec![false; rsm.nodes.len()];
    node_accum_matrices_into(rsm, frame, &mut node_matrices, &mut visited);
    (node_matrices, visited)
}

fn node_accum_matrices_into(
    rsm: &RsmFile,
    frame: f32,
    node_matrices: &mut [glam::Mat4],
    visited: &mut [bool],
) {
    node_matrices.fill(glam::Mat4::IDENTITY);
    visited.fill(false);

    let name_to_idx: HashMap<&str, usize> = rsm
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.name.as_str(), i))
        .collect();

    let root_indices: Vec<usize> = if !rsm.root_node_names.is_empty() {
        rsm.root_node_names
            .iter()
            .filter_map(|name| name_to_idx.get(name.as_str()).copied())
            .collect()
    } else {
        vec![0]
    };

    // RSM1 chains node_local_matrix down the tree; RSM2 chains joints (rsm_anim::rsm2_joint).
    let rsm2 = is_rsm2(rsm);
    fn traverse(
        node_idx: usize,
        parent: rsm_anim::Joint,
        rsm2: bool,
        nodes: &[RsmNode],
        frame: f32,
        node_matrices: &mut [glam::Mat4],
        visited: &mut [bool],
    ) {
        if visited[node_idx] {
            return;
        }
        visited[node_idx] = true;

        let node = &nodes[node_idx];
        let joint = if rsm2 {
            rsm_anim::rsm2_joint(node, frame, &parent)
        } else {
            parent.with_arm(parent.arm * rsm_anim::node_local_matrix(node, frame))
        };
        node_matrices[node_idx] = joint.arm;

        for (i, child) in nodes.iter().enumerate() {
            if i != node_idx && child.parent_name == node.name && node.name != node.parent_name {
                traverse(i, joint, rsm2, nodes, frame, node_matrices, visited);
            }
        }
    }

    for &root_idx in &root_indices {
        traverse(
            root_idx,
            rsm_anim::Joint::ROOT,
            rsm2,
            &rsm.nodes,
            frame,
            node_matrices,
            visited,
        );
    }
}

fn calc_bounding_box(rsm: &RsmFile) -> (BoundingBox, Vec<glam::Mat4>) {
    let is_only = rsm.nodes.len() == 1;
    let (node_matrices, reachable) = node_accum_matrices(rsm, 0.0);

    let mut bbox = BoundingBox::new();
    for (node_idx, node) in rsm.nodes.iter().enumerate() {
        if !reachable[node_idx] {
            continue;
        }
        let local = node_matrices[node_idx] * node_post_matrix(rsm, node, is_only);
        for vert in &node.vertices {
            bbox.extend(local.transform_point3(vec3_from_arr(vert)));
        }
    }

    // Guard against models with no vertices (bbox stays at infinity → NaN center)
    if bbox.min.x == f32::INFINITY {
        bbox.min = glam::Vec3::ZERO;
        bbox.max = glam::Vec3::ZERO;
    }

    bbox.finalize();
    (bbox, node_matrices)
}

fn compile_node(
    node: &RsmNode,
    rsm: &RsmFile,
    node_matrix: &glam::Mat4,
    bbox: &BoundingBox,
    instance_matrix: &glam::Mat4,
    is_only: bool,
    alpha: f32,
    texture_quads: &mut HashMap<String, (Vec<ModelVertex>, Vec<u32>)>,
) {
    let matrix = model_root_matrix(rsm, bbox) * *node_matrix * node_post_matrix(rsm, node, is_only);
    let model_view = *instance_matrix * matrix;
    let normal_sign = if model_view.determinant() < 0.0 {
        -1.0
    } else {
        1.0
    };

    let world_verts: Vec<glam::Vec3> = node
        .vertices
        .iter()
        .map(|v| model_view.transform_point3(vec3_from_arr(v)))
        .collect();

    let vert_count = world_verts.len();
    let normals = corner_normals(
        node,
        &world_verts,
        normal_sign,
        rsm.shade_type == SHADE_SMOOTH,
    );
    let unlit = if rsm.shade_type == SHADE_NONE {
        1.0
    } else {
        0.0
    };

    for (face_index, face) in node.faces.iter().enumerate() {
        let v0_idx = face.vertex_ids[0] as usize;
        let v1_idx = face.vertex_ids[1] as usize;
        let v2_idx = face.vertex_ids[2] as usize;
        if v0_idx >= vert_count || v1_idx >= vert_count || v2_idx >= vert_count {
            continue;
        }

        let tex_name = resolve_texture_name(rsm, node, face.texture_index);
        let tex_path = nostalre_resources::texture::named(&tex_name);

        let entry = texture_quads
            .entry(tex_path)
            .or_insert_with(|| (Vec::new(), Vec::new()));
        let base = entry.0.len() as u32;

        for i in 0..3 {
            let vid = face.vertex_ids[i] as usize;
            let tid = face.tex_vertex_ids[i] as usize;
            let pos = world_verts[vid];
            let normal = normals[face_index * 3 + i];
            let (u, v) = if tid < node.tex_vertices.len() {
                (node.tex_vertices[tid].u, node.tex_vertices[tid].v)
            } else {
                (0.0, 0.0)
            };

            entry.0.push(ModelVertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                tex_coord: [u, v],
                alpha,
                lit_scale: tex_vertex_lit_scale(node, tid),
                unlit,
            });
        }

        entry.1.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

const SHADE_NONE: u32 = 0;
const SHADE_SMOOTH: u32 = 2;

/// One normal per face corner, in the space `verts` is given in. Smooth-shaded
/// models average the faces meeting at a vertex within a smooth group; the
/// others repeat the face normal across its three corners.
fn corner_normals(
    node: &RsmNode,
    verts: &[glam::Vec3],
    normal_sign: f32,
    smooth: bool,
) -> Vec<glam::Vec3> {
    let face_normals: Vec<glam::Vec3> = node
        .faces
        .iter()
        .map(|face| {
            let ids = face.vertex_ids.map(|id| id as usize);
            match (verts.get(ids[0]), verts.get(ids[1]), verts.get(ids[2])) {
                (Some(v0), Some(v1), Some(v2)) => {
                    (*v1 - *v0).cross(*v2 - *v0).normalize_or_zero() * normal_sign
                }
                _ => glam::Vec3::ZERO,
            }
        })
        .collect();

    if !smooth {
        return face_normals.into_iter().flat_map(|n| [n; 3]).collect();
    }

    let mut sums: HashMap<(i32, usize), glam::Vec3> = HashMap::new();
    for (face_index, face) in node.faces.iter().enumerate() {
        for id in face.vertex_ids {
            *sums
                .entry((face.smooth_group, id as usize))
                .or_insert(glam::Vec3::ZERO) += face_normals[face_index];
        }
    }

    node.faces
        .iter()
        .flat_map(|face| {
            face.vertex_ids
                .map(|id| sums[&(face.smooth_group, id as usize)].normalize_or_zero())
        })
        .collect()
}

/// A texture vertex carrying a colour renders at half the lit brightness.
fn tex_vertex_lit_scale(node: &RsmNode, tex_vertex_id: usize) -> f32 {
    match node.tex_vertices.get(tex_vertex_id) {
        Some(tv) if tv.color != 0 => 0.5,
        _ => 1.0,
    }
}

fn resolve_texture_name(rsm: &RsmFile, node: &RsmNode, face_texture_index: u16) -> String {
    if !node.texture_names.is_empty() {
        node.texture_names
            .get(face_texture_index as usize)
            .cloned()
            .unwrap_or_default()
    } else {
        let tex_id = node
            .texture_ids
            .get(face_texture_index as usize)
            .copied()
            .unwrap_or(0) as usize;
        rsm.textures.get(tex_id).cloned().unwrap_or_default()
    }
}

fn preload_rsm_textures(
    rsm: &RsmFile,
    grf: &GrfArchive,
    texture_cache: &mut TextureCache,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    for tex_name in &rsm.textures {
        let path = nostalre_resources::texture::named(&tex_name);
        texture_cache.get_or_load(&path, grf, device, queue, false);
    }
    for node in &rsm.nodes {
        for tex_name in &node.texture_names {
            let path = nostalre_resources::texture::named(&tex_name);
            texture_cache.get_or_load(&path, grf, device, queue, false);
        }
    }
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

fn create_animated_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    global_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    node_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("model_animated"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/model_animated.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("model_animated"),
        bind_group_layouts: &[Some(global_bind_group_layout), Some(texture_bind_group_layout), Some(node_bind_group_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("model_animated"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[AnimatedModelVertex::LAYOUT],
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
            depth_write_enabled:Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    global_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("model"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/model.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("model"),
        bind_group_layouts: &[Some(global_bind_group_layout), Some(texture_bind_group_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("model"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[ModelVertex::LAYOUT],
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
            depth_write_enabled:Some(true),
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

    #[test]
    fn instance_matrix_places_model_at_map_center_when_position_is_zero() {
        let model = nostalre_formats::rsw::RswModel {
            name: None,
            anim_type: None,
            anim_speed: None,
            block_type: None,
            model_name: String::new(),
            node_name: String::new(),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [10.0, 10.0, 10.0],
        };
        let mat = build_instance_matrix(&model, 1.0, 500.0, 500.0);
        let origin = mat.transform_point3(glam::Vec3::ZERO);
        assert!((origin.x - 500.0).abs() < 0.01, "x={}", origin.x);
        assert!(origin.y.abs() < 0.01, "y={}", origin.y);
        assert!((origin.z - 500.0).abs() < 0.01, "z={}", origin.z);

        let point = mat.transform_point3(glam::Vec3::new(0.0, 100.0, 0.0));
        assert!((point.y - 1000.0).abs() < 0.01, "y={}", point.y);
    }

    #[test]
    fn mat3_to_mat4_identity() {
        let m: nostalre_formats::Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let result = mat3_to_mat4(&m);
        let expected = glam::Mat4::IDENTITY;
        for i in 0..16 {
            assert!((result.to_cols_array()[i] - expected.to_cols_array()[i]).abs() < 1e-6);
        }
    }

    fn node(
        name: &str,
        parent: &str,
        rot_keys: Vec<nostalre_formats::rsm::RotKeyframe>,
    ) -> RsmNode {
        RsmNode {
            name: name.into(),
            parent_name: parent.into(),
            texture_ids: vec![0],
            texture_names: vec![],
            local_transform: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation1: Some([0.0, 0.0, 0.0]),
            translation2: [0.0, 0.0, 0.0],
            rotation_angle: Some(0.0),
            rotation_axis: Some([0.0, 1.0, 0.0]),
            scale: Some([1.0, 1.0, 1.0]),
            vertices: vec![[1.0, 0.0, 0.0]],
            tex_vertices: vec![],
            faces: vec![],
            scale_keyframes: vec![],
            rot_keyframes: rot_keys,
            translation_keyframes: vec![],
            textures_keyframes: vec![],
        }
    }

    #[test]
    fn only_nodes_with_a_moving_track_change_between_frames() {
        // 90°, not 180°: slerp's shortest-path flip makes a 180° target ambiguous.
        let quarter_turn = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let blades = node(
            "blades",
            "hub",
            vec![
                nostalre_formats::rsm::RotKeyframe {
                    frame: 0,
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                nostalre_formats::rsm::RotKeyframe {
                    frame: 100,
                    quaternion: [
                        quarter_turn.x,
                        quarter_turn.y,
                        quarter_turn.z,
                        quarter_turn.w,
                    ],
                },
            ],
        );
        let rsm = RsmFile {
            version: (1, 4),
            anim_length: 100,
            shade_type: 0,
            alpha: Some(255),
            fps: None,
            textures: vec!["test.bmp".into()],
            root_node_names: vec!["hub".into()],
            nodes: vec![node("hub", "", vec![]), blades],
        };

        assert!(rsm_anim::model_has_moving_track(&rsm));

        let (at_zero, reachable) = node_accum_matrices(&rsm, 0.0);
        let (at_mid, _) = node_accum_matrices(&rsm, 50.0);
        assert!(reachable.iter().all(|&r| r), "child was not reached");

        let point = glam::Vec3::new(1.0, 0.0, 0.0);
        assert!(
            (at_zero[0].transform_point3(point) - at_mid[0].transform_point3(point)).length()
                < 1e-6,
            "static parent moved"
        );

        let spun = at_mid[1].transform_point3(point);
        let eighth = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_4) * point;
        assert!(
            (spun - eighth).length() < 1e-4,
            "animated child at mid-frame: {spun:?} expected {eighth:?}"
        );
    }

    /// An RSM2 tree (y-up, like `ilusion\\tree_s_01.rsm2`): a trunk at the origin and a branch node 10 up, turned a
    /// quarter about y. Each node lands at `translation2 * matrix`, and the model stands up in the y-down map.
    #[test]
    fn an_rsm2_model_stands_up_with_its_nodes_in_place() {
        let quarter_y = [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
        let rsm2_node =
            |name: &str, parent: &str, at: [f32; 3], matrix: nostalre_formats::Mat3| RsmNode {
                local_transform: matrix,
                translation1: None,
                translation2: at,
                rotation_angle: None,
                rotation_axis: None,
                scale: None,
                ..node(name, parent, vec![])
            };
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let rsm = RsmFile {
            version: (2, 3),
            anim_length: 0,
            shade_type: 0,
            alpha: Some(255),
            fps: Some(30.0),
            textures: vec![],
            root_node_names: vec!["trunk".into()],
            nodes: vec![
                rsm2_node("trunk", "", [0.0, 0.0, 0.0], identity),
                rsm2_node("branch", "trunk", [0.0, 10.0, 0.0], quarter_y),
            ],
        };

        let (bbox, matrices) = calc_bounding_box(&rsm);
        let to_map = |node_idx: usize, v: glam::Vec3| {
            (model_root_matrix(&rsm, &bbox)
                * matrices[node_idx]
                * node_post_matrix(&rsm, &rsm.nodes[node_idx], false))
            .transform_point3(v)
        };
        let expected = |v: glam::Vec3| v * glam::Vec3::new(1.0, -1.0, 1.0);
        assert!(
            (to_map(0, glam::Vec3::new(0.0, 5.0, 0.0)) - expected(glam::Vec3::new(0.0, 5.0, 0.0)))
                .length()
                < 1e-5
        );
        // the branch's x axis is turned onto -z, then raised 10: above the trunk, which is -y in the map
        let tip = to_map(1, glam::Vec3::X);
        let file_pose = glam::Mat4::from_translation(glam::Vec3::new(0.0, 10.0, 0.0))
            * mat3_to_mat4(&quarter_y);
        assert!(
            (tip - expected(file_pose.transform_point3(glam::Vec3::X))).length() < 1e-5,
            "{tip:?}"
        );
        assert!(tip.y < 0.0);
    }

    /// The box `compile_node` places: RSM1 centred on x/z with its top at y=0, RSM2 only turned y-down.
    #[test]
    fn the_root_matrix_places_rsm1_by_its_top_and_rsm2_by_its_origin() {
        let mut bbox = BoundingBox::new();
        bbox.extend(glam::Vec3::new(-2.0, 1.0, 4.0));
        bbox.extend(glam::Vec3::new(6.0, 9.0, 8.0));
        bbox.finalize();
        let model = |version| RsmFile {
            version,
            anim_length: 0,
            shade_type: 0,
            alpha: None,
            fps: None,
            textures: vec![],
            root_node_names: vec![],
            nodes: vec![],
        };
        let middle = |rsm: &RsmFile| {
            let root = model_root_matrix(rsm, &bbox);
            (root.transform_point3(bbox.min) + root.transform_point3(bbox.max)) * 0.5
        };
        assert_eq!(middle(&model((1, 4))), glam::Vec3::new(0.0, -4.0, 0.0));
        assert_eq!(middle(&model((2, 3))), glam::Vec3::new(2.0, -5.0, 6.0));
    }

    /// Two perpendicular triangles sharing the edge `(0,0,0)-(0,0,1)`.
    fn roof_rsm(shade_type: u32, smooth_groups: [i32; 2]) -> RsmFile {
        let face = |vertex_ids: [u16; 3], smooth_group: i32| nostalre_formats::rsm::RsmFace {
            vertex_ids,
            tex_vertex_ids: [0, 0, 0],
            texture_index: 0,
            padding: 0,
            two_sided: 0,
            smooth_group,
            extra_smooth_groups: vec![],
        };

        RsmFile {
            version: (1, 4),
            anim_length: 0,
            shade_type,
            alpha: Some(255),
            fps: None,
            textures: vec!["test.bmp".into()],
            root_node_names: vec![],
            nodes: vec![RsmNode {
                name: "root".into(),
                parent_name: String::new(),
                texture_ids: vec![0],
                texture_names: vec![],
                local_transform: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                translation1: Some([0.0, 0.0, 0.0]),
                translation2: [0.0, 0.0, 0.0],
                rotation_angle: Some(0.0),
                rotation_axis: Some([0.0, 1.0, 0.0]),
                scale: Some([1.0, 1.0, 1.0]),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
                tex_vertices: vec![],
                faces: vec![
                    face([0, 1, 2], smooth_groups[0]),
                    face([0, 3, 1], smooth_groups[1]),
                ],
                scale_keyframes: vec![],
                rot_keyframes: vec![],
                translation_keyframes: vec![],
                textures_keyframes: vec![],
            }],
        }
    }

    fn compile_roof(shade_type: u32, smooth_groups: [i32; 2]) -> Vec<ModelVertex> {
        let rsm = roof_rsm(shade_type, smooth_groups);
        let (bbox, node_matrices) = calc_bounding_box(&rsm);
        let mut texture_quads: HashMap<String, (Vec<ModelVertex>, Vec<u32>)> = HashMap::new();
        compile_node(
            &rsm.nodes[0],
            &rsm,
            &node_matrices[0],
            &bbox,
            &glam::Mat4::IDENTITY,
            true,
            1.0,
            &mut texture_quads,
        );
        texture_quads.into_values().next().unwrap().0
    }

    #[test]
    fn gouraud_averages_normals_over_a_smooth_group() {
        let ridge = glam::Vec3::new(1.0, 1.0, 0.0).normalize();

        let shared = compile_roof(SHADE_SMOOTH, [0, 0]);
        assert!(
            (glam::Vec3::from(shared[0].normal) - ridge).length() < 1e-5,
            "shared corner: {:?}",
            shared[0].normal
        );
        assert!(
            (glam::Vec3::from(shared[3].normal) - ridge).length() < 1e-5,
            "shared corner on the other face: {:?}",
            shared[3].normal
        );
        assert!(
            (glam::Vec3::from(shared[2].normal) - glam::Vec3::Y).length() < 1e-5,
            "corner belonging to one face only: {:?}",
            shared[2].normal
        );

        let split = compile_roof(SHADE_SMOOTH, [0, 1]);
        assert!(
            (glam::Vec3::from(split[0].normal) - glam::Vec3::Y).length() < 1e-5,
            "corner across a smooth-group boundary: {:?}",
            split[0].normal
        );
        assert!(
            (glam::Vec3::from(split[3].normal) - glam::Vec3::X).length() < 1e-5,
            "corner across a smooth-group boundary: {:?}",
            split[3].normal
        );

        let flat = compile_roof(1, [0, 0]);
        assert!(
            (glam::Vec3::from(flat[0].normal) - glam::Vec3::Y).length() < 1e-5,
            "flat shading kept the face normal: {:?}",
            flat[0].normal
        );
    }

    #[test]
    fn unshaded_models_bypass_the_scene_light() {
        assert!(
            compile_roof(SHADE_NONE, [0, 0])
                .iter()
                .all(|v| v.unlit == 1.0),
            "shade type 0 must ignore the light"
        );
        assert!(
            compile_roof(SHADE_SMOOTH, [0, 0])
                .iter()
                .all(|v| v.unlit == 0.0),
            "shaded models must keep the light"
        );
    }

    #[test]
    fn bounding_box_single_node_model() {
        let rsm = RsmFile {
            version: (1, 4),
            anim_length: 0,
            shade_type: 0,
            alpha: Some(255),
            fps: None,
            textures: vec!["test.bmp".into()],
            root_node_names: vec![],
            nodes: vec![RsmNode {
                name: "root".into(),
                parent_name: String::new(),
                texture_ids: vec![0],
                texture_names: vec![],
                local_transform: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                translation1: Some([0.0, 0.0, 0.0]),
                translation2: [0.0, 0.0, 0.0],
                rotation_angle: Some(0.0),
                rotation_axis: Some([0.0, 1.0, 0.0]),
                scale: Some([1.0, 1.0, 1.0]),
                vertices: vec![[-1.0, -1.0, -1.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]],
                tex_vertices: vec![],
                faces: vec![],
                scale_keyframes: vec![],
                rot_keyframes: vec![],
                translation_keyframes: vec![],
                textures_keyframes: vec![],
            }],
        };

        let (bbox, matrices) = calc_bounding_box(&rsm);
        assert_eq!(matrices.len(), 1);
        assert!((bbox.min.x - (-1.0)).abs() < 0.01, "min.x={}", bbox.min.x);
        assert!((bbox.max.x - 1.0).abs() < 0.01, "max.x={}", bbox.max.x);
        assert!(
            (bbox.center.x - 0.0).abs() < 0.01,
            "center.x={}",
            bbox.center.x
        );
    }
}

// Conservative homogeneous AABB test: a sector is rejected only when all eight
// corners are outside the SAME clip plane, including large screen-crossing props.
fn bounds_visible(vp: glam::Mat4, lo: [f32;3], hi: [f32;3]) -> bool {
    let corners: [_; 8] = std::array::from_fn(|i| vp * glam::Vec4::new(
        if i&1==0 {lo[0]} else {hi[0]}, if i&2==0 {lo[1]} else {hi[1]},
        if i&4==0 {lo[2]} else {hi[2]},1.0));
    !(0..6).any(|plane| corners.iter().all(|p| match plane {
        0=>p.x < -p.w, 1=>p.x > p.w, 2=>p.y < -p.w, 3=>p.y > p.w,
        4=>p.z < 0.0, _=>p.z > p.w,
    }))
}
fn sector_batches(vertices: &[ModelVertex], indices: &[u32], batches: Vec<DrawBatch>) -> (Vec<u32>,Vec<DrawBatch>) {
    let mut result=indices.to_vec(); let mut output=batches.clone();
    for batch in batches {
        let mut sectors: std::collections::BTreeMap<(i32,i32),Vec<u32>>=std::collections::BTreeMap::new();
        for triangle in indices[batch.start_index as usize..(batch.start_index+batch.index_count) as usize].chunks_exact(3) {
            let center=triangle.iter().map(|&i| glam::Vec3::from_array(vertices[i as usize].position)).sum::<glam::Vec3>()/3.0;
            sectors.entry(((center.x/160.0).floor() as i32,(center.z/160.0).floor() as i32)).or_default().extend_from_slice(triangle);
        }
        for (_, sector) in sectors {
            let mut lo=glam::Vec3::splat(f32::INFINITY); let mut hi=glam::Vec3::splat(f32::NEG_INFINITY);
            for &i in &sector { let p=glam::Vec3::from_array(vertices[i as usize].position);lo=lo.min(p);hi=hi.max(p); }
            output.push(DrawBatch{texture_name:batch.texture_name.clone(),start_index:result.len() as u32,index_count:sector.len() as u32,bounds:Some((lo.to_array(),hi.to_array())),visible:true});
            result.extend(sector);
        }
    }
    (result,output)
}
#[cfg(test)] mod switch_fluency_tests {
    use super::*;
    #[test] fn conservative_sector_bounds_keep_crossing_objects() {
        assert!(bounds_visible(glam::Mat4::IDENTITY,[-10.0,-10.0,0.2],[10.0,10.0,0.8]));
        assert!(!bounds_visible(glam::Mat4::IDENTITY,[2.0,0.0,0.2],[3.0,1.0,0.8]));
        assert!(!bounds_visible(glam::Mat4::IDENTITY,[0.0,0.0,-2.0],[1.0,1.0,-1.0]));
    }
}
