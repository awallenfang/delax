use crate::slint_ui::elements::EditorChannel;
use crate::slint_ui::frames::{EDITOR_VIS_SIZE, UI_BUFFER_SIZE};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SpectrumUniforms {
    pub levels: [f32; 32],
    pub primary_col: [f32; 4],
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct DoubleBufferUniforms {
    pub levels_dry: [[f32; 4]; UI_BUFFER_SIZE / 4],
    pub levels_wet: [[f32; 4]; UI_BUFFER_SIZE / 4],
    pub primary_col: [f32; 4],
    pub secondary_col: [f32; 4],
    pub params: [f32; 4],
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct BufferUniforms {
    pub levels: [[f32; 4]; EDITOR_VIS_SIZE / 4],
    pub col: [f32; 4],
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct DecayUniforms {
    pub feedback: [f32; 2],
    pub time_s: [f32; 2],
    pub flags: [f32; 4],
    pub color_primary: [f32; 4],
    pub color_secondary: [f32; 4],
    pub grid: [f32; 4],
}

pub fn spectrum_uniform(levels: [f32; 32]) -> SpectrumUniforms {
    SpectrumUniforms {
        levels,
        primary_col: [1.0, 0.6724, 0.003, 1.0],
    }
}

pub fn buffer_uniform(
    dry: &[f32; UI_BUFFER_SIZE],
    wet: &[f32; UI_BUFFER_SIZE],
    wetness: f32,
) -> DoubleBufferUniforms {
    let mut levels_dry = [[0.0f32; 4]; UI_BUFFER_SIZE / 4];
    let mut levels_wet = [[0.0f32; 4]; UI_BUFFER_SIZE / 4];
    for i in 0..UI_BUFFER_SIZE / 4 {
        levels_dry[i] = [dry[i * 4], dry[i * 4 + 1], dry[i * 4 + 2], dry[i * 4 + 3]];
        levels_wet[i] = [wet[i * 4], wet[i * 4 + 1], wet[i * 4 + 2], wet[i * 4 + 3]];
    }
    DoubleBufferUniforms {
        levels_dry,
        levels_wet,
        primary_col: [1.0, 214. / 255., 10. / 256., 0.5],
        secondary_col: [0.0, 143. / 256., 186. / 256., 0.5],
        params: [wetness, 0., 0., 0.],
    }
}

pub fn editor_buffer_uniform(
    src: &[f32; EDITOR_VIS_SIZE],
    channel: EditorChannel,
) -> BufferUniforms {
    let col = match channel {
        EditorChannel::Left => [1.0, 214. / 255., 10. / 256., 0.5],
        EditorChannel::Right => [0.0, 143. / 255., 186. / 256., 0.5],
    };
    let mut levels = [[0.0f32; 4]; EDITOR_VIS_SIZE / 4];
    for i in 0..EDITOR_VIS_SIZE / 4 {
        levels[i] = [src[i * 4], src[i * 4 + 1], src[i * 4 + 2], src[i * 4 + 3]];
    }
    BufferUniforms {
        levels,
        col,
        params: [0.5, 0., 0., 0.],
    }
}

pub fn decay_uniform(
    feedback: [f32; 2],
    time_s: [f32; 2],
    bpm: f32,
    flags: [f32; 4],
) -> DecayUniforms {
    DecayUniforms {
        feedback,
        time_s,
        flags,
        color_primary: [1.0, 214. / 255., 10. / 255., 0.5],
        color_secondary: [0.0, 143. / 255., 186. / 255., 0.5],
        grid: [240.0 / bpm.max(1.0), 0.0, 0.0, 0.0],
    }
}
