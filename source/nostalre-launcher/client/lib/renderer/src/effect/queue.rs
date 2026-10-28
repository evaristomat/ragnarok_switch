use std::borrow::Cow;

use crate::camera::Camera;
use crate::effect::BlendKind;
use crate::sprite::SpriteVertices;

pub fn view_z(camera: &Camera, pos: [f32; 3]) -> f32 {
    let p = camera.view_matrix() * glam::Vec4::new(pos[0], pos[1], pos[2], 1.0);
    p.z
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BlendBucket {
    Alpha,
    AlphaNoDepth,
    Additive,
    AdditiveNoDepth,
    Multiply,
}

impl BlendBucket {
    pub const FLUSH_ORDER: [BlendBucket; 5] = [
        BlendBucket::Alpha,
        BlendBucket::AlphaNoDepth,
        BlendBucket::Additive,
        BlendBucket::AdditiveNoDepth,
        BlendBucket::Multiply,
    ];

    pub fn from_blend_kind(blend: BlendKind) -> BlendBucket {
        match blend {
            BlendKind::Alpha | BlendKind::AlphaGamma => BlendBucket::Alpha,
            BlendKind::Additive => BlendBucket::Additive,
            BlendKind::Multiply => BlendBucket::Multiply,
            BlendKind::Raw { src: _, dst } => {
                if dst == 6 {
                    BlendBucket::Alpha
                } else {
                    BlendBucket::Additive
                }
            }
        }
    }

    pub fn flush_index(self) -> usize {
        Self::FLUSH_ORDER
            .iter()
            .position(|b| *b == self)
            .expect("BlendBucket missing from FLUSH_ORDER")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PipelineKind {
    Frustum,
    Cylinder,
    GroundDisc,
    QuadHorn,
    Sphere,
    WorldQuad,
    WorldQuadGamma,
    Texture3D,
    RadialRing,
    LineStrip,
    Sprite,
    FullscreenOverlay,
}

pub struct DrawRecord<'tex> {
    pub depth: f32,
    pub emission_index: u32,
    pub blend: BlendBucket,
    pub pipeline: PipelineKind,
    pub vertices: SpriteVertices,
    pub indices: Cow<'static, [u32]>,
    pub texture: &'tex wgpu::BindGroup,
}

impl<'tex> DrawRecord<'tex> {
    pub fn new(
        depth: f32,
        emission_index: u32,
        blend: BlendBucket,
        pipeline: PipelineKind,
        vertices: impl Into<SpriteVertices>,
        indices: impl Into<Cow<'static, [u32]>>,
        texture: &'tex wgpu::BindGroup,
    ) -> Self {
        Self {
            depth,
            emission_index,
            blend,
            pipeline,
            vertices: vertices.into(),
            indices: indices.into(),
            texture,
        }
    }
}

/// Sorts the records' indices into `buckets`, one a blend bucket in depth order, refilling lists kept across frames.
pub fn partition_and_sort(records: &[DrawRecord<'_>], buckets: &mut [Vec<usize>; 5]) {
    buckets.iter_mut().for_each(Vec::clear);
    for (i, r) in records.iter().enumerate() {
        buckets[r.blend.flush_index()].push(i);
    }
    // a total order (a NaN depth sorts to one end instead of breaking the sort), ties kept in push order without
    // the stable sort's buffer
    for list in buckets.iter_mut() {
        list.sort_unstable_by(|&a, &b| {
            let (ra, rb) = (&records[a], &records[b]);
            ra.depth
                .total_cmp(&rb.depth)
                .then(ra.emission_index.cmp(&rb.emission_index))
                .then(a.cmp(&b))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(depth: f32, emission: u32, blend: BlendBucket) -> DrawRecord<'static> {
        let texture = &crate::test_support::bind_groups()[0];
        DrawRecord {
            depth,
            emission_index: emission,
            blend,
            pipeline: PipelineKind::Sprite,
            vertices: Vec::new().into(),
            indices: Vec::new().into(),
            texture,
        }
    }

    #[test]
    fn flush_order_alpha_before_additive() {
        let alpha_idx = BlendBucket::Alpha.flush_index();
        let additive_idx = BlendBucket::Additive.flush_index();
        assert!(alpha_idx < additive_idx);
    }

    #[test]
    fn raw_dst_six_lands_in_alpha() {
        assert_eq!(
            BlendBucket::from_blend_kind(BlendKind::Raw { src: 5, dst: 6 }),
            BlendBucket::Alpha
        );
        assert_eq!(
            BlendBucket::from_blend_kind(BlendKind::Raw { src: 5, dst: 2 }),
            BlendBucket::Additive
        );
    }

    #[test]
    fn partition_and_sort_orders_by_depth_then_emission() {
        let records = vec![
            record_with(20.0, 0, BlendBucket::Alpha),
            record_with(5.0, 1, BlendBucket::Alpha),
            record_with(10.0, 2, BlendBucket::Additive),
            record_with(10.0, 3, BlendBucket::Alpha),
            record_with(10.0, 4, BlendBucket::Alpha),
        ];
        let mut buckets = Default::default();
        partition_and_sort(&records, &mut buckets);
        let alpha_idx = BlendBucket::Alpha.flush_index();
        assert_eq!(buckets[alpha_idx], vec![1, 3, 4, 0]);
        let add_idx = BlendBucket::Additive.flush_index();
        assert_eq!(buckets[add_idx], vec![2]);
    }

    #[test]
    fn billboard_and_sprite_particle_interleave_by_depth() {
        let records = vec![
            record_with(-10.0, 0, BlendBucket::Alpha),
            record_with(-5.0, 1, BlendBucket::Alpha),
            record_with(-20.0, 2, BlendBucket::Alpha),
        ];
        // a stale list from a frame before is cleared, not added to
        let mut buckets: [Vec<usize>; 5] = Default::default();
        buckets[BlendBucket::Alpha.flush_index()].push(9);
        partition_and_sort(&records, &mut buckets);
        let alpha_idx = BlendBucket::Alpha.flush_index();
        assert_eq!(buckets[alpha_idx], vec![2, 0, 1]);
    }

    #[test]
    fn nan_depths_and_ties_sort_without_breaking_the_order() {
        let mut records: Vec<_> = (0..40)
            .map(|i| {
                record_with(
                    if i % 7 == 0 { f32::NAN } else { (i % 5) as f32 },
                    0,
                    BlendBucket::Alpha,
                )
            })
            .collect();
        records.push(record_with(-1.0, 0, BlendBucket::Alpha));
        let mut buckets = Default::default();
        partition_and_sort(&records, &mut buckets);
        let alpha = &buckets[BlendBucket::Alpha.flush_index()];
        assert_eq!(alpha[0], 40, "the nearest first");
        assert!(
            alpha
                .iter()
                .rev()
                .take(6)
                .all(|&i| records[i].depth.is_nan()),
            "a NaN to one end"
        );
        let depth_2: Vec<_> = alpha
            .iter()
            .copied()
            .filter(|&i| records[i].depth == 2.0)
            .collect();
        assert!(depth_2.is_sorted(), "ties in push order");
    }
}
