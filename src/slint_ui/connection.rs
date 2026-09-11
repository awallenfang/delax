use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU8, AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use nice_plug::prelude::AtomicF32;
use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex32;
use crate::slint_ui::{self, EditorData};
use crate::slint_ui::elements::{ElementId, GpuElementData};
use crate::slint_ui::HeaderData;
use crate::slint_ui::uniforms::{BufferUniforms, DecayUniforms, DoubleBufferUniforms, SpectrumUniforms};

pub const UI_BUFFER_SIZE: usize = 128;
pub const EDITOR_VIS_SIZE: usize = UI_BUFFER_SIZE * 4;
pub const EDITOR_CHUNK_SAMPLES: usize = 64;
pub const EDITOR_RING_SIZE: usize = 2048;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorChannel {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct EditorChunk {
    pub l: f32,
    pub r: f32,
    pub pos_l: u32,
    pub pos_r: u32,
}

pub struct InputData {
    pub in_l: AtomicF32,
    pub in_r: AtomicF32,
    pub out_l: AtomicF32,
    pub out_r: AtomicF32,
    pub out_spectrum: [AtomicF32; 32],
    pub bpm: AtomicF32,
    pub dry_buffer: [AtomicF32; UI_BUFFER_SIZE],
    pub wet_buffer: [AtomicF32; UI_BUFFER_SIZE],
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
    pub editor_l: [AtomicF32; EDITOR_VIS_SIZE],
    pub editor_r: [AtomicF32; EDITOR_VIS_SIZE],

    dry_skip_counter: AtomicU16,
    wet_skip_counter: AtomicU16,
    skip: u16,

    dry_pos: AtomicUsize,
    wet_pos: AtomicUsize,

    out_fft: Arc<dyn Fft<f32>>,
    spectrum_producer: std::sync::Mutex<Option<rtrb::Producer<f32>>>,
    spectrum_consumer: Arc<std::sync::Mutex<Option<rtrb::Consumer<f32>>>>,
    fft_scratch: std::sync::Mutex<Vec<Complex32>>,
    hann_window: Vec<f32>,
    spectrum_pending: std::sync::Mutex<Vec<f32>>,
    editor_producer: std::sync::Mutex<Option<rtrb::Producer<EditorChunk>>>,
    editor_consumer: Arc<std::sync::Mutex<Option<rtrb::Consumer<EditorChunk>>>>,
    pub active_len_l: AtomicUsize,
    pub active_len_r: AtomicUsize,
    seen_editor_len: std::sync::Mutex<(usize, usize)>,
}

impl Default for InputData {
    fn default() -> Self {
        let mut fft_planner = FftPlanner::new();
        let fft_plan = fft_planner.plan_fft_forward(64);
        let (spec_prod, spec_cons) = rtrb::RingBuffer::new(4096);
        let (editor_prod, editor_cons) = rtrb::RingBuffer::new(EDITOR_RING_SIZE);
        let hann_window: Vec<f32> = hann(64);
        let scratch_len = fft_plan.get_inplace_scratch_len();
        Self {
            in_l: AtomicF32::new(0.),
            in_r: AtomicF32::new(0.),
            out_l: AtomicF32::new(0.),
            out_r: AtomicF32::new(0.),
            out_spectrum: [const { AtomicF32::new(0.) }; 32],
            bpm: AtomicF32::new(120.),
            dry_buffer: [const { AtomicF32::new(0.) }; UI_BUFFER_SIZE],
            wet_buffer: [const { AtomicF32::new(0.) }; UI_BUFFER_SIZE],
            dry_skip_counter: AtomicU16::new(0),
            wet_skip_counter: AtomicU16::new(0),
            skip: 1024,
            dry_pos: AtomicUsize::new(0),
            wet_pos: AtomicUsize::new(0),
            out_fft: fft_plan,
            spectrum_producer: std::sync::Mutex::new(Some(spec_prod)),
            spectrum_consumer: Arc::new(std::sync::Mutex::new(Some(spec_cons))),
            editor_producer: std::sync::Mutex::new(Some(editor_prod)),
            editor_consumer: Arc::new(std::sync::Mutex::new(Some(editor_cons))),
            active_len_l: AtomicUsize::new(0),
            active_len_r: AtomicUsize::new(0),
            seen_editor_len: std::sync::Mutex::new((0, 0)),
            fft_scratch: std::sync::Mutex::new(vec![Complex32::new(0.0, 0.0); scratch_len]),
            hann_window,
            spectrum_pending: std::sync::Mutex::new(Vec::with_capacity(32)),
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
            editor_l: [const { AtomicF32::new(0.) }; EDITOR_VIS_SIZE],
            editor_r: [const { AtomicF32::new(0.) }; EDITOR_VIS_SIZE],
            read_head_l: AtomicF32::new(0.),
            read_head_r: AtomicF32::new(0.),
            write_head_l: AtomicF32::new(0.),
            write_head_r: AtomicF32::new(0.),
        }
    }
}

impl InputData {
    pub fn push_spectrum(&self, mono: f32) {
        if let Ok(mut guard) = self.spectrum_producer.try_lock() {
            if let Some(prod) = guard.as_mut() {
                let _ = prod.push(mono);
            }
        }
    }

    pub fn push_dry(&self, l: f32, r: f32) {
        // Skip a lot of data to slow down the display
        let dry_skip = self.dry_skip_counter.fetch_add(1, Relaxed);
        if dry_skip % self.skip != 0 {
            return;
        }
        let mono = (l + r) * 0.5;
        let db = (1. + util::gain_to_db(mono.abs()) / 100.).clamp(0., 1.5);

        let pos = self.dry_pos.fetch_add(1, Relaxed) % self.dry_buffer.len();
        self.dry_buffer[pos].store(db, Relaxed);
    }

    pub fn push_wet(&self, l: f32, r: f32) {
        // Skip a lot of data to slow down the display
        let wet_skip = self.wet_skip_counter.fetch_add(1, Relaxed);
        if wet_skip % self.skip != 0 {
            return;
        }
        let mono = (l + r) * 0.5;
        let db = (1. + util::gain_to_db(mono.abs()) / 100.).clamp(0., 1.5);
        let pos = self.wet_pos.fetch_add(1, Relaxed) % self.wet_buffer.len();
        self.wet_buffer[pos].store(db, Relaxed);
    }

    pub fn set_bpm(&self, bpm: f32) {
        self.bpm.store(bpm, Relaxed);
    }

    /// Push the per-sample delay/feedback state used by the decay visualizer.
    pub fn set_decay_state(
        &self,
        feedback_l: f32,
        feedback_r: f32,
        time_s_l: f32,
        time_s_r: f32,
        is_stereo: bool,
        is_ping_pong: bool,
        bpm_bound_l: bool,
        bpm_bound_r: bool,
    ) {
        self.feedback_l.store(feedback_l, Relaxed);
        self.feedback_r.store(feedback_r, Relaxed);
        self.time_s_l.store(time_s_l, Relaxed);
        self.time_s_r.store(time_s_r, Relaxed);
        self.is_stereo.store(is_stereo as u8, Relaxed);
        self.is_ping_pong.store(is_ping_pong as u8, Relaxed);
        self.bpm_bound_l.store(bpm_bound_l as u8, Relaxed);
        self.bpm_bound_r.store(bpm_bound_r as u8, Relaxed);
    }

    pub fn set_clamped(&self, clamped_l: bool, clamped_r: bool) {
        self.clamped_l.store(clamped_l as u8, Relaxed);
        self.clamped_r.store(clamped_r as u8, Relaxed);
    }

    pub fn push_editor_chunk(&self, chunk: EditorChunk) {
        if let Ok(mut guard) = self.editor_producer.try_lock() {
            if let Some(prod) = guard.as_mut() {
                let _ = prod.push(chunk);
            }
        }
    }

    pub fn poll_editor(&self) {
        let cur_l = self.active_len_l.load(Relaxed);
        let cur_r = self.active_len_r.load(Relaxed);
        if let Ok(mut seen) = self.seen_editor_len.try_lock() {
            if *seen != (cur_l, cur_r) {
                *seen = (cur_l, cur_r);
                for cell in self.editor_l.iter().chain(self.editor_r.iter()) {
                    cell.store(0., Relaxed);
                }
            }
        }
        let Ok(mut guard) = self.editor_consumer.try_lock() else {
            return;
        };
        let Some(cons) = guard.as_mut() else {
            return;
        };
        while let Ok(chunk) = cons.pop() {
            if cur_l > 0 {
                let bin = ((chunk.pos_l as usize * EDITOR_VIS_SIZE) / cur_l)
                    .min(EDITOR_VIS_SIZE - 1);
                self.editor_l[bin].store(chunk.l.clamp(0., 1.), Relaxed);
            }
            if cur_r > 0 {
                let bin = ((chunk.pos_r as usize * EDITOR_VIS_SIZE) / cur_r)
                    .min(EDITOR_VIS_SIZE - 1);
                self.editor_r[bin].store(chunk.r.clamp(0., 1.), Relaxed);
            }
        }
    }

    pub fn reset(&self) {
        self.in_l.store(0., Relaxed);
        self.in_r.store(0., Relaxed);
        self.out_l.store(0., Relaxed);
        self.out_r.store(0., Relaxed);
        for buf in [&self.dry_buffer, &self.wet_buffer] {
            for cell in buf.iter() {
                cell.store(0., Relaxed);
            }
        }
        self.dry_pos.store(0, Relaxed);
        self.wet_pos.store(0, Relaxed);
        for cell in self.editor_l.iter().chain(self.editor_r.iter()) {
            cell.store(0., Relaxed);
        }
        if let Ok(mut guard) = self.spectrum_consumer.try_lock() {
            if let Some(cons) = guard.as_mut() {
                while cons.pop().is_ok() {}
            }
        }
        if let Ok(mut guard) = self.editor_consumer.try_lock() {
            if let Some(cons) = guard.as_mut() {
                while cons.pop().is_ok() {}
            }
        }
        if let Ok(mut pending) = self.spectrum_pending.try_lock() {
            pending.clear();
        }
    }

    pub fn update_ui(&self, app: &slint_ui::AppWindow) {
        app.set_header_data(HeaderData {
            in_level_l: self.in_l.load(Relaxed),
            in_level_r: self.in_r.load(Relaxed),
            out_level_l: self.in_l.load(Relaxed),
            out_level_r: self.out_r.load(Relaxed),
        });
        app.set_bpm(self.bpm.load(Relaxed));
        app.set_editor_data(EditorData {
            write_head_l: self.write_head_l.load(Relaxed),
            write_head_r: self.write_head_r.load(Relaxed),
            read_head_l: self.read_head_l.load(Relaxed),
            read_head_r: self.read_head_r.load(Relaxed)
        });

        self.poll_spectrum(app);
        self.poll_waveforms(app);
        self.poll_editor();
    }

    fn poll_spectrum(&self, _app: &slint_ui::AppWindow) {
        let mut samples = [0.0f32; 64];
        let mut have_samples = false;
        {
            let Ok(mut guard) = self.spectrum_consumer.try_lock() else {
                return;
            };
            let Some(cons) = guard.as_mut() else {
                return;
            };
            let Ok(mut pending) = self.spectrum_pending.try_lock() else {
                return;
            };
            while let Ok(v) = cons.pop() {
                pending.push(v);
            }
            if pending.len() < 64 {
                if pending.len() > 256 {
                    let excess = pending.len() - 32;
                    pending.drain(0..excess);
                }
                return;
            }
            if pending.len() > 64 {
                let excess = pending.len() - 64;
                pending.drain(0..excess);
            }
            debug_assert_eq!(pending.len(), 64);
            samples.copy_from_slice(&pending[..64]);
            pending.clear();
            have_samples = true;
        }
        if !have_samples {
            return;
        }
        for (s, w) in samples.iter_mut().zip(self.hann_window.iter()) {
            *s *= *w;
        }

        let mut complex = [Complex32::new(0.0, 0.0); 64];
        for (c, s) in complex.iter_mut().zip(samples.iter()) {
            *c = Complex32::new(*s, 0.0);
        }
        if let Ok(mut scratch) = self.fft_scratch.try_lock() {
            if scratch.len() == self.out_fft.get_inplace_scratch_len() {
                self.out_fft
                    .process_with_scratch(&mut complex, &mut scratch);
            } else {
                self.out_fft.process(&mut complex);
            }
        } else {
            self.out_fft.process(&mut complex);
        }
        let mut spectrum = [0.0f32; 32];
        for (out, c) in spectrum.iter_mut().zip(complex[0..32].iter()) {
            let mag = c.norm();
            let db = util::gain_to_db_fast((mag * 2.0).max(1e-5));
            *out = ((db + 80.0) / 80.0).clamp(0.0, 1.0);
        }
        for i in 0..32 {
            self.out_spectrum[i].store(spectrum[i], Relaxed);
        }
    }

    fn poll_waveforms(&self, app: &slint_ui::AppWindow) {
        let dry_pos = self.dry_pos.load(Relaxed) % self.dry_buffer.len();
        let wet_pos = self.wet_pos.load(Relaxed) % self.wet_buffer.len();
        let mut dry = [0.0f32; UI_BUFFER_SIZE];
        let mut wet = [0.0f32; UI_BUFFER_SIZE];
        for i in 0..self.wet_buffer.len().min(self.dry_buffer.len()) {
            let idx = (dry_pos + 1 + i) % self.dry_buffer.len();
            dry[i] = self.dry_buffer[idx].load(Relaxed);
            let idx = (wet_pos + 1 + i) % self.wet_buffer.len();
            wet[i] = self.wet_buffer[idx].load(Relaxed);
        }
        app.set_dry_buffer(slint::ModelRc::new(slint::VecModel::from(dry.to_vec())));
        app.set_wet_buffer(slint::ModelRc::new(slint::VecModel::from(wet.to_vec())));
    }

    pub fn spectrum_uniform(&self) -> Option<SpectrumUniforms> {
        let mut spectrum = [0.0f32; 32];
        for i in 0..32 {
            spectrum[i] = self.out_spectrum[i].load(Relaxed);
        }
        Some(SpectrumUniforms {
            levels: spectrum,
            // #ffd60a
            primary_col: [1.0, 0.6724, 0.003, 1.0],
        })
    }

    pub fn buffer_uniform(&self) -> Option<DoubleBufferUniforms> {
        let dry_pos = self.dry_pos.load(Relaxed);
        let wet_pos = self.wet_pos.load(Relaxed);

        let mut dry_flat = [0.0f32; UI_BUFFER_SIZE];
        let mut wet_flat = [0.0f32; UI_BUFFER_SIZE];
        for i in 0..UI_BUFFER_SIZE {
            dry_flat[i] = self.dry_buffer[(dry_pos + 1 + i) % UI_BUFFER_SIZE].load(Relaxed);
            wet_flat[i] = self.wet_buffer[(wet_pos + 1 + i) % UI_BUFFER_SIZE].load(Relaxed);
        }

        // Pack 32 flat floats into 8 vec4 chunks (8 * 4 = 32)
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
            primary_col: [1.0, 214./255., 10./256., 0.5],
            secondary_col: [0.0, 143./256., 186./256., 0.5],
            params: [self.wetness.load(Relaxed), 0., 0., 0.]
        })
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
                src[i * 4].load(Relaxed),
                src[i * 4 + 1].load(Relaxed),
                src[i * 4 + 2].load(Relaxed),
                src[i * 4 + 3].load(Relaxed),
            ];
        }
        Some(BufferUniforms {
            levels,
            col,
            params: [0.5, 0., 0., 0.],
        })
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

impl GpuElementData for InputData {
    fn element_uniform(&self, element: ElementId) -> Option<Vec<u8>> {
        match element {
            ElementId::Spectrum => {
                let uniforms = self.spectrum_uniform()?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            },
            ElementId::Buffer => {
                let uniforms = self.buffer_uniform()?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            }
            ElementId::EditorBufferL => {
                let uniforms = self.editor_buffer_uniform(EditorChannel::Left)?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            }
            ElementId::EditorBufferR => {
                let uniforms = self.editor_buffer_uniform(EditorChannel::Right)?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            }
            ElementId::Decay => {
                let uniforms = self.decay_uniform()?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn feed_revolution(data: &InputData, len: usize, l_val: f32, r_val: f32) {
        data.active_len_l.store(len, Relaxed);
        data.active_len_r.store(len, Relaxed);
        // One chunk per 64-sample window, positions sweeping the loop.
        let mut pos = 0;
        while pos < len {
            data.push_editor_chunk(EditorChunk {
                l: l_val,
                r: r_val,
                pos_l: pos as u32,
                pos_r: pos as u32,
            });
            pos += EDITOR_CHUNK_SAMPLES;
        }
        data.poll_editor();
    }

    #[test]
    fn editor_stream_places_chunks_into_bins() {
        let data = InputData::default();
        // active_len == bins: chunk at pos i lands in bin i.
        data.active_len_l.store(EDITOR_VIS_SIZE, Relaxed);
        data.active_len_r.store(EDITOR_VIS_SIZE, Relaxed);
        data.push_editor_chunk(EditorChunk { l: 0.75, r: 0.25, pos_l: 0, pos_r: 0 });
        data.push_editor_chunk(EditorChunk {
            l: 0.5,
            r: 1.5,
            pos_l: (EDITOR_VIS_SIZE - 1) as u32,
            pos_r: (EDITOR_VIS_SIZE - 1) as u32,
        });
        data.poll_editor();
        assert_eq!(data.editor_l[0].load(Relaxed), 0.75);
        assert_eq!(data.editor_r[0].load(Relaxed), 0.25);
        // Hot feedback clamps at the shadow, like the old scan did.
        assert_eq!(data.editor_l[EDITOR_VIS_SIZE - 1].load(Relaxed), 0.5);
        assert_eq!(data.editor_r[EDITOR_VIS_SIZE - 1].load(Relaxed), 1.0);
    }

    #[test]
    fn editor_stream_keeps_channels_separate() {
        let data = InputData::default();
        feed_revolution(&data, 32768, 0.8, 0.2);
        // A full revolution fills every bin of both shadows.
        for cell in data.editor_l.iter() {
            assert!((cell.load(Relaxed) - 0.8).abs() < 1e-6);
        }
        for cell in data.editor_r.iter() {
            assert!((cell.load(Relaxed) - 0.2).abs() < 1e-6);
        }
    }

    #[test]
    fn editor_stream_clears_shadow_on_length_change() {
        let data = InputData::default();
        feed_revolution(&data, 32768, 0.9, 0.9);
        assert_eq!(data.editor_l[0].load(Relaxed), 0.9);
        data.active_len_l.store(4096, Relaxed);
        data.active_len_r.store(4096, Relaxed);
        data.poll_editor();
        assert_eq!(data.editor_l[0].load(Relaxed), 0.0);
        assert_eq!(data.editor_r[0].load(Relaxed), 0.0);
    }

    #[test]
    fn editor_uniforms_differ_by_channel_buffer_and_color() {
        let data = InputData::default();
        data.editor_l[0].store(0.75, Relaxed);
        data.editor_r[0].store(0.25, Relaxed);
        let l = data.editor_buffer_uniform(EditorChannel::Left).expect("uniforms");
        let r = data.editor_buffer_uniform(EditorChannel::Right).expect("uniforms");
        assert_eq!(l.levels[0][0], 0.75);
        assert_eq!(r.levels[0][0], 0.25);
        assert_ne!(l.col, r.col);
    }
}
