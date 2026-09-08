use std::collections::HashMap;
use wgpu::RenderPassDescriptor;

use crate::slint_ui::gpu_context::GpuContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementId {
    Spectrum,
    Decay,
    Buffer,
    Peak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementSpec {
    pub shader: &'static str,
    pub uniform_size: u32,
}

pub struct WGPURenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    uniform_size: u32,
}

impl WGPURenderer {
    pub fn new(ctx: &GpuContext, spec: &ElementSpec, w: u32, h: u32) -> Self {
        let device = ctx.device();
        let queue = ctx.queue();
        Self::with_device_queue(device, queue, spec, w, h)
    }

    pub fn with_device_queue(
        device: wgpu::Device,
        queue: wgpu::Queue,
        spec: &ElementSpec,
        w: u32,
        h: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(spec.shader)),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: spec.uniform_size.max(16) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            spec.uniform_size.max(16) as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let texture = Self::create_texture(&device, w.max(1), h.max(1));

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            texture,
            uniform_size: spec.uniform_size,
        }
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn render(&mut self, w: u32, h: u32, uniforms: &[u8]) {
        debug_assert!(
            uniforms.len() % 4 == 0,
            "WGSL uniform data must be 4-byte aligned"
        );
        if uniforms.len() as u32 > self.uniform_size {
            eprintln!(
                "WGPURenderer: uniform data ({} B) exceeds element uniform size ({} B)",
                uniforms.len(),
                self.uniform_size
            );
            return;
        }
        self.resize(w, h);
        self.queue.write_buffer(&self.uniform_buffer, 0, uniforms);

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut renderpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderpass.set_pipeline(&self.pipeline);
            renderpass.set_bind_group(0, &self.bind_group, &[]);
            renderpass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn to_image(&self) -> Option<slint::Image> {
        use std::sync::atomic::{AtomicBool, Ordering};

        let size = self.texture.size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        const BYTES_PER_PIXEL: u32 = 4;
        let unpadded_row = w * BYTES_PER_PIXEL;
        let padded_row = unpadded_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (padded_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let mapped = std::sync::Arc::new(AtomicBool::new(false));
        let mapped_cb = mapped.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |_| {
                mapped_cb.store(true, Ordering::SeqCst);
            });
        if self
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .is_err()
            || !mapped.load(Ordering::SeqCst)
        {
            return None;
        }

        let data = buffer.slice(..).get_mapped_range().ok()?;
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for row in 0..h {
            let start = (row * padded_row) as usize;
            let row_bytes = &data[start..start + unpadded_row as usize];
            for px in row_bytes.chunks_exact(4) {
                pixels.push(slint::Rgba8Pixel {
                    r: px[0],
                    g: px[1],
                    b: px[2],
                    a: px[3],
                });
            }
        }
        drop(data);
        buffer.unmap();

        let mut pixbuf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
        pixbuf.make_mut_slice().copy_from_slice(&pixels);
        Some(slint::Image::from_rgba8(pixbuf))
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        let w = w.max(1);
        let h = h.max(1);
        let size = self.texture.size();
        if size.width != w || size.height != h {
            self.texture = Self::create_texture(&self.device, w, h);
        }
    }

    pub fn size(&self) -> (u32, u32) {
        let s = self.texture.size();
        (s.width, s.height)
    }
}

pub struct WgpuRegistry {
    ctx: std::sync::Arc<GpuContext>,
    specs: HashMap<ElementId, ElementSpec>,
    renderers: HashMap<ElementId, WGPURenderer>,
    render_cache: HashMap<ElementId, Vec<u8>>
}

impl WgpuRegistry {
    pub fn new(ctx: std::sync::Arc<GpuContext>) -> Self {
        Self {
            ctx,
            specs: HashMap::new(),
            renderers: HashMap::new(),
            render_cache: HashMap::new()
        }
    }

    pub fn register(&mut self, id: ElementId, spec: ElementSpec) {
        if self.specs.get(&id) == Some(&spec) {
            return;
        }
        self.specs.insert(id, spec);
        let renderer = match self.renderers.remove(&id) {
            // Rebuild on spec change, keeping the previous size.
            Some(old) => {
                let (w, h) = old.size();
                WGPURenderer::new(&self.ctx, &spec, w, h)
            }
            // First registration: start at the element's default size.
            None => {
                let (w, h) = id.default_size();
                WGPURenderer::new(&self.ctx, &spec, w, h)
            }
        };
        self.renderers.insert(id, renderer);
    }

    pub fn render_to_image(
        &mut self,
        id: ElementId,
        w: u32,
        h: u32,
        uniforms: &[u8],
    ) -> Option<slint::Image> {
        if let Some(cached_uniform) = self.render_cache.get(&id) {
            if cached_uniform.len() == uniforms.len() {
                if cached_uniform == uniforms {
                    return None;
                }
            }
        } else {
            self.render_cache.insert(id, uniforms.to_vec());
        }
        self.renderers.get_mut(&id)?.render(w, h, uniforms);
        self.renderers.get(&id)?.to_image()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slint_ui::elements::{SPECTRUM_SHADER, DECAY_SHADER};
    use crate::slint_ui::uniforms::{SpectrumUniforms, DecayUniforms};

    #[test]
    fn render_yellow_square_to_slint_image() {
        let ctx = GpuContext::ensure_initialized().expect("headless Vulkan device");
        let spec = ElementSpec {
            shader: SPECTRUM_SHADER,
            uniform_size: std::mem::size_of::<SpectrumUniforms>() as u32,
        };

        let mut renderer = WGPURenderer::with_device_queue(
            ctx.device.clone(),
            ctx.queue.clone(),
            &spec,
            100,
            40,
        );

        // Flat zero levels: everything is background, which stays transparent
        // so the image can overlay Slint UI.
        let uniforms = SpectrumUniforms {
            levels: [0.0; 32],
            primary_col: [1.0, 1.0, 0.0, 1.0],
        };
        renderer.render(100, 40, bytemuck::bytes_of(&uniforms));
        let img = renderer.to_image().expect("render + readback should succeed");

        let pixel_buffer = img.to_rgba8().expect("expected a shared (CPU) image back");

        let data = pixel_buffer.as_bytes();
        assert_eq!(data.len(), 100 * 40 * 4, "RGBA8 byte size");
        assert_eq!(&data[0..4], &[255, 255, 0, 0], "background is transparent");

        // Full levels: interior of bar 0 (x=1, mid height) is opaque yellow.
        let uniforms = SpectrumUniforms {
            levels: [1.0; 32],
            primary_col: [1.0, 1.0, 0.0, 1.0],
        };
        renderer.render(100, 40, bytemuck::bytes_of(&uniforms));
        let img = renderer.to_image().expect("render + readback should succeed");
        let pixel_buffer = img.to_rgba8().expect("expected a shared (CPU) image back");
        let data = pixel_buffer.as_bytes();
        let bar_idx = (20 * 100 + 1) * 4;
        assert_eq!(
            &data[bar_idx..bar_idx + 4],
            &[255, 255, 0, 255],
            "lit bar is opaque yellow"
        );
    }

    #[test]
    fn registry_renders_registered_element_to_image() {
        let ctx = GpuContext::ensure_initialized().expect("headless Vulkan device");
        let spec = ElementSpec {
            shader: SPECTRUM_SHADER,
            uniform_size: std::mem::size_of::<SpectrumUniforms>() as u32,
        };

        let mut registry = WgpuRegistry::new(ctx.clone());
        registry.register(ElementId::Spectrum, spec);
        // Re-registering the same spec must not rebuild the pipeline.
        registry.register(ElementId::Spectrum, spec);

        let uniforms = SpectrumUniforms {
            levels: [0.0; 32],
            primary_col: [0.0, 1.0, 0.0, 1.0],
        };
        let img = registry
            .render_to_image(ElementId::Spectrum, 100, 40, bytemuck::bytes_of(&uniforms))
            .expect("registry render + readback should succeed");

        let pixel_buffer = img.to_rgba8().unwrap();
        assert_eq!((pixel_buffer.width(), pixel_buffer.height()), (100, 40));
        assert_eq!(&pixel_buffer.as_bytes()[0..4], &[0, 255, 0, 0], "green pixel");
    }

    #[test]
    fn decay_shader_renders_feedback_line() {
        let ctx = GpuContext::ensure_initialized().expect("headless Vulkan device");
        // Building the pipeline validates the WGSL layout against the uniform size.
        let spec = ElementSpec {
            shader: DECAY_SHADER,
            uniform_size: std::mem::size_of::<DecayUniforms>() as u32,
        };
        let uniforms = DecayUniforms {
            feedback: [0.9, 0.6],
            time_s: [0.5, 0.5],
            flags: [1.0, 0.0, 0.0, 0.0],
            color_primary: [1.0, 0.839, 0.039, 1.0],
            color_secondary: [0.0, 0.561, 0.729, 1.0],
            grid: [2.0, 0.0, 0.0, 0.0],
        };

        let mut registry = WgpuRegistry::new(ctx.clone());
        registry.register(ElementId::Decay, spec);
        let img = registry
            .render_to_image(ElementId::Decay, 110, 60, bytemuck::bytes_of(&uniforms))
            .expect("decay render + readback should succeed");

        let pixel_buffer = img.to_rgba8().unwrap();
        assert_eq!((pixel_buffer.width(), pixel_buffer.height()), (110, 60));
        // The decay line is drawn: some pixel must have non-zero alpha.
        let bytes = pixel_buffer.as_bytes();
        assert!(
            bytes.chunks_exact(4).any(|px| px[3] > 0),
            "decay bars should produce visible pixels"
        );
    }

    #[test]
    fn decay_shader_ping_pong_with_independent_times_renders() {
        let ctx = GpuContext::ensure_initialized().expect("headless Vulkan device");
        let spec = ElementSpec {
            shader: DECAY_SHADER,
            uniform_size: std::mem::size_of::<DecayUniforms>() as u32,
        };
        let uniforms = DecayUniforms {
            feedback: [0.9, 0.7],
            // Left/right times differ; the double step alone exceeds the
            // visible window, which must not suppress the first pair.
            time_s: [0.6, 0.3],
            flags: [1.0, 1.0, 0.0, 0.0], // is_stereo + is_ping_pong
            color_primary: [1.0, 0.839, 0.039, 1.0],
            color_secondary: [0.0, 0.561, 0.729, 1.0],
            grid: [2.0, 0.0, 0.0, 0.0],
        };

        let mut registry = WgpuRegistry::new(ctx.clone());
        registry.register(ElementId::Decay, spec);
        let img = registry
            .render_to_image(ElementId::Decay, 110, 60, bytemuck::bytes_of(&uniforms))
            .expect("decay ping-pong render + readback should succeed");

        let pixel_buffer = img.to_rgba8().unwrap();
        assert_eq!((pixel_buffer.width(), pixel_buffer.height()), (110, 60));
        let bytes = pixel_buffer.as_bytes();
        assert!(
            bytes.chunks_exact(4).any(|px| px[3] > 0),
            "ping-pong pairs should produce visible pixels"
        );
    }

    #[test]
    fn decay_shader_tempo_grid_renders() {
        let ctx = GpuContext::ensure_initialized().expect("headless Vulkan device");
        let spec = ElementSpec {
            shader: DECAY_SHADER,
            uniform_size: std::mem::size_of::<DecayUniforms>() as u32,
        };
        let uniforms = DecayUniforms {
            feedback: [0.9, 0.6],
            time_s: [0.25, 0.25],
            // Mono + left tempo-bound: bar (bar_s) lines top and bottom.
            flags: [0.0, 0.0, 1.0, 0.0], // is_stereo=0, is_ping_pong=0, bpm_bound_l=1
            color_primary: [1.0, 0.839, 0.039, 1.0],
            color_secondary: [0.0, 0.561, 0.729, 1.0],
            grid: [2.0, 0.0, 0.0, 0.0], // 120 bpm -> one bar every 2 s
        };

        let mut registry = WgpuRegistry::new(ctx.clone());
        registry.register(ElementId::Decay, spec);
        let img = registry
            .render_to_image(ElementId::Decay, 110, 60, bytemuck::bytes_of(&uniforms))
            .expect("tempo-grid decay render + readback should succeed");

        let pixel_buffer = img.to_rgba8().unwrap();
        assert_eq!((pixel_buffer.width(), pixel_buffer.height()), (110, 60));
        let bytes = pixel_buffer.as_bytes();
        assert!(
            bytes.chunks_exact(4).any(|px| px[3] > 0),
            "tempo-bound grid + decay bars should produce visible pixels"
        );
    }
}