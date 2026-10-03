use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::delay_engine::jump_builder::{Jump, JumpSegment};
use crate::slint_ui::frames::{JumpSnapshot, SPECTRUM_RAW_SIZE};
use crate::slint_ui::{UIJump, UIJumpSegment};

pub struct SpectrumState {
    out_fft: std::sync::Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex32>,
    hann_window: Vec<f32>,
    levels: [f32; 32],
}

impl Default for SpectrumState {
    fn default() -> Self {
        let mut fft_planner = FftPlanner::new();
        let fft_plan = fft_planner.plan_fft_forward(SPECTRUM_RAW_SIZE);
        let hann_window: Vec<f32> = hann(SPECTRUM_RAW_SIZE);
        let scratch_len = fft_plan.get_inplace_scratch_len();
        Self {
            out_fft: fft_plan,
            fft_scratch: vec![Complex32::new(0.0, 0.0); scratch_len],
            hann_window,
            levels: [0.; 32],
        }
    }
}

impl SpectrumState {
    pub fn push_raw(&mut self, raw: &[f32; SPECTRUM_RAW_SIZE]) {
        let mut samples = [0.0f32; SPECTRUM_RAW_SIZE];
        samples.copy_from_slice(raw);
        for (s, w) in samples.iter_mut().zip(self.hann_window.iter()) {
            *s *= *w;
        }
        let mut complex = [Complex32::new(0.0, 0.0); SPECTRUM_RAW_SIZE];
        for (c, s) in complex.iter_mut().zip(samples.iter()) {
            *c = Complex32::new(*s, 0.0);
        }
        if self.fft_scratch.len() == self.out_fft.get_inplace_scratch_len() {
            self.out_fft
                .process_with_scratch(&mut complex, &mut self.fft_scratch);
        } else {
            self.out_fft.process(&mut complex);
        }
        for (out, c) in self.levels.iter_mut().zip(complex[0..32].iter()) {
            let mag = c.norm();
            let db = util::gain_to_db_fast((mag * 2.0).max(1e-5));
            *out = ((db + 80.0) / 80.0).clamp(0.0, 1.0);
        }
    }

    pub fn levels(&self) -> [f32; 32] {
        self.levels
    }
}

#[derive(Default)]
pub struct JumpCursor {
    seen: u64,
}

impl JumpCursor {
    pub fn fresh<'a>(&mut self, jumps: &'a JumpSnapshot) -> Option<&'a JumpSnapshot> {
        if jumps.version != self.seen {
            self.seen = jumps.version;
            Some(jumps)
        } else {
            None
        }
    }
}

fn normalize_ratio(value: usize, active_len: usize) -> f32 {
    value as f32 / active_len.max(1) as f32
}

pub fn normalize_jumps(jumps: &[Jump], active_len: usize) -> Vec<UIJump> {
    jumps
        .iter()
        .map(|j| UIJump {
            from: normalize_ratio(j.from, active_len),
            to: normalize_ratio(j.to, active_len),
            order: j.rank as i32,
        })
        .collect()
}

pub fn normalize_segments(segments: &[JumpSegment], active_len: usize) -> Vec<UIJumpSegment> {
    segments
        .iter()
        .map(|s| UIJumpSegment {
            start: normalize_ratio(s.start, active_len),
            end: normalize_ratio(s.end, active_len),
            order: s.order as i32,
        })
        .collect()
}
