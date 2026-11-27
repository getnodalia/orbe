// SPDX-License-Identifier: MIT
//! Orbe's wgpu backend for the 2D renderer.
//!
//! [`Gpu`] draws the same [`orbe_render::DrawList`] the CPU rasteriser draws, offscreen, and gives
//! back an [`orbe_render::Image`]. The CPU rasteriser is the oracle: this backend is tested
//! against it. Blending happens in linear light on a 16-bit float target and the result is encoded
//! with the same function as the CPU path, so both agree except for the anti-aliasing of edges.
//!
//! Creating a [`Gpu`] fails with [`GpuError::NoAdapter`] when the machine has no usable graphics
//! device; callers fall back to the CPU rasteriser.

use orbe_math::Color;
use orbe_render::{build_mesh, check_size, DrawList, Image, RenderError, Vertex};
use std::fmt;

/// Most vertices one frame may use.
pub const MAX_VERTICES: usize = 4_000_000;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const SAMPLES: u32 = 4;

const SHADER: &str = r#"
struct Frame { size: vec2<f32> };
@group(0) @binding(0) var<uniform> frame: Frame;

struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = vec4<f32>(v.position.x / frame.size.x * 2.0 - 1.0, 1.0 - v.position.y / frame.size.y * 2.0, 0.0, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs_main(v: VertexOut) -> @location(0) vec4<f32> {
    return v.color;
}
"#;

/// Why the GPU could not draw.
#[derive(Debug)]
pub enum GpuError {
    /// No graphics adapter is available.
    NoAdapter,
    /// The adapter refused to create a device.
    Device(String),
    /// The image size is invalid or beyond what the device supports.
    Image(RenderError),
    /// The frame has more than [`MAX_VERTICES`] vertices.
    TooManyVertices,
    /// Reading the result back failed.
    Readback(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapter => f.write_str("no graphics adapter is available"),
            GpuError::Device(e) => write!(f, "could not create a graphics device: {e}"),
            GpuError::Image(e) => write!(f, "{e}"),
            GpuError::TooManyVertices => f.write_str("the frame has too many vertices"),
            GpuError::Readback(e) => write!(f, "could not read the image back: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<RenderError> for GpuError {
    fn from(e: RenderError) -> Self {
        GpuError::Image(e)
    }
}

/// A graphics device ready to draw 2D draw lists offscreen.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    adapter_name: String,
}

/// Converts an IEEE half float to `f32`.
fn half_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let man = f32::from(h & 0x3ff);
    match exp {
        0 => sign * man * 2f32.powi(-24),
        31 => {
            if man == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        e => sign * (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    }
}

impl Gpu {
    /// Opens the best available adapter.
    pub fn new() -> Result<Gpu, GpuError> {
        pollster::block_on(Self::open())
    }

    async fn open() -> Result<Gpu, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|_| GpuError::NoAdapter)?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("orbe 2d"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("orbe 2d"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    // Source over, producing premultiplied colour like the CPU rasteriser.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Ok(Gpu {
            device,
            queue,
            pipeline,
            adapter_name,
        })
    }

    /// The name of the adapter in use.
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Draws `list` into a new `width` x `height` image on a background of `clear`.
    pub fn render(
        &self,
        list: &DrawList,
        width: u32,
        height: u32,
        clear: Color,
    ) -> Result<Image, GpuError> {
        check_size(width, height)?;
        let max_side = self.device.limits().max_texture_dimension_2d;
        if width > max_side || height > max_side {
            return Err(GpuError::Image(RenderError::TooLarge));
        }
        let vertices = build_mesh(list, width, height);
        if vertices.len() > MAX_VERTICES {
            return Err(GpuError::TooManyVertices);
        }
        let device = &self.device;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let make_texture = |samples: u32, usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("orbe 2d target"),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR_FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let msaa = make_texture(SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolved = make_texture(
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa_view = msaa.create_view(&wgpu::TextureViewDescriptor::default());
        let resolved_view = resolved.create_view(&wgpu::TextureViewDescriptor::default());

        let frame: [f32; 4] = [width as f32, height as f32, 0.0, 0.0];
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("orbe 2d frame"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bytes = Vec::with_capacity(16);
        for v in frame {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        self.queue.write_buffer(&uniform, 0, &bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("orbe 2d frame"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let vertex_buffer = if vertices.is_empty() {
            None
        } else {
            let mut data = Vec::with_capacity(vertices.len() * std::mem::size_of::<Vertex>());
            for v in &vertices {
                for f in v.position.iter().chain(v.color.iter()) {
                    data.extend_from_slice(&f.to_le_bytes());
                }
            }
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("orbe 2d vertices"),
                size: data.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&buffer, 0, &data);
            Some(buffer)
        };

        let clear_a = f64::from(if clear.a.is_finite() {
            clear.a.clamp(0.0, 1.0)
        } else {
            0.0
        });
        let premul = |v: f32| {
            if v.is_finite() {
                f64::from(v.clamp(0.0, 1.0)) * clear_a
            } else {
                0.0
            }
        };
        let row_bytes = width as usize * 8;
        let padded = row_bytes.div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("orbe 2d readback"),
            size: (padded * height as usize) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("orbe 2d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&resolved_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: premul(clear.r),
                            g: premul(clear.g),
                            b: premul(clear.b),
                            a: clear_a,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(buffer) = &vertex_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &resolved,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let mut pixels: Vec<[f32; 4]> = Vec::with_capacity(width as usize * height as usize);
        for row in data.chunks(padded).take(height as usize) {
            for texel in row.get(..row_bytes).unwrap_or(&[]).chunks_exact(8) {
                let mut c = [0.0f32; 4];
                for (out, h) in c.iter_mut().zip(texel.chunks_exact(2)) {
                    if let [lo, hi] = h {
                        *out = half_to_f32(u16::from_le_bytes([*lo, *hi]));
                    }
                }
                pixels.push(c);
            }
        }
        drop(data);
        readback.unmap();
        Ok(Image::from_premultiplied_linear(width, height, &pixels)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_decode() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0x3800), 0.5);
        assert_eq!(half_to_f32(0xc000), -2.0);
        assert!(half_to_f32(0x7c00).is_infinite());
        assert!(half_to_f32(0x7e00).is_nan());
        assert!(half_to_f32(0x0001) > 0.0);
    }
}
