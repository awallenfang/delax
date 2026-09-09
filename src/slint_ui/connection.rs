use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU8, AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use nice_plug::prelude::AtomicF32;
use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex32;
use wgpu::Buffer;
use crate::slint_ui;
use crate::slint_ui::elements::{ElementId, GpuElementData};
use crate::slint_ui::HeaderData;
use crate::slint_ui::uniforms::{BufferUniforms, DecayUniforms, SpectrumUniforms};

pub const UI_BUFFER_SIZE: usize = 128;

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

    // Decay visualizer state (mirrors the old CPU DecayVisualizer params).
    pub feedback_l: AtomicF32,
    pub feedback_r: AtomicF32,
    pub time_s_l: AtomicF32,
    pub time_s_r: AtomicF32,
    pub is_stereo: AtomicU8,
    pub is_ping_pong: AtomicU8,
    pub bpm_bound_l: AtomicU8,
    pub bpm_bound_r: AtomicU8,

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
}

impl Default for InputData {
    fn default() -> Self {
        let mut fft_planner = FftPlanner::new();
        let fft_plan = fft_planner.plan_fft_forward(64);
        let (spec_prod, spec_cons) = rtrb::RingBuffer::new(4096);
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
        if let Ok(mut guard) = self.spectrum_consumer.try_lock() {
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

        self.poll_spectrum(app);
        self.poll_waveforms(app);
    }

    fn poll_spectrum(&self, app: &slint_ui::AppWindow) {
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
        // app.set_out_spectrum(slint::ModelRc::new(slint::VecModel::from(spectrum.to_vec())));
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

    pub fn buffer_uniform(&self) -> Option<BufferUniforms> {
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

        Some(BufferUniforms {
            levels_dry,
            levels_wet,
            primary_col: [1.0, 214./255., 10./256., 0.5],
            secondary_col: [0.0, 143./256., 186./256., 0.5],
            params: [self.wetness.load(Relaxed), 0., 0., 0.]
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
            // One whole bar of 4 beats = 240 / bpm seconds.
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
            ElementId::Decay => {
                let uniforms = self.decay_uniform()?;
                Some(bytemuck::bytes_of(&uniforms).to_vec())
            }
            _ => None,
        }
    }
}