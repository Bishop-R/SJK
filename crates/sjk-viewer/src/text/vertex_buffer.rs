//! A text vertex buffer that grows to the frame's text, up to [`MAX_TEXT_VERTICES`].
//!
//! Most frames fit the first [`INITIAL_VERTICES`]; an outlined chat (`cg_chatBoxOutline`
//! draws nine quads a letter instead of two) or a full page of text can need several
//! times that, so the buffer is made again, twice as large, the first frame that needs
//! it, rather than every buffer being sized for the worst case.

use super::{MAX_TEXT_VERTICES, TextVertex};

/// Vertices a buffer holds before it first grows (the cap all text had until 03/11/2026).
pub(crate) const INITIAL_VERTICES: usize = 32_768;

/// One batch's GPU vertices, grown on demand.
pub(crate) struct TextVertexBuffer {
    label: &'static str,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl TextVertexBuffer {
    pub(crate) fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            label,
            buffer: create(device, label, INITIAL_VERTICES),
            capacity: INITIAL_VERTICES,
        }
    }

    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Copy `vertices` (at most [`MAX_TEXT_VERTICES`], which the text appenders keep to)
    /// to the GPU, first growing the buffer if they do not fit; returns how many it holds.
    pub(crate) fn write(
        &mut self,
        device: &wgpu::Device,
        queue: &crate::frame_queue::FrameQueue,
        vertices: &[TextVertex],
    ) -> u32 {
        let count = vertices.len().min(MAX_TEXT_VERTICES);
        if count > self.capacity {
            self.capacity = grown(self.capacity, count);
            self.buffer = create(device, self.label, self.capacity);
        }
        if count != 0 {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&vertices[..count]));
        }
        u32::try_from(count).unwrap_or(0)
    }
}

/// The capacity, doubled from `capacity`, that holds `needed`, at most the cap.
fn grown(capacity: usize, needed: usize) -> usize {
    let mut capacity = capacity.max(1);
    while capacity < needed {
        capacity *= 2;
    }
    capacity.min(MAX_TEXT_VERTICES)
}

fn create(device: &wgpu::Device, label: &'static str, vertices: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (vertices * std::mem::size_of::<TextVertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_doubles_to_fit_and_stops_at_the_cap() {
        assert_eq!(
            grown(INITIAL_VERTICES, INITIAL_VERTICES + 6),
            2 * INITIAL_VERTICES
        );
        assert_eq!(
            grown(INITIAL_VERTICES, 3 * INITIAL_VERTICES),
            4 * INITIAL_VERTICES
        );
        assert_eq!(grown(INITIAL_VERTICES, usize::MAX / 4), MAX_TEXT_VERTICES);
    }
}
