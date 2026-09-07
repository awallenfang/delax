use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SpectrumUniforms {
    pub levels: [f32; 32],
    pub primary_col: [f32; 4],
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct BufferUniforms {
    pub levels_dry: [f32; 128],
    pub levels_wet: [f32; 128],
    pub primary_col: [f32; 4],
    pub secondary_col: [f32; 4],
}

