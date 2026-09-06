use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SpectrumUniforms {
    pub levels: [f32; 32],
    pub primary_col: [f32; 4],
}
