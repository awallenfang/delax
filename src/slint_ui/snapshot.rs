use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex32;

use crate::slint_ui::data_transport::{
    DataTransportRx, InputData, EDITOR_VIS_SIZE, UI_BUFFER_SIZE,
};
use crate::slint_ui::uniforms::{BufferUniforms, DoubleBufferUniforms, SpectrumUniforms};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorChannel {
    Left,
    Right,
}

pub struct UiVisualState {
    out_fft: std::sync::Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex32>,
    hann_window: Vec<f32>,
    spectrum_pending: Vec<f32>,
    seen_editor_len: (usize, usize),
    spectrum: [f32; 32],
    editor_l: [f32; EDITOR_VIS_SIZE],
    editor_r: [f32; EDITOR_VIS_SIZE],
    wave_dry: [f32; UI_BUFFER_SIZE],
    wave_wet: [f32; UI_BUFFER_SIZE],
    wave_pos: usize,
    wave_filled: usize,
}

impl Default for UiVisualState {
    fn default() -> Self {
        let mut fft_planner = FftPlanner::new();
        let fft_plan = fft_planner.plan_fft_forward(64);
        let hann_window: Vec<f32> = hann(64);
        let scratch_len = fft_plan.get_inplace_scratch_len();
        Self {
            out_fft: fft_plan,
            fft_scratch: vec![Complex32::new(0.0, 0.0); scratch_len],
            hann_window,
            spectrum_pending: Vec::with_capacity(32),
            seen_editor_len: (0, 0),
            spectrum: [0.; 32],
            editor_l: [0.; EDITOR_VIS_SIZE],
            editor_r: [0.; EDITOR_VIS_SIZE],
            wave_dry: [0.; UI_BUFFER_SIZE],
            wave_wet: [0.; UI_BUFFER_SIZE],
            wave_pos: 0,
            wave_filled: 0,
        }
    }
}

impl UiVisualState {
    pub fn poll_wave(&mut self, rx: &mut DataTransportRx) {
        while let Ok(s) = rx.wave_cons.pop() {
            self.wave_dry[self.wave_pos] = s.dry;
            self.wave_wet[self.wave_pos] = s.wet;
            self.wave_pos = (self.wave_pos + 1) % UI_BUFFER_SIZE;
            self.wave_filled = (self.wave_filled + 1).min(UI_BUFFER_SIZE);
        }
    }

    pub fn wave_snapshot(&self) -> ([f32; UI_BUFFER_SIZE], [f32; UI_BUFFER_SIZE]) {
        let mut dry = [0.0f32; UI_BUFFER_SIZE];
        let mut wet = [0.0f32; UI_BUFFER_SIZE];
        let start = if self.wave_filled < UI_BUFFER_SIZE {
            0
        } else {
            self.wave_pos
        };
        for i in 0..UI_BUFFER_SIZE {
            let idx = (start + i) % UI_BUFFER_SIZE;
            dry[i] = self.wave_dry[idx];
            wet[i] = self.wave_wet[idx];
        }
        (dry, wet)
    }

    pub fn buffer_uniform(&self, wetness: f32) -> Option<DoubleBufferUniforms> {
        let (dry_flat, wet_flat) = self.wave_snapshot();
        let mut levels_dry = [[0.0f32; 4]; UI_BUFFER_SIZE / 4];
        let mut levels_wet = [[0.0f32; 4]; UI_BUFFER_SIZE / 4];
        for i in 0..UI_BUFFER_SIZE / 4 {
            levels_dry[i] = [
                dry_flat[i * 4],
                dry_flat[i * 4 + 1],
                dry_flat[i * 4 + 2],
                dry_flat[i * 4 + 3],
            ];
            levels_wet[i] = [
                wet_flat[i * 4],
                wet_flat[i * 4 + 1],
                wet_flat[i * 4 + 2],
                wet_flat[i * 4 + 3],
            ];
        }
        Some(DoubleBufferUniforms {
            levels_dry,
            levels_wet,
            primary_col: [1.0, 214. / 255., 10. / 256., 0.5],
            secondary_col: [0.0, 143. / 256., 186. / 256., 0.5],
            params: [wetness, 0., 0., 0.],
        })
    }

    pub fn poll_spectrum(&mut self, rx: &mut DataTransportRx) {
        while let Ok(v) = rx.spec_cons.pop() {
            self.spectrum_pending.push(v);
        }
        if self.spectrum_pending.len() < 64 {
            if self.spectrum_pending.len() > 256 {
                let excess = self.spectrum_pending.len() - 32;
                self.spectrum_pending.drain(0..excess);
            }
            return;
        }
        if self.spectrum_pending.len() > 64 {
            let excess = self.spectrum_pending.len() - 64;
            self.spectrum_pending.drain(0..excess);
        }
        debug_assert_eq!(self.spectrum_pending.len(), 64);
        let mut samples = [0.0f32; 64];
        samples.copy_from_slice(&self.spectrum_pending[..64]);
        self.spectrum_pending.clear();

        for (s, w) in samples.iter_mut().zip(self.hann_window.iter()) {
            *s *= *w;
        }

        let mut complex = [Complex32::new(0.0, 0.0); 64];
        for (c, s) in complex.iter_mut().zip(samples.iter()) {
            *c = Complex32::new(*s, 0.0);
        }
        if self.fft_scratch.len() == self.out_fft.get_inplace_scratch_len() {
            self.out_fft
                .process_with_scratch(&mut complex, &mut self.fft_scratch);
        } else {
            self.out_fft.process(&mut complex);
        }
        for (out, c) in self.spectrum.iter_mut().zip(complex[0..32].iter()) {
            let mag = c.norm();
            let db = util::gain_to_db_fast((mag * 2.0).max(1e-5));
            *out = ((db + 80.0) / 80.0).clamp(0.0, 1.0);
        }
    }

    pub fn spectrum_uniform(&self) -> Option<SpectrumUniforms> {
        Some(SpectrumUniforms {
            levels: self.spectrum,
            // #ffd60a
            primary_col: [1.0, 0.6724, 0.003, 1.0],
        })
    }

    pub fn poll_editor(&mut self, rx: &mut DataTransportRx, data: &InputData) {
        use std::sync::atomic::Ordering::Relaxed;
        let cur_l = data.active_len_l.load(Relaxed);
        let cur_r = data.active_len_r.load(Relaxed);
        if self.seen_editor_len != (cur_l, cur_r) {
            self.seen_editor_len = (cur_l, cur_r);
            self.editor_l.fill(0.);
            self.editor_r.fill(0.);
        }
        while let Ok(chunk) = rx.editor_cons.pop() {
            if cur_l > 0 {
                let bin = ((chunk.pos_l as usize * EDITOR_VIS_SIZE) / cur_l)
                    .min(EDITOR_VIS_SIZE - 1);
                self.editor_l[bin] = chunk.l.clamp(0., 1.);
            }
            if cur_r > 0 {
                let bin = ((chunk.pos_r as usize * EDITOR_VIS_SIZE) / cur_r)
                    .min(EDITOR_VIS_SIZE - 1);
                self.editor_r[bin] = chunk.r.clamp(0., 1.);
            }
        }
    }

    pub fn editor_buffer_uniform(&self, channel: EditorChannel) -> Option<BufferUniforms> {
        let src = match channel {
            EditorChannel::Left => &self.editor_l,
            EditorChannel::Right => &self.editor_r,
        };
        let col = match channel {
            EditorChannel::Left => [1.0, 214. / 255., 10. / 256., 0.5],
            EditorChannel::Right => [0.0, 143. / 255., 186. / 256., 0.5],
        };
        let mut levels = [[0.0f32; 4]; EDITOR_VIS_SIZE / 4];
        for i in 0..EDITOR_VIS_SIZE / 4 {
            levels[i] = [
                src[i * 4],
                src[i * 4 + 1],
                src[i * 4 + 2],
                src[i * 4 + 3],
            ];
        }
        Some(BufferUniforms {
            levels,
            col,
            params: [0.5, 0., 0., 0.],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slint_ui::data_transport as dt;
    use std::sync::atomic::Ordering::Relaxed;

    struct Fixture {
        data: InputData,
        tx: dt::DataTransportTx,
        rx: dt::DataTransportRx,
        visual: UiVisualState,
    }

    impl Fixture {
        fn new() -> Self {
            let (tx, rx) = dt::channel();
            Self {
                data: InputData::default(),
                tx,
                rx,
                visual: UiVisualState::default(),
            }
        }

        fn poll(&mut self) {
            let (rx, visual, data) = (&mut self.rx, &mut self.visual, &self.data);
            visual.poll_editor(rx, data);
        }
    }

    fn feed_revolution(fx: &mut Fixture, len: usize, l_val: f32, r_val: f32) {
        fx.data.active_len_l.store(len, Relaxed);
        fx.data.active_len_r.store(len, Relaxed);
        let mut pos = 0;
        while pos < len {
            fx.tx.push_editor_chunk(dt::EditorChunk {
                l: l_val,
                r: r_val,
                pos_l: pos as u32,
                pos_r: pos as u32,
            });
            pos += dt::EDITOR_CHUNK_SAMPLES;
        }
        fx.poll();
    }

    #[test]
    fn editor_stream_places_chunks_into_bins() {
        let mut fx = Fixture::new();
        fx.data.active_len_l.store(EDITOR_VIS_SIZE, Relaxed);
        fx.data.active_len_r.store(EDITOR_VIS_SIZE, Relaxed);
        fx.tx.push_editor_chunk(dt::EditorChunk {
            l: 0.75,
            r: 0.25,
            pos_l: 0,
            pos_r: 0,
        });
        fx.tx.push_editor_chunk(dt::EditorChunk {
            l: 0.5,
            r: 1.5,
            pos_l: (EDITOR_VIS_SIZE - 1) as u32,
            pos_r: (EDITOR_VIS_SIZE - 1) as u32,
        });
        fx.poll();
        assert_eq!(fx.visual.editor_l[0], 0.75);
        assert_eq!(fx.visual.editor_r[0], 0.25);
        assert_eq!(fx.visual.editor_l[EDITOR_VIS_SIZE - 1], 0.5);
        assert_eq!(fx.visual.editor_r[EDITOR_VIS_SIZE - 1], 1.0);
    }

    #[test]
    fn editor_stream_keeps_channels_separate() {
        let mut fx = Fixture::new();
        feed_revolution(&mut fx, 32768, 0.8, 0.2);
        for cell in fx.visual.editor_l.iter() {
            assert!((cell - 0.8).abs() < 1e-6);
        }
        for cell in fx.visual.editor_r.iter() {
            assert!((cell - 0.2).abs() < 1e-6);
        }
    }

    #[test]
    fn editor_stream_clears_shadow_on_length_change() {
        let mut fx = Fixture::new();
        feed_revolution(&mut fx, 32768, 0.9, 0.9);
        assert_eq!(fx.visual.editor_l[0], 0.9);
        fx.data.active_len_l.store(4096, Relaxed);
        fx.data.active_len_r.store(4096, Relaxed);
        fx.poll();
        assert_eq!(fx.visual.editor_l[0], 0.0);
        assert_eq!(fx.visual.editor_r[0], 0.0);
    }

    #[test]
    fn wave_history_keeps_chronological_order() {
        let mut fx = Fixture::new();
        for i in 0..3 {
            for _ in 0..dt::WAVE_DECIM {
                fx.tx.push_wave_sample(i as f32 * 0.1, 0., 0., 0.);
            }
        }
        fx.visual.poll_wave(&mut fx.rx);
        let (dry, _) = fx.visual.wave_snapshot();
        assert!(dry[UI_BUFFER_SIZE - 1] >= dry[UI_BUFFER_SIZE - 3]);
        let u = fx.visual.buffer_uniform(0.5).expect("uniforms");
        assert_eq!(u.levels_dry[UI_BUFFER_SIZE / 4 - 1][3], dry[UI_BUFFER_SIZE - 1]);
    }

    #[test]
    fn editor_uniforms_differ_by_channel_buffer_and_color() {
        let mut fx = Fixture::new();
        fx.visual.editor_l[0] = 0.75;
        fx.visual.editor_r[0] = 0.25;
        let l = fx
            .visual
            .editor_buffer_uniform(EditorChannel::Left)
            .expect("uniforms");
        let r = fx
            .visual
            .editor_buffer_uniform(EditorChannel::Right)
            .expect("uniforms");
        assert_eq!(l.levels[0][0], 0.75);
        assert_eq!(r.levels[0][0], 0.25);
        assert_ne!(l.col, r.col);
    }
}
