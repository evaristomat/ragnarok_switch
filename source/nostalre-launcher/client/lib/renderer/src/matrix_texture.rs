//! Matrices for vertex shaders (animated RSM nodes, GR2 bones) as an RGBA32F texture rather than a storage
//! buffer: Adreno's Vulkan driver returns wrong values for storage buffer reads in the vertex stage, which threw
//! those models across the screen (Retroid Pocket 6). Four texels per matrix, its columns, ROW matrices per row;
//! the shaders read them with textureLoad.

/// Matrices per texture row. model_animated.wgsl and gr2_model.wgsl hard-code it.
pub const ROW: usize = 2048;

pub struct MatrixTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl MatrixTexture {
    /// Room for `count` matrices (at least one), identity until written.
    pub fn new(device: &wgpu::Device, label: &str, count: usize) -> Self {
        let (width, height) = Self::size(count);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view }
    }

    /// Texture size in texels for `count` matrices: whole rows of ROW (fewer when they fit in one).
    pub fn size(count: usize) -> (u32, u32) {
        let per_row = ROW.min(count.max(1));
        ((per_row * 4) as u32, count.max(1).div_ceil(per_row) as u32)
    }

    /// Matrices the texture holds, the padding of its last row included.
    pub fn capacity(&self) -> usize {
        let size = self.texture.size();
        (size.width / 4 * size.height) as usize
    }

    /// Writes `matrices` from the first slot; the rest of the texture keeps what it had.
    pub fn write(&self, queue: &wgpu::Queue, matrices: &[[f32; 16]]) {
        let per_row = (self.texture.size().width / 4) as usize;
        let count = matrices.len().min(self.capacity());
        let full_rows = count / per_row;
        // the whole rows in one copy, then what is left of the last one
        self.write_rows(queue, 0, per_row, &matrices[..full_rows * per_row]);
        self.write_rows(
            queue,
            full_rows,
            count % per_row,
            &matrices[full_rows * per_row..count],
        );
    }

    /// `matrices` as rows of `width` matrices from row `row`.
    fn write_rows(&self, queue: &wgpu::Queue, row: usize, width: usize, matrices: &[[f32; 16]]) {
        if width == 0 || matrices.is_empty() {
            return;
        }
        let height = matrices.len() / width;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: row as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(matrices),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((width * 64) as u32),
                rows_per_image: Some(height as u32),
            },
            wgpu::Extent3d {
                width: (width * 4) as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The bind group layout entry of a vertex shader's matrix texture.
    pub fn layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MatrixTexture;

    #[test]
    fn whole_rows_hold_every_matrix() {
        assert_eq!(MatrixTexture::size(0), (4, 1));
        assert_eq!(MatrixTexture::size(3), (12, 1));
        assert_eq!(MatrixTexture::size(747), (2988, 1));
        assert_eq!(MatrixTexture::size(2049), (8192, 2));
    }
}
