use crate::delay_engine::delay_time_from_bpm_and_16th;
use delay_engine::{
    engine::{DelayEngine, DelayInterpolationMode},
    params::DelayMode,
};
use filters::peak_follower::PeakFollower;
use filters::simper::SimperSinSVF;
use nice_plug::prelude::*;
use params::DelaxParams;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use slint::SharedString;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

mod delay_engine;
mod filter_pipeline;
pub mod filters;
mod params;
mod slint_ui;

pub struct InputData {
    pub in_l: AtomicF32,
    pub in_r: AtomicF32,
    pub out_l: AtomicF32,
    pub out_r: AtomicF32,
    pub out_spectrum: [AtomicF32; 32],
}

impl Default for InputData {
    fn default() -> Self {
        Self {
            in_l: AtomicF32::new(0.),
            in_r: AtomicF32::new(0.),
            out_l: AtomicF32::new(0.),
            out_r: AtomicF32::new(0.),
            out_spectrum: [const { AtomicF32::new(0.) }; 32],
        }
    }
}

pub struct Delax {
    params: Arc<DelaxParams>,
    left_delay_engine: DelayEngine,
    right_delay_engine: DelayEngine,
    sample_rate: f32,
    sin_svf_l: SimperSinSVF,
    sin_svf_r: SimperSinSVF,
    input_sin_svf_l: SimperSinSVF,
    input_sin_svf_r: SimperSinSVF,
    input_data: Arc<InputData>,
    peak_in_l: PeakFollower,
    peak_in_r: PeakFollower,
    peak_out_l: PeakFollower,
    peak_out_r: PeakFollower,
    out_fft: Arc<dyn Fft<f32>>,
    spectrum_producer: Option<rtrb::Producer<f32>>,
    spectrum_consumer: Arc<std::sync::Mutex<Option<rtrb::Consumer<f32>>>>,
}

impl Default for Delax {
    fn default() -> Self {
        let mut left_delay_engine = DelayEngine::new(44100, 44100.);
        left_delay_engine.set_delay_amount(0.);
        let mut right_delay_engine = DelayEngine::new(44100, 44100.);
        right_delay_engine.set_delay_amount(0.);

        let mut fft_planner = FftPlanner::new();
        let fft_plan = fft_planner.plan_fft_forward(64);
        let (prod, cons) = rtrb::RingBuffer::new(4096);
        Self {
            params: Arc::new(DelaxParams::default()),
            left_delay_engine,
            right_delay_engine,
            sample_rate: 44100.,
            sin_svf_l: SimperSinSVF::new(44100.),
            sin_svf_r: SimperSinSVF::new(44100.),
            input_sin_svf_l: SimperSinSVF::new(44100.),
            input_sin_svf_r: SimperSinSVF::new(44100.),
            input_data: Arc::new(InputData::default()),
            peak_in_l: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_in_r: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_out_l: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_out_r: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            out_fft: fft_plan,
            spectrum_producer: Some(prod),
            spectrum_consumer: Arc::new(std::sync::Mutex::new(Some(cons))),
        }
    }
}

impl Plugin for Delax {
    const NAME: &'static str = "Delax";
    const VENDOR: &'static str = "Ava Wallenfang";
    const URL: &'static str = "https://ritzin.dev";
    const EMAIL: &'static str = "ava@wallenfang.de";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = slint_ui::editor::UIEditor<slint_ui::AppWindow, DelaxParams>;
    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();

    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        use slint_ui::param_component::ParamComponent;

        Some(
            slint_ui::editor::UIEditor::new(
                self.params.editor_state.clone(),
                Arc::new({
                    let params = self.params.clone();
                    move |event_tx| {
                        let app = slint_ui::AppWindow::new()?;
                        app.set_version(env!("CARGO_PKG_VERSION").into());
                        app.bind_param_changed(event_tx, params.clone());
                        Ok(app)
                    }
                }),
                self.params.clone(),
            )
            .on_frame({
                let params = self.params.clone();
                let input = self.input_data.clone();
                let spectrum_consumer = self.spectrum_consumer.clone();
                let fft = self.out_fft.clone();
                let fft_scratch = std::sync::Arc::new(std::sync::Mutex::new(vec![
                    Complex32::new(0.0, 0.0);
                    fft.get_inplace_scratch_len()
                ]));
                let hann_window: Vec<f32> = (0..64)
                    .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / 63.0).cos()))
                    .collect();
                let hann_window = std::sync::Arc::new(hann_window);
                let spectrum_buffer =
                    std::sync::Arc::new(std::sync::Mutex::new(Vec::<f32>::with_capacity(128)));
                let spectrum_buffer_clone = spectrum_buffer.clone();
                move |app| {
                    for (p_id, param_ptr, _) in params.param_map().iter() {
                        let val = unsafe { param_ptr.unmodulated_normalized_value() };
                        let display_val =
                            unsafe { param_ptr.normalized_value_to_string(val, true) };
                        <slint_ui::AppWindow as ParamComponent<DelaxParams>>::set_param_from_host(
                            app,
                            p_id,
                            val,
                            SharedString::from(display_val),
                        );
                    }
                    app.set_in_level_l(input.in_l.load(Relaxed));
                    app.set_in_level_r(input.in_r.load(Relaxed));
                    app.set_out_level_l(input.out_l.load(Relaxed));
                    app.set_out_level_r(input.out_r.load(Relaxed));

                    if let Ok(mut guard) = spectrum_consumer.try_lock() {
                        if let Some(cons) = guard.as_mut() {
                            if let Ok(mut buf) = spectrum_buffer_clone.try_lock() {
                                while let Ok(v) = cons.pop() {
                                    buf.push(v);
                                }
                                while buf.len() >= 64 {
                                    let mut samples: Vec<f32> = buf.drain(0..64).collect();
                                    for (s, w) in samples.iter_mut().zip(hann_window.iter()) {
                                        *s *= *w;
                                    }
                                    let mut complex: Vec<Complex32> = samples
                                        .into_iter()
                                        .map(|s| Complex32::new(s, 0.0))
                                        .collect();
                                    if let Ok(mut scratch) = fft_scratch.try_lock() {
                                        if scratch.len() == fft.get_inplace_scratch_len() {
                                            fft.process_with_scratch(&mut complex, &mut scratch);
                                        } else {
                                            fft.process(&mut complex);
                                        }
                                    } else {
                                        fft.process(&mut complex);
                                    }
                                    let spectrum: Vec<f32> = complex[0..32]
                                        .iter()
                                        .map(|c| {
                                            let mag = c.norm();
                                            let db = (1.0
                                                + util::gain_to_db_fast(mag.max(1e-5)) / 100.0)
                                                .clamp(0.0, 1.0);
                                            db
                                        })
                                        .collect();
                                    app.set_out_spectrum(slint::ModelRc::new(
                                        slint::VecModel::from(spectrum),
                                    ));
                                }
                                // Keep buffer bounded: drop oldest if host produced too fast
                                if buf.len() > 256 {
                                    let excess = buf.len() - 128;
                                    buf.drain(0..excess);
                                }
                            }
                        }
                    }
                }
            }),
        )
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = buffer_config.sample_rate;

        let mut left_delay_engine = DelayEngine::new(self.sample_rate as usize, self.sample_rate);
        left_delay_engine.set_delay_amount(0.);
        let mut right_delay_engine = DelayEngine::new(self.sample_rate as usize, self.sample_rate);
        right_delay_engine.set_delay_amount(0.);

        self.left_delay_engine = left_delay_engine;
        self.right_delay_engine = right_delay_engine;

        self.sin_svf_l.set_sample_rate(self.sample_rate);
        self.sin_svf_r.set_sample_rate(self.sample_rate);
        self.input_sin_svf_l.set_sample_rate(self.sample_rate);
        self.input_sin_svf_r.set_sample_rate(self.sample_rate);

        self.peak_in_l.set_sample_rate(self.sample_rate);
        self.peak_in_r.set_sample_rate(self.sample_rate);
        self.peak_out_l.set_sample_rate(self.sample_rate);
        self.peak_out_r.set_sample_rate(self.sample_rate);

        // self.filter_pipeline.register_stereo(Arc::new(Mutex::new(self.datorro.clone())));

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.left_delay_engine.reset();
        self.right_delay_engine.reset();
        self.peak_in_l.peak = 0.;
        self.peak_in_r.peak = 0.;
        self.peak_out_l.peak = 0.;
        self.peak_out_r.peak = 0.;
        self.peak_in_l.hold_counter = 0.;
        self.peak_in_r.hold_counter = 0.;
        self.peak_out_l.hold_counter = 0.;
        self.peak_out_r.hold_counter = 0.;
        self.input_data.in_l.store(0., Relaxed);
        self.input_data.in_r.store(0., Relaxed);
        self.input_data.out_l.store(0., Relaxed);
        self.input_data.out_r.store(0., Relaxed);
        for i in 0..32 {
            self.input_data.out_spectrum[i].store(0., Relaxed);
        }
        if let Ok(mut guard) = self.spectrum_consumer.try_lock() {
            if let Some(cons) = guard.as_mut() {
                while cons.pop().is_ok() {}
            }
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            // Update all the elements to the current params
            self.update_params(_context.transport());
            // ########## Input ###########
            // Read the values sample by sample for now
            let mut channel_iter = channel_samples.into_iter();

            // If for some reason the iterator is empty something went very wrong and ig a panic is in order
            let left_sample = channel_iter.next().unwrap();
            let right_sample = channel_iter.next().unwrap();

            self.input_ui_send(*left_sample, *right_sample);

            // The output of the banks
            let pop_left = self
                .left_delay_engine
                .interpolate_sample(DelayInterpolationMode::Nearest);
            let pop_right = self
                .right_delay_engine
                .interpolate_sample(DelayInterpolationMode::Nearest);

            // ####### Feedback loop #########
            // The feedback values, used for the feedback loop.
            let feedbacked_left;
            let feedbacked_right;

            match self.params.delay_params.stereo_delay.value() {
                DelayMode::Mono => {
                    let feedback_l = self.params.delay_params.feedback_l.smoothed.next();
                    feedbacked_left = feedback_l * pop_left;
                    feedbacked_right = feedback_l * pop_right;
                }
                DelayMode::Stereo => {
                    let feedback_l = self.params.delay_params.feedback_l.smoothed.next();
                    let feedback_r = self.params.delay_params.feedback_r.smoothed.next();
                    feedbacked_left = feedback_l * pop_left;
                    feedbacked_right = feedback_r * pop_right;
                }
            }

            // ############ Filtering ###############

            // Run the signal through the filters
            let (filtered_output_l, filtered_output_r) =
                self.run_filters(feedbacked_left, feedbacked_right);

            // ########### Mixing #######
            // Get the mix amount
            let mix_left;
            let mix_right;
            match self.params.filter_params.svf_stereo_mode.value() {
                filters::params::SVFStereoMode::Mono => {
                    let mix = self.params.filter_params.svf_mix_l.smoothed.next();
                    mix_left = mix;
                    mix_right = mix;
                }
                filters::params::SVFStereoMode::Stereo => {
                    mix_left = self.params.filter_params.svf_mix_l.smoothed.next();
                    mix_right = self.params.filter_params.svf_mix_r.smoothed.next();
                }
            }

            // Mix the feedback and filtered signal together
            // Make the filtered output more stable by using the feedback param as well
            let (input_left, input_right) = self.run_input_filters(*left_sample, *right_sample);
            self.left_delay_engine.write_sample(
                input_left + (feedbacked_left * (1. - mix_left) + filtered_output_l * mix_left),
            );
            self.right_delay_engine.write_sample(
                input_right + (feedbacked_right * (1. - mix_right) + filtered_output_r * mix_right),
            );

            // ########### Output ##########
            let wetness = self.params.wetness.smoothed.next();

            *left_sample = *left_sample * (1. - wetness) + pop_left * wetness;
            *right_sample = *right_sample * (1. - wetness) + pop_right * wetness;

            if let Some(prod) = self.spectrum_producer.as_mut() {
                let mono = (*left_sample + *right_sample) * 0.5;
                let _ = prod.push(mono); // drop if full — UI is slower (~60Hz vs 44.1kHz), backpressure is expected
            }

            self.output_ui_send(*left_sample, *right_sample);
        }

        ProcessStatus::Normal
    }
}

impl Delax {
    fn update_params(&mut self, transport: &Transport) {
        match self.params.delay_params.stereo_delay.value() {
            DelayMode::Mono => {
                // Advance all smoothers so they stay in sync when switching modes.
                let ms_l = self.params.delay_params.delay_len_l.smoothed.next();
                let ms_r = self.params.delay_params.delay_len_r.smoothed.next();
                let note_l = self.params.delay_params.delay_note_l.smoothed.next();
                let _note_r = self.params.delay_params.delay_note_r.smoothed.next();
                let _ = (ms_r, _note_r);
                let bpm_bound = self.params.delay_params.bpm_bound_l.value();
                let mut bpm = 120.;
                if let Some(t) = transport.tempo {
                    bpm = t;
                }
                let delay_amt = if bpm_bound {
                    delay_time_from_bpm_and_16th(note_l, bpm as f32)
                } else {
                    ms_l
                };
                self.left_delay_engine.set_delay_amount(delay_amt);
                self.right_delay_engine.set_delay_amount(delay_amt);
            }
            DelayMode::Stereo => {
                let ms_l = self.params.delay_params.delay_len_l.smoothed.next();
                let ms_r = self.params.delay_params.delay_len_r.smoothed.next();
                let note_l = self.params.delay_params.delay_note_l.smoothed.next();
                let note_r = self.params.delay_params.delay_note_r.smoothed.next();
                let bpm_bound_l = self.params.delay_params.bpm_bound_l.value();
                let bpm_bound_r = self.params.delay_params.bpm_bound_r.value();
                let mut bpm = 120.;
                if let Some(t) = transport.tempo {
                    bpm = t;
                }
                let delay_amt_l = if bpm_bound_l {
                    delay_time_from_bpm_and_16th(note_l, bpm as f32)
                } else {
                    ms_l
                };
                let delay_amt_r = if bpm_bound_r {
                    delay_time_from_bpm_and_16th(note_r, bpm as f32)
                } else {
                    ms_r
                };
                self.left_delay_engine.set_delay_amount(delay_amt_l);
                self.right_delay_engine.set_delay_amount(delay_amt_r);
            }
        }

        // Plugin owns SVFs directly – no pipeline indirection.
        match self.params.filter_params.svf_stereo_mode.value() {
            filters::params::SVFStereoMode::Mono => {
                // `smoothed.next()` once for mono – keeps L/R smoothers in sync.
                let res = self.params.filter_params.svf_res_l.smoothed.next();
                let cutoff = self.params.filter_params.svf_cutoff_l.smoothed.next();
                let mode = self.params.filter_params.svf_filter_mode_l.value();
                self.sin_svf_l.set_res(res);
                self.sin_svf_r.set_res(res);
                self.input_sin_svf_l.set_res(res);
                self.input_sin_svf_r.set_res(res);
                self.sin_svf_l.set_cutoff(cutoff);
                self.sin_svf_r.set_cutoff(cutoff);
                self.input_sin_svf_l.set_cutoff(cutoff);
                self.input_sin_svf_r.set_cutoff(cutoff);
                self.sin_svf_l.set_mode(mode);
                self.sin_svf_r.set_mode(mode);
                self.input_sin_svf_l.set_mode(mode);
                self.input_sin_svf_r.set_mode(mode);
            }
            filters::params::SVFStereoMode::Stereo => {
                let res_l = self.params.filter_params.svf_res_l.smoothed.next();
                let res_r = self.params.filter_params.svf_res_r.smoothed.next();
                self.sin_svf_l.set_res(res_l);
                self.sin_svf_r.set_res(res_r);
                self.input_sin_svf_l.set_res(res_l);
                self.input_sin_svf_r.set_res(res_r);
                let cutoff_l = self.params.filter_params.svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.filter_params.svf_cutoff_r.smoothed.next();
                self.sin_svf_l.set_cutoff(cutoff_l);
                self.sin_svf_r.set_cutoff(cutoff_r);
                self.input_sin_svf_l.set_cutoff(cutoff_l);
                self.input_sin_svf_r.set_cutoff(cutoff_r);
                let mode_l = self.params.filter_params.svf_filter_mode_l.value();
                let mode_r = self.params.filter_params.svf_filter_mode_r.value();
                self.sin_svf_l.set_mode(mode_l);
                self.sin_svf_r.set_mode(mode_r);
                self.input_sin_svf_l.set_mode(mode_l);
                self.input_sin_svf_r.set_mode(mode_r);
            }
        }
    }

    /// Run the current filter chain. Input is the stereo signal, output is the resulting stereo signal.
    fn run_filters(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        use filters::Filter;
        let l = self.sin_svf_l.process(input_l);
        let r = self.sin_svf_r.process(input_r);
        (l, r)
    }

    /// Run the filter chain on the input signal. This can probably be refactored out down the line. But for now it doesn't work correctly without
    fn run_input_filters(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        use filters::Filter;
        let l = self.input_sin_svf_l.process(input_l);
        let r = self.input_sin_svf_r.process(input_r);
        (l, r)
    }

    fn input_ui_send(&mut self, l: f32, r: f32) {
        // Convert to 0..1 dB range, then smooth with peak follower for stable UI meter
        let l_db = (1. + util::gain_to_db(l.abs()) / 100.).clamp(0., 1.5);
        let r_db = (1. + util::gain_to_db(r.abs()) / 100.).clamp(0., 1.5);
        let l = self.peak_in_l.process(l_db).clamp(0., 1.5);
        let r = self.peak_in_r.process(r_db).clamp(0., 1.5);

        self.input_data.in_l.store(l, Relaxed);
        self.input_data.in_r.store(r, Relaxed);
    }

    fn output_ui_send(&mut self, l: f32, r: f32) {
        let l_db = (1. + util::gain_to_db_fast(l.abs()) / 100.).clamp(0., 1.5);
        let r_db = (1. + util::gain_to_db_fast(r.abs()) / 100.).clamp(0., 1.5);
        let l = self.peak_out_l.process(l_db).clamp(0., 1.5);
        let r = self.peak_out_r.process(r_db).clamp(0., 1.5);

        self.input_data.out_l.store(l, Relaxed);
        self.input_data.out_r.store(r, Relaxed);
    }
}

impl ClapPlugin for Delax {
    const CLAP_ID: &'static str = "com.ritzin-dev.delax";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A for now simple delay plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Delay,
        ClapFeature::Distortion,
    ];
}

// impl Vst3Plugin for Delax {
//     const VST3_CLASS_ID: [u8; 16] = *b"Exactly16Chars!!";

//     // And also don't forget to change these categories
//     const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
//         Vst3SubCategory::Fx,
//         Vst3SubCategory::Delay,
//         Vst3SubCategory::Stereo,
//     ];
// }

nice_export_clap!(Delax);
// nih_export_vst3!(Delax);
