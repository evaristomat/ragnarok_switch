use crate::effect::queue::{BlendBucket, DrawRecord, PipelineKind, partition_and_sort};
use crate::effect::registry::EffectPrimitiveRegistry;
use crate::mesh_batch::MeshBatcher;
use crate::sprite::{SpriteRenderer, SpriteVertex};

const INITIAL_VERTEX_CAPACITY: usize = 4096;
const INITIAL_INDEX_CAPACITY: usize = 8192;

pub struct EffectDispatcher {
    meshes: MeshBatcher<SpriteVertex>,
    /// Each blend bucket's records in draw order, kept across frames.
    buckets: [Vec<usize>; 5],
    /// How many records the last dispatch drew, so the next frame's list is allocated once at about its size.
    last_records: usize,
}

impl EffectDispatcher {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            meshes: MeshBatcher::new(
                device,
                "effect_dispatch",
                INITIAL_VERTEX_CAPACITY,
                INITIAL_INDEX_CAPACITY,
            ),
            buckets: Default::default(),
            last_records: 0,
        }
    }

    /// An empty record list for this frame, with room for as many records as the last dispatch drew.
    pub fn record_list<'tex>(&self) -> Vec<DrawRecord<'tex>> {
        Vec::with_capacity(self.last_records)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<'tex>(
        &mut self,
        records: &[DrawRecord<'tex>],
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_bind_group: &wgpu::BindGroup,
        sprite_uniform_bind_group: &wgpu::BindGroup,
        sprite_renderer: &SpriteRenderer,
        primitives: &EffectPrimitiveRegistry,
    ) {
        self.last_records = records.len();
        batch(&mut self.meshes, &mut self.buckets, records);
        if self.meshes.is_empty() {
            return;
        }
        self.meshes.upload(device, queue);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("effect_unified"),
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

        self.meshes.bind(&mut pass);

        let mut current_kind: Option<PipelineKind> = None;
        let mut current_bucket: Option<BlendBucket> = None;
        let mut current_group0_kind: Option<PipelineKind> = None;
        let mut current_texture: Option<&wgpu::BindGroup> = None;

        for run in self.meshes.runs() {
            let record = &records[run.first];
            let (kind, bucket, texture) = (record.pipeline, record.blend, record.texture);
            let group0_kind = match kind {
                PipelineKind::Sprite => PipelineKind::Sprite,
                _ => PipelineKind::Frustum,
            };
            if current_group0_kind != Some(group0_kind) {
                match group0_kind {
                    PipelineKind::Sprite => {
                        pass.set_bind_group(0, sprite_uniform_bind_group, &[]);
                    }
                    _ => {
                        pass.set_bind_group(0, camera_bind_group, &[]);
                    }
                }
                current_group0_kind = Some(group0_kind);
                // wgpu keeps group 1 across this (one texture layout everywhere); bound again to be safe
                current_texture = None;
            }

            if current_kind != Some(kind) || current_bucket != Some(bucket) {
                pass.set_pipeline(pipeline_for(kind, bucket, sprite_renderer, primitives));
                current_kind = Some(kind);
                current_bucket = Some(bucket);
            }

            if !current_texture.is_some_and(|t| std::ptr::eq(t, texture)) {
                pass.set_bind_group(1, texture, &[]);
                current_texture = Some(texture);
            }
            pass.draw_indexed(run.indices.clone(), 0, 0..1);
        }
    }
}

/// Lays the records out in `meshes` in draw order (blend bucket, then depth), neighbours sharing pipeline, blend
/// and texture joined into one draw: a run's indices are contiguous, so it draws in the order one call a record
/// would. A run's pipeline, blend and texture are its first record's.
fn batch(
    meshes: &mut MeshBatcher<SpriteVertex>,
    buckets: &mut [Vec<usize>; 5],
    records: &[DrawRecord<'_>],
) {
    partition_and_sort(records, buckets);
    meshes.clear();
    // a bucket's index is its place in BlendBucket::FLUSH_ORDER, so walking them in turn is draw order
    for &i in buckets.iter().flatten() {
        let r = &records[i];
        meshes.push(i, &r.vertices, &r.indices, |first| {
            let f = &records[first];
            f.pipeline == r.pipeline && f.blend == r.blend && std::ptr::eq(f.texture, r.texture)
        });
    }
}

fn pipeline_for<'a>(
    kind: PipelineKind,
    bucket: BlendBucket,
    sprite_renderer: &'a SpriteRenderer,
    primitives: &'a EffectPrimitiveRegistry,
) -> &'a wgpu::RenderPipeline {
    match kind {
        PipelineKind::Sprite => match bucket {
            BlendBucket::Alpha => &sprite_renderer.pipelines.normal,
            BlendBucket::AlphaNoDepth => &sprite_renderer.pipelines.overlay,
            BlendBucket::Additive => &sprite_renderer.pipelines.additive,
            BlendBucket::AdditiveNoDepth => &sprite_renderer.pipelines.additive_overlay,
            BlendBucket::Multiply => &sprite_renderer.pipelines.normal,
        },
        _ => primitives
            .get(kind)
            .expect("primitive renderer registered for kind")
            .pipeline(bucket),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{bind_groups as textures, device};

    fn batched(records: &[DrawRecord<'_>]) -> MeshBatcher<SpriteVertex> {
        let mut meshes = MeshBatcher::new(&device().0, "test", 4, 6);
        batch(&mut meshes, &mut Default::default(), records);
        meshes
    }

    fn quad(
        depth: f32,
        emission: u32,
        blend: BlendBucket,
        texture: &'static wgpu::BindGroup,
    ) -> DrawRecord<'static> {
        let vertex = SpriteVertex {
            position: [depth; 3],
            tex_coord: [0.0; 2],
            color: [1.0; 4],
        };
        DrawRecord::new(
            depth,
            emission,
            blend,
            PipelineKind::WorldQuad,
            vec![vertex; 4],
            vec![0, 1, 2, 0, 2, 3],
            texture,
        )
    }

    #[test]
    fn records_sharing_state_draw_as_one() {
        let t = textures();
        let records: Vec<_> = (0..3)
            .map(|i| quad(i as f32, i, BlendBucket::Additive, &t[0]))
            .collect();
        let mut meshes = batched(&records[..1]);
        let mut buckets = Default::default();
        // a frame before leaves its lists behind; the next batch starts them over
        batch(&mut meshes, &mut buckets, &records[..2]);
        batch(&mut meshes, &mut buckets, &records);
        assert_eq!(
            (
                meshes.vertices().len(),
                meshes.indices().len(),
                meshes.runs().len()
            ),
            (12, 18, 1)
        );
        assert_eq!(meshes.runs()[0].indices, 0..18);
        assert_eq!(&meshes.indices()[6..12], &[4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn a_change_of_texture_or_blend_starts_a_draw_in_depth_order() {
        let t = textures();
        let records = vec![
            quad(2.0, 0, BlendBucket::Alpha, &t[0]),
            quad(1.0, 1, BlendBucket::Alpha, &t[1]),
            quad(3.0, 2, BlendBucket::Alpha, &t[0]),
            quad(0.0, 3, BlendBucket::Additive, &t[0]),
            quad(4.0, 4, BlendBucket::Alpha, &t[0]),
        ];
        let meshes = batched(&records);
        let runs: Vec<_> = meshes
            .runs()
            .iter()
            .map(|run| (records[run.first].blend, run.indices.len() / 6))
            .collect();
        // alpha by depth: t1, then t0 three times; then additive
        assert_eq!(
            runs,
            [
                (BlendBucket::Alpha, 1),
                (BlendBucket::Alpha, 3),
                (BlendBucket::Additive, 1)
            ]
        );
        assert!(std::ptr::eq(records[meshes.runs()[0].first].texture, &t[1]));
        assert_eq!(
            meshes.vertices()[4].position,
            [2.0; 3],
            "geometry laid out in draw order"
        );
    }
}
