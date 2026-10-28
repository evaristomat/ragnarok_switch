//! One vertex and one index buffer a pass: meshes appended in draw order, neighbours drawn with the same state
//! joined into one draw, the lists and GPU buffers kept across frames (grown, never rebuilt a frame).

use std::ops::Range;

/// A quad's two triangles over its corners 0 to 3, clockwise from the top left (as
/// [`build_clip_quad`](crate::sprite::build_clip_quad) makes them), shared rather than allocated.
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 0, 2, 3];

/// A quad's two triangles over corners laid out top-left, top-right, bottom-left, bottom-right (UI quads, billboards,
/// STR layers).
pub const Z_QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 1, 3];

/// `indices` borrowed as the shared slice when they are one of the quad patterns above, else copied.
pub fn shared_indices(indices: &[u32]) -> std::borrow::Cow<'static, [u32]> {
    [QUAD_INDICES, Z_QUAD_INDICES]
        .into_iter()
        .find(|q| *q == indices)
        .map_or_else(|| indices.to_vec().into(), std::borrow::Cow::Borrowed)
}

/// A mesh's vertices: a quad's four kept inline (no allocation a quad, the most common draw), or any number.
#[derive(Clone, Debug)]
pub enum Vertices<V> {
    Quad([V; 4]),
    Mesh(Vec<V>),
}

impl<V> Vertices<V> {
    /// The same shape over other vertices: a quad stays inline.
    pub fn map<U>(&self, mut f: impl FnMut(&V) -> U) -> Vertices<U> {
        match self {
            Self::Quad(q) => Vertices::Quad(std::array::from_fn(|i| f(&q[i]))),
            Self::Mesh(m) => Vertices::Mesh(m.iter().map(f).collect()),
        }
    }
}

impl<V> std::ops::Deref for Vertices<V> {
    type Target = [V];
    fn deref(&self) -> &[V] {
        match self {
            Self::Quad(q) => q,
            Self::Mesh(m) => m,
        }
    }
}

impl<V> std::ops::DerefMut for Vertices<V> {
    fn deref_mut(&mut self) -> &mut [V] {
        match self {
            Self::Quad(q) => q,
            Self::Mesh(m) => m,
        }
    }
}

impl<V> From<[V; 4]> for Vertices<V> {
    fn from(quad: [V; 4]) -> Self {
        Self::Quad(quad)
    }
}

impl<V> From<Vec<V>> for Vertices<V> {
    fn from(mesh: Vec<V>) -> Self {
        Self::Mesh(mesh)
    }
}

impl<V> Default for Vertices<V> {
    fn default() -> Self {
        Self::Mesh(Vec::new())
    }
}

impl<'s, V> IntoIterator for &'s Vertices<V> {
    type Item = &'s V;
    type IntoIter = std::slice::Iter<'s, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'s, V> IntoIterator for &'s mut Vertices<V> {
    type Item = &'s mut V;
    type IntoIter = std::slice::IterMut<'s, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// One draw: a run of meshes sharing draw state, named by the item its first mesh came from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub first: usize,
    pub indices: Range<u32>,
}

pub(crate) struct MeshBatcher<V: bytemuck::Pod> {
    label: &'static str,
    vertices: Vec<V>,
    indices: Vec<u32>,
    runs: Vec<Run>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl<V: bytemuck::Pod> MeshBatcher<V> {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        vertices: usize,
        indices: usize,
    ) -> Self {
        Self {
            label,
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
            runs: Vec::new(),
            vertex_buffer: Self::vertex_buffer(device, label, vertices),
            index_buffer: Self::index_buffer(device, label, indices),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.runs.clear();
    }

    /// Appends item `item`'s mesh, its indices moved past the vertices already there. It joins the last run when
    /// `joins(first)` says it draws like that run's first item, so the run's triangles keep their order.
    pub fn push(
        &mut self,
        item: usize,
        vertices: &[V],
        indices: &[u32],
        joins: impl FnOnce(usize) -> bool,
    ) {
        // joined draws: a stray index would shift every triangle after it
        debug_assert_eq!(indices.len() % 3, 0, "a mesh of whole triangles");
        debug_assert!(
            indices.iter().all(|&i| (i as usize) < vertices.len()),
            "a mesh indexing only its own vertices"
        );
        let vertex_offset = u32::try_from(self.vertices.len()).expect("vertex count fits u32");
        let start = self.indices.len() as u32;
        self.vertices.extend_from_slice(vertices);
        self.indices
            .extend(indices.iter().map(|i| i + vertex_offset));
        let end = u32::try_from(self.indices.len()).expect("index count fits u32");
        match self.runs.last_mut() {
            Some(run) if joins(run.first) => run.indices.end = end,
            _ => self.runs.push(Run {
                first: item,
                indices: start..end,
            }),
        }
    }

    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Writes the meshes to the GPU buffers, growing them first when too small. The writes land when the queue is
    /// next submitted, so a batcher serves one pass a submit: a second upload before it would overwrite the first.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.vertices.is_empty() {
            return;
        }
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&self.vertices);
        if vertex_bytes.len() as u64 > self.vertex_buffer.size() {
            self.vertex_buffer =
                Self::vertex_buffer(device, self.label, self.vertices.len().next_power_of_two());
        }
        queue.write_buffer(&self.vertex_buffer, 0, vertex_bytes);
        let index_bytes: &[u8] = bytemuck::cast_slice(&self.indices);
        if index_bytes.len() as u64 > self.index_buffer.size() {
            self.index_buffer =
                Self::index_buffer(device, self.label, self.indices.len().next_power_of_two());
        }
        queue.write_buffer(&self.index_buffer, 0, index_bytes);
    }

    /// Binds the uploaded buffers to `pass`, ready for each run's `draw_indexed`.
    pub fn bind(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    }

    fn vertex_buffer(device: &wgpu::Device, label: &str, count: usize) -> wgpu::Buffer {
        Self::buffer(
            device,
            label,
            count * size_of::<V>(),
            wgpu::BufferUsages::VERTEX,
        )
    }

    fn index_buffer(device: &wgpu::Device, label: &str, count: usize) -> wgpu::Buffer {
        Self::buffer(
            device,
            label,
            count * size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        )
    }

    fn buffer(
        device: &wgpu::Device,
        label: &str,
        bytes: usize,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes.max(wgpu::COPY_BUFFER_ALIGNMENT as usize) as u64,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    #[cfg(test)]
    pub fn vertices(&self) -> &[V] {
        &self.vertices
    }

    #[cfg(test)]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::test_support::device;

    #[test]
    fn neighbours_that_join_share_a_run_and_indices_follow_earlier_vertices() {
        let (device, _) = device();
        let mut b = MeshBatcher::<[f32; 4]>::new(&device, "test", 4, 6);
        let quad = [[0.0; 4]; 4];
        let quad_indices = [0, 1, 2, 0, 2, 3];
        let state = [0, 0, 1];
        for (item, s) in state.iter().enumerate() {
            b.push(item, &quad, &quad_indices, |first| state[first] == *s);
        }
        assert_eq!(
            b.runs(),
            [
                Run {
                    first: 0,
                    indices: 0..12
                },
                Run {
                    first: 2,
                    indices: 12..18
                }
            ]
        );
        assert_eq!(&b.indices()[6..12], &[4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn a_mapped_quad_stays_inline_and_known_index_patterns_are_borrowed() {
        let quad = Vertices::from([1u8, 2, 3, 4]).map(|v| u32::from(*v) * 10);
        assert!(matches!(quad, Vertices::Quad([10, 20, 30, 40])));
        let mesh = Vertices::from(vec![1u8, 2, 3]).map(|v| *v);
        assert!(matches!(mesh, Vertices::Mesh(ref m) if m == &[1, 2, 3]));
        for pattern in [QUAD_INDICES, Z_QUAD_INDICES] {
            assert!(matches!(shared_indices(pattern), Cow::Borrowed(p) if p == pattern));
        }
        assert!(matches!(shared_indices(&[0, 1, 2]), Cow::Owned(ref v) if v == &[0, 1, 2]));
    }

    #[test]
    fn a_clear_starts_the_lists_over_and_uploads_grow_the_buffers() {
        let (device, queue) = device();
        let mut b = MeshBatcher::<[f32; 4]>::new(&device, "test", 1, 3);
        let tri = [[0.0; 4]; 3];
        for item in 0..10 {
            b.push(item, &tri, &[0, 1, 2], |_| true);
        }
        b.upload(&device, &queue);
        assert!(b.vertex_buffer.size() >= 30 * 16);
        assert!(b.index_buffer.size() >= 30 * 4);
        b.clear();
        assert!(b.is_empty());
        b.push(0, &tri, &[0, 1, 2], |_| true);
        assert_eq!(b.indices(), [0, 1, 2]);
        assert_eq!(b.vertices().len(), 3);
    }
}
