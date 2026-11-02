//! Opt-in tint of existing sky assets; the authored pipeline is kept verbatim for off.
use super::*;
use crate::world_materials::shadows::day::Settings;

impl Runtime {
    /// Compile once at world installation; a moving clock never recreates resources.
    pub(crate) fn configure_day(
        &mut self,
        device: &wgpu::Device,
        camera: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        settings: Settings,
    ) {
        let built = settings
            .enabled
            .then(|| {
                // A compile wgpu rejects, or one the last session closed in, leaves the
                // map's own sky (hub report #219).
                crate::day_sky_guard::build(device, || {
                    self.day_pipelines(device, camera, format, settings)
                })
            })
            .flatten();
        let Some((clock, box_pipeline, face_pipeline)) = built else {
            self.box_pipeline = self.authored_box_pipeline.clone();
            self.face_pipeline = self.authored_face_pipeline.clone();
            self.day_clock = None;
            return;
        };
        self.day_clock = Some(clock);
        self.box_pipeline = box_pipeline;
        self.face_pipeline = face_pipeline;
    }

    /// The tint's clock buffer and binding and its box and face pipelines.
    fn day_pipelines(
        &self,
        device: &wgpu::Device,
        camera: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        settings: Settings,
    ) -> (
        (wgpu::Buffer, wgpu::BindGroup),
        wgpu::RenderPipeline,
        gpu::FacePipelines,
    ) {
        use wgpu::util::DeviceExt;
        let clock_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SJK live day clock"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SJK live day clock"),
            contents: bytemuck::cast_slice(&[
                settings.hour,
                settings.minutes,
                0.,
                0.,
                0.,
                0.,
                0.,
                0.,
            ]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &clock_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let texture = gpu::texture_layout(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SJK day sky layout"),
            bind_group_layouts: &[Some(camera), Some(&texture), Some(&clock_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SJK day sky"),
            source: wgpu::ShaderSource::Wgsl(source().into()),
        });
        (
            (buffer, binding),
            gpu::box_pipeline(device, &layout, &shader, format, self.radiance),
            gpu::face_pipeline(device, &layout, &shader, format, self.radiance),
        )
    }

    /// Match the directed sun used by surface lighting, shadows and godrays.
    pub(crate) fn update_director_sun(
        &self,
        queue: &crate::frame_queue::FrameQueue,
        sun: Option<(glam::Vec3, f32)>,
    ) {
        if let Some((buffer, _)) = &self.day_clock {
            let value = sun.map_or([0.; 4], |(v, weight)| [v.x, v.y, v.z, weight]);
            queue.write_buffer(buffer, 16, bytemuck::cast_slice(&value));
        }
    }

    /// The sky alone following `values` (hour, day length) without the live sun, the
    /// twilight over `azimuth`: written only when the hour or day length changes.
    pub(crate) fn follow_day_clock(
        &self,
        queue: &crate::frame_queue::FrameQueue,
        values: [f32; 2],
        azimuth: [f32; 2],
    ) {
        if self.day_clock.is_none() || self.followed.get() == values {
            return;
        }
        self.followed.set(values);
        self.update_day_clock(queue, values, azimuth);
    }

    /// Write the same live hour/rate used by the shadow fit and volume injection, and the
    /// authored sun azimuth (unit xy) the twilight glow sits over.
    pub(crate) fn update_day_clock(
        &self,
        queue: &crate::frame_queue::FrameQueue,
        values: [f32; 2],
        azimuth: [f32; 2],
    ) {
        if let Some((buffer, _)) = &self.day_clock {
            queue.write_buffer(
                buffer,
                0,
                bytemuck::cast_slice(&[values[0], values[1], azimuth[0], azimuth[1]]),
            );
        }
    }
}

fn source() -> String {
    patched_source(include_str!("sky_stage.wgsl"))
}

/// The sky program `sky` with both of its returns tinted; a CRLF checkout's source is
/// normalised first.
fn patched_source(sky: &str) -> String {
    let shader = crate::wgsl_source::lf(sky).replace(
        "return texel;",
        "return vec4(day_sky(texel.rgb, normalize(input.direction)), texel.a);",
    );
    format!(
        "struct DayControls {{ clock: vec4<f32>, sun: vec4<f32> }};\n@group(2) @binding(0) var<uniform> day_controls: DayControls;\n{}\n{}",
        shader,
        crate::wgsl_source::lf(include_str!("day_sky.wgsl"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgsl_source::{crlf, lf};

    /// Both sky returns are tinted whatever line endings the checkout gave the program;
    /// on CRLF the discard path used to stay untinted without an error (#67).
    #[test]
    fn day_sky_tints_lf_and_crlf_programs() {
        let sky = include_str!("sky_stage.wgsl");
        let unix = patched_source(&lf(sky));
        assert_eq!(unix.matches("vec4(day_sky(texel.rgb").count(), 2);
        assert!(!unix.contains("return texel;"));
        assert!(!unix.contains('\r'));
        assert_eq!(patched_source(&crlf(sky)), unix);
        assert_eq!(source(), unix);
    }
}
