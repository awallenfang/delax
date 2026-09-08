use std::marker::PhantomData;
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
    pub levels_dry: [[f32; 4]; 16],
    pub levels_wet: [[f32; 4]; 16],
    pub primary_col: [f32; 4],
    pub secondary_col: [f32; 4],
    pub params: [f32; 4],
}

