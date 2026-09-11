use nice_plug::prelude::AtomicF32;
use nice_plug::util::gain_to_db;
use std::sync::atomic::{AtomicU8, AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;

use crate::slint_ui::uniforms::DecayUniforms;

pub const SPECTRUM_RING_SIZE: usize = 1024;
pub const EDITOR_RING_SIZE: usize = 2048;
pub const WAVE_RING_SIZE: usize = 512;
pub const WAVE_DECIM: u32 = 256;
pub const SPEC_DECIM: u32 = 8;
pub const UI_BUFFER_SIZE: usize = 128;
pub const EDITOR_VIS_SIZE: usize = UI_BUFFER_SIZE * 4;
pub const EDITOR_CHUNK_SAMPLES: usize = 64;

#[derive(Debug, Clone, Copy)]
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
        // Decimate input
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

#[derive(Debug, Clone, Copy)]
pub struct UiBlock {
    pub in_l: f32,
    pub in_r: f32,
    pub out_l: f32,
    pub out_r: f32,
    pub wetness: f32,
    pub read_head_l: f32,
    pub read_head_r: f32,
    pub write_head_l: f32,
    pub write_head_r: f32,
    pub feedback_l: f32,
    pub feedback_r: f32,
    pub time_s_l: f32,
    pub time_s_r: f32,
    pub bpm: f32,
    pub is_stereo: bool,
    pub is_ping_pong: bool,
    pub bpm_bound_l: bool,
    pub bpm_bound_r: bool,
    pub clamped_l: bool,
    pub clamped_r: bool,
    pub active_len_l: usize,
    pub active_len_r: usize,
}

pub struct InputData {
    pub in_l: AtomicF32,
    pub in_r: AtomicF32,
    pub out_l: AtomicF32,
    pub out_r: AtomicF32,
    pub bpm: AtomicF32,
    pub wetness: AtomicF32,

    pub read_head_l: AtomicF32,
    pub read_head_r: AtomicF32,
    pub write_head_l: AtomicF32,
    pub write_head_r: AtomicF32,

    pub feedback_l: AtomicF32,
    pub feedback_r: AtomicF32,
    pub time_s_l: AtomicF32,
    pub time_s_r: AtomicF32,
    pub is_stereo: AtomicU8,
    pub is_ping_pong: AtomicU8,
    pub bpm_bound_l: AtomicU8,
    pub bpm_bound_r: AtomicU8,
    pub clamped_l: AtomicU8,
    pub clamped_r: AtomicU8,

    pub active_len_l: AtomicUsize,
    pub active_len_r: AtomicUsize,
}

impl Default for InputData {
    fn default() -> Self {
        Self {
            in_l: AtomicF32::new(0.),
            in_r: AtomicF32::new(0.),
            out_l: AtomicF32::new(0.),
            out_r: AtomicF32::new(0.),
            bpm: AtomicF32::new(120.),
            active_len_l: AtomicUsize::new(0),
            active_len_r: AtomicUsize::new(0),
            wetness: AtomicF32::new(0.5),
            feedback_l: AtomicF32::new(0.5),
            feedback_r: AtomicF32::new(0.5),
            time_s_l: AtomicF32::new(0.5),
            time_s_r: AtomicF32::new(0.5),
            is_stereo: AtomicU8::new(0),
            is_ping_pong: AtomicU8::new(0),
            bpm_bound_l: AtomicU8::new(0),
            bpm_bound_r: AtomicU8::new(0),
            clamped_l: AtomicU8::new(0),
            clamped_r: AtomicU8::new(0),
            read_head_l: AtomicF32::new(0.),
            read_head_r: AtomicF32::new(0.),
            write_head_l: AtomicF32::new(0.),
            write_head_r: AtomicF32::new(0.),
        }
    }
}

impl InputData {
    pub fn publish_block(&self, b: &UiBlock) {
        self.in_l.store(b.in_l, Relaxed);
        self.in_r.store(b.in_r, Relaxed);
        self.out_l.store(b.out_l, Relaxed);
        self.out_r.store(b.out_r, Relaxed);
        self.wetness.store(b.wetness, Relaxed);
        self.read_head_l.store(b.read_head_l, Relaxed);
        self.read_head_r.store(b.read_head_r, Relaxed);
        self.write_head_l.store(b.write_head_l, Relaxed);
        self.write_head_r.store(b.write_head_r, Relaxed);
        self.feedback_l.store(b.feedback_l, Relaxed);
        self.feedback_r.store(b.feedback_r, Relaxed);
        self.time_s_l.store(b.time_s_l, Relaxed);
        self.time_s_r.store(b.time_s_r, Relaxed);
        self.bpm.store(b.bpm, Relaxed);
        self.is_stereo.store(b.is_stereo as u8, Relaxed);
        self.is_ping_pong.store(b.is_ping_pong as u8, Relaxed);
        self.bpm_bound_l.store(b.bpm_bound_l as u8, Relaxed);
        self.bpm_bound_r.store(b.bpm_bound_r as u8, Relaxed);
        self.clamped_l.store(b.clamped_l as u8, Relaxed);
        self.clamped_r.store(b.clamped_r as u8, Relaxed);
        self.active_len_l.store(b.active_len_l, Relaxed);
        self.active_len_r.store(b.active_len_r, Relaxed);
    }

    pub fn reset(&self) {
        self.in_l.store(0., Relaxed);
        self.in_r.store(0., Relaxed);
        self.out_l.store(0., Relaxed);
        self.out_r.store(0., Relaxed);
    }

    pub fn decay_uniform(&self) -> Option<DecayUniforms> {
        let bpm = self.bpm.load(Relaxed);
        Some(DecayUniforms {
            feedback: [self.feedback_l.load(Relaxed), self.feedback_r.load(Relaxed)],
            time_s: [self.time_s_l.load(Relaxed), self.time_s_r.load(Relaxed)],
            flags: [
                self.is_stereo.load(Relaxed) as f32,
                self.is_ping_pong.load(Relaxed) as f32,
                self.bpm_bound_l.load(Relaxed) as f32,
                self.bpm_bound_r.load(Relaxed) as f32,
            ],
            color_primary: [1.0, 214. / 255., 10. / 255., 0.5],
            color_secondary: [0.0, 143. / 255., 186. / 255., 0.5],
            grid: [240.0 / bpm.max(1.0), 0.0, 0.0, 0.0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_never_block_and_drop_on_full() {
        let (mut tx, mut rx) = channel();
        for i in 0..(SPECTRUM_RING_SIZE * SPEC_DECIM as usize + 100) {
            tx.push_spectrum_sample(i as f32);
        }
        let mut n = 0;
        while rx.spec_cons.pop().is_ok() {
            n += 1;
        }
        assert_eq!(n, SPECTRUM_RING_SIZE);

        for i in 0..(EDITOR_RING_SIZE + 10) {
            tx.push_editor_chunk(EditorChunk {
                l: i as f32,
                r: 0.,
                pos_l: i as u32,
                pos_r: i as u32,
            });
        }
        let mut m = 0;
        while rx.editor_cons.pop().is_ok() {
            m += 1;
        }
        assert_eq!(m, EDITOR_RING_SIZE);
    }

    #[test]
    fn wave_peak_hold_and_stride() {
        let (mut tx, mut rx) = channel();
        for _ in 0..(WAVE_DECIM - 1) {
            tx.push_wave_sample(0.1, 0.1, 0.1, 0.1);
        }
        assert!(rx.wave_cons.pop().is_err());
        tx.push_wave_sample(0.5, 0.5, 0.25, 0.25);
        let s = rx.wave_cons.pop().expect("one sample per stride");
        assert!((s.dry - db01(0.5)).abs() < 1e-6);
        assert!((s.wet - db01(0.25)).abs() < 1e-6);
    }

    #[test]
    fn spectrum_boxcar_mean() {
        let (mut tx, mut rx) = channel();
        for i in 1..=SPEC_DECIM {
            tx.push_spectrum_sample(i as f32);
        }
        let v = rx.spec_cons.pop().expect("one mean per stride");
        let expect: f32 = (1..=SPEC_DECIM).map(|i| i as f32).sum::<f32>() / SPEC_DECIM as f32;
        assert!((v - expect).abs() < 1e-6);
    }
}
