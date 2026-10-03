use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::delay_engine::jump_builder::{Jump, JumpSegment, Portal};
use crate::slint_ui::frames::{JumpSnapshot, SPECTRUM_RAW_SIZE};
use crate::slint_ui::{UIJump, UIJumpSegment, UIPortal};

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

pub fn normalize_portals(portals: &[Option<Portal>], active_len: usize) -> Vec<UIPortal> {
    portals
        .iter()
        .enumerate()
        .filter_map(|(boundary, slot)| {
            slot.map(|p| UIPortal {
                boundary: boundary as i32,
                exit: normalize_ratio(p.exit, active_len),
                entry: normalize_ratio(p.entry, active_len),
                live: p.exit + 1 != p.entry,
            })
        })
        .collect()
}

pub fn ratio_to_sample(ratio: f32, active_len: usize) -> usize {
    if ratio.is_nan() || active_len == 0 {
        return 0;
    }
    (ratio.clamp(0.0, 1.0) * active_len as f32).round() as usize
}

#[cfg(test)]
mod tests {
    use super::{normalize_portals, ratio_to_sample};
    use crate::delay_engine::jump_builder::Portal;

    #[test]
    fn portals_keep_their_boundary_index_when_flattened() {
        let slots = vec![
            None,
            Some(Portal { exit: 3, entry: 4 }),
            None,
            Some(Portal { exit: 7, entry: 7 }),
        ];
        let out = normalize_portals(&slots, 12);
        assert_eq!(out.len(), 2, "welded boundaries are dropped");
        assert_eq!(out[0].boundary, 1, "and the survivors keep their slot");
        assert_eq!(out[1].boundary, 3);
    }

    #[test]
    fn a_fresh_unglue_is_not_live_but_a_moved_entry_is() {
        let fresh = Some(Portal { exit: 3, entry: 4 });
        assert!(
            !normalize_portals(&[fresh], 12)[0].live,
            "a fresh unglue anchors exit/entry at the welded values, so it changes nothing"
        );

        let moved = Some(Portal { exit: 1, entry: 4 });
        assert!(normalize_portals(&[moved], 12)[0].live);
    }

    #[test]
    fn portal_positions_are_ratios_of_the_buffer() {
        let out = normalize_portals(&[Some(Portal { exit: 3, entry: 9 })], 12);
        assert!((out[0].exit - 0.25).abs() < 1e-6);
        assert!((out[0].entry - 0.75).abs() < 1e-6);
    }

    #[test]
    fn ratio_to_sample_inverts_the_normalization() {
        assert_eq!(ratio_to_sample(0.0, 12), 0);
        assert_eq!(ratio_to_sample(1.0, 12), 12);
        assert_eq!(ratio_to_sample(0.25, 12), 3);
        assert_eq!(ratio_to_sample(-3.0, 12), 0);
        assert_eq!(ratio_to_sample(f32::NAN, 12), 0);
        assert_eq!(ratio_to_sample(f32::INFINITY, 12), 12);
        assert_eq!(ratio_to_sample(0.25, 0), 0);
    }
}
