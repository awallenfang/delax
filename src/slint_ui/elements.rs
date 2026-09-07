pub use crate::slint_ui::renderer::{ElementId, ElementSpec};
use crate::slint_ui::uniforms::{BufferUniforms, SpectrumUniforms};

pub const SPECTRUM_SHADER: &str = include_str!("ui/shaders/spectrum.wgsl");

impl ElementId {
    pub const ALL: &'static [ElementId] =
        &[ElementId::Spectrum, ElementId::Decay, ElementId::Buffer, ElementId::Peak];

    pub fn spec(self) -> Option<ElementSpec> {
        match self {
            ElementId::Spectrum => Some(ElementSpec {
                shader: SPECTRUM_SHADER,
                uniform_size: std::mem::size_of::<SpectrumUniforms>() as u32,
            }),
            ElementId::Buffer => Some(ElementSpec {
                shader: SPECTRUM_SHADER,
                uniform_size: std::mem::size_of::<BufferUniforms>() as u32,
            }),
            _ => None,
        }
    }

    pub fn default_size(self) -> (u32, u32) {
        match self {
            ElementId::Spectrum => (100, 40),
            _ => (100, 40),
        }
    }
}

pub trait GpuImageSink {
    fn set_element_image(&self, element: ElementId, image: slint::Image);
}

pub trait GpuElementData: Send + Sync {
    fn element_uniform(&self, element: ElementId) -> Option<Vec<u8>>;
}