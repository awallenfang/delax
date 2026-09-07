use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use nice_plug::prelude::AtomicF32;
use nice_plug::util;
use nice_plug::util::window::hann;
use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex32;
use wgpu::Buffer;
use crate::slint_ui;
use crate::slint_ui::elements::{ElementId, GpuElementData};
use crate::slint_ui::uniforms::{BufferUniforms, SpectrumUniforms};

pub struct InputData {
    pub in_l: AtomicF32,
    pub in_r: AtomicF32,
    pub out_l: AtomicF32,
    pub out_r: AtomicF32,
    pub out_spectrum: [AtomicF32; 32],
    pub bpm: AtomicF32,
    pub dry_buffer: [AtomicF32; 128],
    pub wet_buffer: [AtomicF32; 128],
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
            dry_buffer: [const { AtomicF32::new(0.) }; 128],
            wet_buffer: [const { AtomicF32::new(0.) }; 128],
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
            spectrum_pending: std::sync::Mutex::new(Vec::with_capacity(128)),
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
        app.set_in_level_l(self.in_l.load(Relaxed));
        app.set_in_level_r(self.in_r.load(Relaxed));
        app.set_out_level_l(self.out_l.load(Relaxed));
        app.set_out_level_r(self.out_r.load(Relaxed));
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
                    let excess = pending.len() - 128;
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
        app.set_out_spectrum(slint::ModelRc::new(slint::VecModel::from(spectrum.to_vec())));
    }

    fn poll_waveforms(&self, app: &slint_ui::AppWindow) {
        let dry_pos = self.dry_pos.load(Relaxed) % self.dry_buffer.len();
        let wet_pos = self.wet_pos.load(Relaxed) % self.wet_buffer.len();
        let mut dry = [0.0f32; 128];
        let mut wet = [0.0f32; 128];
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
        let mut dry = [0.0f32; 128];
        let mut wet = [0.0f32; 128];
        for i in 0..128 {
            dry[i] = self.dry_buffer[i].load(Relaxed);
            wet[i] = self.wet_buffer[i].load(Relaxed);
        }
        Some(BufferUniforms {
            levels_dry: dry,
            levels_wet: wet,
            // #ffd60a
            primary_col: [1.0, 0.6724, 0.003, 1.0],
            secondary_col: [0.0, 0.56, 0.73, 1.0],
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
            _ => None,
        }
    }
}