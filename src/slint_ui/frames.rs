use crate::delay_engine::jump_builder::{Jump, JumpSegment, Portal};
use crate::slint_ui::channels::{Channels, Heads};
use crate::slint_ui::transport::FRAME_STORE;

pub const UI_BUFFER_SIZE: usize = 128;
pub const EDITOR_VIS_SIZE: usize = UI_BUFFER_SIZE * 4;
pub const WAVE_DECIM: u32 = 256;
pub const SPEC_DECIM: u32 = 8;
pub const EDITOR_CHUNK_SAMPLES: usize = 64;
pub const SPECTRUM_RAW_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferChannel {
    Left,
    Right,
}

impl BufferChannel {
    pub fn from_i32(v: i32) -> Self {
        if v == 0 { Self::Left } else { Self::Right }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrame {
    pub meters_in: Channels<f32>,
    pub meters_out: Channels<f32>,
    pub heads: Channels<Heads>,
    pub feedback: Channels<f32>,
    pub decay: Channels<f32>,
    pub bpm: f32,
    pub clamped: Channels<bool>,
    pub active_len: Channels<usize>,
}

impl Default for UiFrame {
    fn default() -> Self {
        Self {
            meters_in: Channels::default(),
            meters_out: Channels::default(),
            heads: Channels::default(),
            feedback: Channels::default(),
            decay: Channels::default(),
            bpm: 120.,
            clamped: Channels::default(),
            active_len: Channels::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct JumpChannelState {
    pub read: Vec<Jump>,
    pub write: Vec<Jump>,
    pub read_segments: Vec<JumpSegment>,
    pub write_segments: Vec<JumpSegment>,
    #[allow(dead_code)]
    pub read_portals: Vec<Option<Portal>>,
}

#[derive(Debug, Clone, Default)]
pub struct JumpSnapshot {
    pub channels: Channels<JumpChannelState>,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct WaveSnapshot {
    pub dry: [f32; UI_BUFFER_SIZE],
    pub wet: [f32; UI_BUFFER_SIZE],
}

impl Default for WaveSnapshot {
    fn default() -> Self {
        Self {
            dry: [0.; UI_BUFFER_SIZE],
            wet: [0.; UI_BUFFER_SIZE],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpectrumRaw {
    pub samples: [f32; SPECTRUM_RAW_SIZE],
}

impl Default for SpectrumRaw {
    fn default() -> Self {
        Self {
            samples: [0.; SPECTRUM_RAW_SIZE],
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorSnapshot {
    pub levels_l: [f32; EDITOR_VIS_SIZE],
    pub levels_r: [f32; EDITOR_VIS_SIZE],
}

impl Default for EditorSnapshot {
    fn default() -> Self {
        Self {
            levels_l: [0.; EDITOR_VIS_SIZE],
            levels_r: [0.; EDITOR_VIS_SIZE],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FrameReads {
    pub frame: UiFrame,
    pub wave: WaveSnapshot,
    pub spectrum_raw: Option<SpectrumRaw>,
    pub editor: EditorSnapshot,
    pub jumps: Option<JumpSnapshot>,
}

impl FrameReads {
    pub fn read() -> Self {
        Self {
            frame: FRAME_STORE.read().unwrap_or_default(),
            wave: FRAME_STORE.read().unwrap_or_default(),
            spectrum_raw: FRAME_STORE.read(),
            editor: FRAME_STORE.read().unwrap_or_default(),
            jumps: FRAME_STORE.read(),
        }
    }
}
