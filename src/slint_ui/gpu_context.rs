use std::sync::{Arc, OnceLock};

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

static GPU_CONTEXT: OnceLock<Arc<GpuContext>> = OnceLock::new();

impl GpuContext {
    fn init_blocking() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
        });

        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions::default(),
        ))
        .map_err(|e| format!("wgpu: no suitable Vulkan adapter: {e:?}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::IMMEDIATES,
                memory_hints: wgpu::MemoryHints::default(),
                required_limits: wgpu::Limits {
                    max_immediate_size: 256,
                    ..wgpu::Limits::default()
                },
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            },
        ))
        .map_err(|e| format!("wgpu: request_device failed: {e:?}"))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    pub fn ensure_initialized() -> Result<&'static Arc<Self>, String> {
        if let Some(ctx) = GPU_CONTEXT.get() {
            return Ok(ctx);
        }
        let ctx = Self::init_blocking().map(Arc::new)?;
        Ok(GPU_CONTEXT.get_or_init(|| ctx))
    }

    pub fn get() -> Option<&'static Arc<Self>> {
        GPU_CONTEXT.get()
    }

    pub fn device(&self) -> wgpu::Device {
        self.device.clone()
    }

    pub fn queue(&self) -> wgpu::Queue {
        self.queue.clone()
    }
}
