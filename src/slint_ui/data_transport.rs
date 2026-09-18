use nice_plug::util::gain_to_db;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::delay_engine::jump_builder::{Jump, JumpSegment};
use crate::slint_ui::channels::{Channels, Heads};
use crate::slint_ui::uniforms::DecayUniforms;

pub const SPECTRUM_RING_SIZE: usize = 1024;
pub const EDITOR_RING_SIZE: usize = 2048;
pub const WAVE_RING_SIZE: usize = 512;
pub const WAVE_DECIM: u32 = 256;
pub const SPEC_DECIM: u32 = 8;
pub const UI_BUFFER_SIZE: usize = 128;
pub const EDITOR_VIS_SIZE: usize = UI_BUFFER_SIZE * 4;
pub const EDITOR_CHUNK_SAMPLES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferChannel {
    Left,
    Right,
}

impl BufferChannel {
    pub fn from_i32(v: i32) -> Self {
        if v == 0 {
            Self::Left
        } else {
            Self::Right
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorChunk {
    pub l: f32,
    pub r: f32,
    pub pos_l: u32,
    pub pos_r: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct WaveSample {
    pub dry: f32,
    pub wet: f32,
}

pub struct DataTransportTx {
    spec_prod: rtrb::Producer<f32>,
    editor_prod: rtrb::Producer<EditorChunk>,
    wave_prod: rtrb::Producer<WaveSample>,
    wave_peak_dry: f32,
    wave_peak_wet: f32,
    wave_count: u32,
    spec_sum: f32,
    spec_count: u32,
}

pub struct DataTransportRx {
    pub spec_cons: rtrb::Consumer<f32>,
    pub editor_cons: rtrb::Consumer<EditorChunk>,
    pub wave_cons: rtrb::Consumer<WaveSample>,
}

pub fn channel() -> (DataTransportTx, DataTransportRx) {
    let (spec_prod, spec_cons) = rtrb::RingBuffer::new(SPECTRUM_RING_SIZE);
    let (editor_prod, editor_cons) = rtrb::RingBuffer::new(EDITOR_RING_SIZE);
    let (wave_prod, wave_cons) = rtrb::RingBuffer::new(WAVE_RING_SIZE);
    (
        DataTransportTx {
            spec_prod,
            editor_prod,
            wave_prod,
            wave_peak_dry: 0.,
            wave_peak_wet: 0.,
            wave_count: 0,
            spec_sum: 0.,
            spec_count: 0,
        },
        DataTransportRx {
            spec_cons,
            editor_cons,
            wave_cons,
        },
    )
}

fn db01(peak_abs: f32) -> f32 {
    (1. + gain_to_db(peak_abs) / 100.).clamp(0., 1.5)
}

impl DataTransportTx {
    pub fn push_spectrum_sample(&mut self, mono: f32) {
        self.spec_sum += mono;
        self.spec_count += 1;
        if self.spec_count >= SPEC_DECIM {
            let _ = self.spec_prod.push(self.spec_sum / SPEC_DECIM as f32);
            self.spec_sum = 0.;
            self.spec_count = 0;
        }
    }

    pub fn push_wave_sample(&mut self, dry_l: f32, dry_r: f32, wet_l: f32, wet_r: f32) {
        let dm = ((dry_l + dry_r) * 0.5).abs();
        let wm = ((wet_l + wet_r) * 0.5).abs();
        self.wave_peak_dry = self.wave_peak_dry.max(dm);
        self.wave_peak_wet = self.wave_peak_wet.max(wm);
        self.wave_count += 1;
        if self.wave_count >= WAVE_DECIM {
            let _ = self.wave_prod.push(WaveSample {
                dry: db01(self.wave_peak_dry.max(1e-5)),
                wet: db01(self.wave_peak_wet.max(1e-5)),
            });
            self.wave_peak_dry = 0.;
            self.wave_peak_wet = 0.;
            self.wave_count = 0;
        }
    }

    pub fn push_editor_chunk(&mut self, chunk: EditorChunk) {
        let _ = self.editor_prod.push(chunk);
    }

    pub fn reset_decim(&mut self) {
        self.wave_peak_dry = 0.;
        self.wave_peak_wet = 0.;
        self.wave_count = 0;
        self.spec_sum = 0.;
        self.spec_count = 0;
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

pub fn ui_block_channel() -> (
    triple_buffer::Input<UiFrame>,
    triple_buffer::Output<UiFrame>,
) {
    triple_buffer::TripleBuffer::new(&UiFrame::default()).split()
}

#[derive(Debug, Clone, Default)]
pub struct JumpChannelState {
    pub read: Vec<Jump>,
    pub write: Vec<Jump>,
    pub read_segments: Vec<JumpSegment>,
    pub write_segments: Vec<JumpSegment>,
}

#[derive(Debug, Clone, Default)]
pub struct JumpState {
    pub channels: Channels<JumpChannelState>,
}

pub struct JumpCache {
    read_jumps: Channels<Mutex<Vec<Jump>>>,
    write_jumps: Channels<Mutex<Vec<Jump>>>,
    read_segments: Channels<Mutex<Vec<JumpSegment>>>,
    write_segments: Channels<Mutex<Vec<JumpSegment>>>,
    version: AtomicU64,
}

impl Default for JumpCache {
    fn default() -> Self {
        Self {
            read_jumps: Channels::new(Mutex::new(vec![]), Mutex::new(vec![])),
            write_jumps: Channels::new(Mutex::new(vec![]), Mutex::new(vec![])),
            read_segments: Channels::new(Mutex::new(vec![]), Mutex::new(vec![])),
            write_segments: Channels::new(Mutex::new(vec![]), Mutex::new(vec![])),
            version: AtomicU64::new(0),
        }
    }
}

impl JumpCache {
    pub fn publish(&self, state: JumpState) {
        for ch in [BufferChannel::Left, BufferChannel::Right] {
            let s = state.channels.get(ch);
            if let Ok(mut g) = self.read_jumps.get(ch).lock() {
                *g = s.read.clone();
            }
            if let Ok(mut g) = self.write_jumps.get(ch).lock() {
                *g = s.write.clone();
            }
            if let Ok(mut g) = self.read_segments.get(ch).lock() {
                *g = s.read_segments.clone();
            }
            if let Ok(mut g) = self.write_segments.get(ch).lock() {
                *g = s.write_segments.clone();
            }
        }
        self.version.fetch_add(1, Relaxed);
    }

    pub fn version(&self) -> u64 {
        self.version.load(Relaxed)
    }

    pub fn read_jumps(&self, ch: BufferChannel) -> Vec<Jump> {
        self.read_jumps
            .get(ch)
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn read_segments(&self, ch: BufferChannel) -> Vec<JumpSegment> {
        self.read_segments
            .get(ch)
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn write_jumps(&self, ch: BufferChannel) -> Vec<Jump> {
        self.write_jumps
            .get(ch)
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn write_segments(&self, ch: BufferChannel) -> Vec<JumpSegment> {
        self.write_segments
            .get(ch)
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

pub struct UiState {
    block_output: Mutex<triple_buffer::Output<UiFrame>>,
    pub jumps: JumpCache,
}

impl UiState {
    pub fn with_output(output: triple_buffer::Output<UiFrame>) -> Self {
        Self {
            block_output: Mutex::new(output),
            jumps: JumpCache::default(),
        }
    }

    pub fn read_block(&self) -> UiFrame {
        self.block_output
            .lock()
            .map(|mut o| o.read().clone())
            .unwrap_or_default()
    }

    pub fn jump_version(&self) -> u64 {
        self.jumps.version()
    }
}

impl Default for UiState {
    fn default() -> Self {
        let (_, output) = ui_block_channel();
        Self::with_output(output)
    }
}

impl UiState {
    pub fn publish_jump_state(&self, s: JumpState) {
        self.jumps.publish(s);
    }

    pub fn active_len_for(&self, ch: BufferChannel) -> usize {
        *self.read_block().active_len.get(ch)
    }

    pub fn reset(&self) {}

    pub fn decay_uniform(&self) -> Option<DecayUniforms> {
        let block = self.read_block();
        Some(DecayUniforms {
            feedback: [block.feedback.left, block.feedback.right],
            time_s: [block.decay.left, block.decay.right],
            flags: [0., 0., 0., 0.],
            color_primary: [1.0, 214. / 255., 10. / 255., 0.5],
            color_secondary: [0.0, 143. / 255., 186. / 255., 0.5],
            grid: [240.0 / block.bpm.max(1.0), 0.0, 0.0, 0.0],
        })
    }

    pub fn decay_uniform_with_flags(
        &self,
        is_stereo: bool,
        is_ping_pong: bool,
        bpm_bound_l: bool,
        bpm_bound_r: bool,
    ) -> Option<DecayUniforms> {
        let block = self.read_block();
        Some(DecayUniforms {
            feedback: [block.feedback.left, block.feedback.right],
            time_s: [block.decay.left, block.decay.right],
            flags: [
                is_stereo as u8 as f32,
                is_ping_pong as u8 as f32,
                bpm_bound_l as u8 as f32,
                bpm_bound_r as u8 as f32,
            ],
            color_primary: [1.0, 214. / 255., 10. / 255., 0.5],
            color_secondary: [0.0, 143. / 255., 186. / 255., 0.5],
            grid: [240.0 / block.bpm.max(1.0), 0.0, 0.0, 0.0],
        })
    }
}
