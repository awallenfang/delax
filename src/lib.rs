use crate::delay_engine::delay_time_from_bpm_and_16th;
use delay_engine::{
    engine::{DelayEngine, DelayInterpolationMode},
    params::DelayMode,
};
use filters::peak_follower::PeakFollower;
use filters::simper::SimperSinSVF;
use nice_plug::prelude::*;
use nice_plug::util::window::hann;
use params::DelaxParams;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use slint::{PhysicalSize, SharedString};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicU8, AtomicU16, AtomicUsize};
use slint_ui::connection::InputData;
use crate::filter_pipeline::pipeline::FilterPipeline;
use crate::slint_ui::editor_new::DelaxSlintHost;
use crate::slint_ui::plug_con::editor::SlintEditor;

mod delay_engine;
mod filter_pipeline;
pub mod filters;
mod params;
mod slint_ui;





pub struct Delax {
    params: Arc<DelaxParams>,
    left_delay_engine: DelayEngine,
    right_delay_engine: DelayEngine,
    sample_rate: f32,
    input_sin_svf_l: SimperSinSVF,
    input_sin_svf_r: SimperSinSVF,
    input_data: Arc<InputData>,
    peak_in_l: PeakFollower,
    peak_in_r: PeakFollower,
    peak_out_l: PeakFollower,
    peak_out_r: PeakFollower,
    filter_pipeline: FilterPipeline,
}

impl Default for Delax {
    fn default() -> Self {
        // 20 seconds buffer to accommodate long BPM-synced delays (e.g. 32 1/4 notes at 60 BPM = 8s, 32 half notes = 64s clamped to 20s covers most musical use)
        let default_buf = 44100 * 20;
        let mut left_delay_engine = DelayEngine::new(default_buf, 44100.);
        left_delay_engine.set_delay_amount(0.);
        let mut right_delay_engine = DelayEngine::new(default_buf, 44100.);
        right_delay_engine.set_delay_amount(0.);

        let mut filter_pipeline = FilterPipeline::new();
        filter_pipeline.register_stereo_pair(Box::new(SimperSinSVF::new(44100.)), Box::new(SimperSinSVF::new(44100.)), "svf_filter");

        Self {
            params: Arc::new(DelaxParams::default()),
            left_delay_engine,
            right_delay_engine,
            sample_rate: 44100.,
            input_sin_svf_l: SimperSinSVF::new(44100.),
            input_sin_svf_r: SimperSinSVF::new(44100.),
            input_data: Arc::new(InputData::default()),
            peak_in_l: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_in_r: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_out_l: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            peak_out_r: PeakFollower::new(0.0008, 0.1, 44100., 0.2),
            filter_pipeline
        }
    }
}

fn sync_params_to_ui(params: &DelaxParams, app: &slint_ui::AppWindow) {
    use slint_ui::param_component::ParamComponent;
    for (p_id, param_ptr, _) in params.param_map().iter() {
        let val = unsafe { param_ptr.unmodulated_normalized_value() };
        let display_val = unsafe { param_ptr.normalized_value_to_string(val, true) };
        <slint_ui::AppWindow as ParamComponent<DelaxParams>>::set_param_from_host(
            app,
            p_id,
            val,
            SharedString::from(display_val),
        );
    }
    // TODO: Very dirty way of generating the labels. This should be done together somewhere with the params
    let count_l = params.delay_params.delay_note_l.value();
    let div_l = params.delay_params.delay_div_l.value();
    let factor_l = div_l.factor();
    let suffix_l = div_l.suffix();
    let display_l = {
        let c = (count_l * 10.0).round() / 10.0;
        if c.fract().abs() < 0.0005 {
            format!("{} {}", c as i32, suffix_l)
        } else {
            format!("{:.1} {}", c, suffix_l)
        }
    };
    app.set_timing_display_l(display_l.into());
    app.set_timing_factor_l(factor_l);
    let count_r = params.delay_params.delay_note_r.value();
    let div_r = params.delay_params.delay_div_r.value();
    let factor_r = div_r.factor();
    let suffix_r = div_r.suffix();
    let display_r = {
        let c = (count_r * 10.0).round() / 10.0;
        if c.fract().abs() < 0.0005 {
            format!("{} {}", c as i32, suffix_r)
        } else {
            format!("{:.1} {}", c, suffix_r)
        }
    };
    app.set_timing_display_r(display_r.into());
    app.set_timing_factor_r(factor_r);
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

    type Editor = SlintEditor<DelaxSlintHost>;
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
        let host = Arc::new(DelaxSlintHost::new(self.params.clone(), self.input_data.clone()));
        let (w, h) = self.params.editor_state.size();
        Some(SlintEditor::new(host, baseview::dpi::PhysicalSize::new(w,h), self.params.editor_state.title.clone()))
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

        let buffer_size = (self.sample_rate * 20.0) as usize;
        let mut left_delay_engine = DelayEngine::new(buffer_size, self.sample_rate);
        left_delay_engine.set_delay_amount(0.);
        let mut right_delay_engine = DelayEngine::new(buffer_size, self.sample_rate);
        right_delay_engine.set_delay_amount(0.);

        self.left_delay_engine = left_delay_engine;
        self.right_delay_engine = right_delay_engine;

        self.filter_pipeline.set_param("svf_filter", "sample_rate", self.sample_rate);
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
        self.input_data.reset();
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

            let dry_l = *left_sample;
            let dry_r = *right_sample;
            self.input_data.push_dry(dry_l, dry_r);
            self.input_ui_send(dry_l, dry_r);

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
                DelayMode::PingPong => {
                    let feedback_l = self.params.delay_params.feedback_l.smoothed.next();
                    let feedback_r = self.params.delay_params.feedback_r.smoothed.next();
                    feedbacked_left = feedback_r * pop_left;
                    feedbacked_right = feedback_l * pop_right;
                }
            }

            // ############ Filtering ###############

            // Run the signal through the filters
            let (filtered_output_l, filtered_output_r) =
                self.run_bank_filters(feedbacked_left, feedbacked_right);

            // Mix the feedback and filtered signal together
            // Make the filtered output more stable by using the feedback param as well
            let (input_left, input_right) = self.run_input_filters(*left_sample, *right_sample);
            self.left_delay_engine.write_sample(
                input_left + filtered_output_l,
            );
            self.right_delay_engine.write_sample(
                input_right + filtered_output_r,
            );

            // ########### Output ##########
            let wetness = self.params.wetness.smoothed.next();

            *left_sample = *left_sample * (1. - wetness) + pop_left * wetness;
            *right_sample = *right_sample * (1. - wetness) + pop_right * wetness;

            let wet_l = *left_sample;
            let wet_r = *right_sample;
            self.input_data.push_wet(pop_left, pop_right);
            self.input_data.push_spectrum((pop_left * wetness + pop_right * wetness) * 0.5);

            self.output_ui_send(wet_l, wet_r);
        }

        ProcessStatus::Normal
    }
}

impl Delax {
    fn update_params(&mut self, transport: &Transport) {
        self.input_data
            .set_bpm(transport.tempo.unwrap_or(120.) as f32);
        match self.params.delay_params.stereo_delay.value() {
            DelayMode::Mono => {
                // Advance all smoothers so they stay in sync when switching modes.
                let ms_l = self.params.delay_params.delay_len_l.smoothed.next();
                let ms_r = self.params.delay_params.delay_len_r.smoothed.next();
                let count_l = self.params.delay_params.delay_note_l.smoothed.next();
                let _count_r = self.params.delay_params.delay_note_r.smoothed.next();
                let _ = (ms_r, _count_r);
                // Advance div smoothers not needed – EnumParam not smoothed; just read value
                let div_factor_l = self.params.delay_params.delay_div_l.value().factor();
                let bpm_bound = self.params.delay_params.bpm_bound_l.value();
                let mut bpm = 120.;
                if let Some(t) = transport.tempo {
                    bpm = t;
                }
                let delay_amt = if bpm_bound {
                    let total_l = count_l * div_factor_l;
                    delay_time_from_bpm_and_16th(total_l, bpm as f32)
                } else {
                    ms_l
                };
                self.left_delay_engine.set_delay_amount(delay_amt);
                self.right_delay_engine.set_delay_amount(delay_amt);

                let res = self.params.filter_params.input_svf_res_l.smoothed.next();
                let cutoff = self.params.filter_params.input_svf_cutoff_l.smoothed.next();
                let mode = self.params.filter_params.input_svf_filter_mode_l.value();

                self.input_sin_svf_l.set_res(res);
                self.input_sin_svf_r.set_res(res);
                self.input_sin_svf_l.set_cutoff(cutoff);
                self.input_sin_svf_r.set_cutoff(cutoff);
                self.input_sin_svf_l.set_mode(mode);
                self.input_sin_svf_r.set_mode(mode);
            }
            DelayMode::Stereo | DelayMode::PingPong => {
                let ms_l = self.params.delay_params.delay_len_l.smoothed.next();
                let ms_r = self.params.delay_params.delay_len_r.smoothed.next();
                let count_l = self.params.delay_params.delay_note_l.smoothed.next();
                let count_r = self.params.delay_params.delay_note_r.smoothed.next();
                let div_factor_l = self.params.delay_params.delay_div_l.value().factor();
                let div_factor_r = self.params.delay_params.delay_div_r.value().factor();
                let bpm_bound_l = self.params.delay_params.bpm_bound_l.value();
                let bpm_bound_r = self.params.delay_params.bpm_bound_r.value();
                let mut bpm = 120.;
                if let Some(t) = transport.tempo {
                    bpm = t;
                }
                let delay_amt_l = if bpm_bound_l {
                    let total_l = count_l * div_factor_l;
                    delay_time_from_bpm_and_16th(total_l, bpm as f32)
                } else {
                    ms_l
                };
                let delay_amt_r = if bpm_bound_r {
                    let total_r = count_r * div_factor_r;
                    delay_time_from_bpm_and_16th(total_r, bpm as f32)
                } else {
                    ms_r
                };
                self.left_delay_engine.set_delay_amount(delay_amt_l);
                self.right_delay_engine.set_delay_amount(delay_amt_r);

                let res_l = self.params.filter_params.input_svf_res_l.smoothed.next();
                let res_r = self.params.filter_params.input_svf_res_r.smoothed.next();
                let cutoff_l = self.params.filter_params.input_svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.filter_params.input_svf_cutoff_r.smoothed.next();
                let mode_l = self.params.filter_params.input_svf_filter_mode_l.value();
                let mode_r = self.params.filter_params.input_svf_filter_mode_r.value();

                self.input_sin_svf_l.set_res(res_l);
                self.input_sin_svf_r.set_res(res_r);
                self.input_sin_svf_l.set_cutoff(cutoff_l);
                self.input_sin_svf_r.set_cutoff(cutoff_r);
                self.input_sin_svf_l.set_mode(mode_l);
                self.input_sin_svf_r.set_mode(mode_r);
            }
        }

        match self.params.filter_params.svf_stereo_mode.value() {
            filters::params::SVFStereoMode::Mono => {
                // `smoothed.next()` once for mono – keeps L/R smoothers in sync.
                let res = self.params.filter_params.svf_res_l.smoothed.next();
                let cutoff = self.params.filter_params.svf_cutoff_l.smoothed.next();
                let mode = self.params.filter_params.svf_filter_mode_l.value();
                let mix = self.params.filter_params.svf_mix_l.value();
                self.filter_pipeline.set_param("svf_filter", "res", res);
                self.filter_pipeline.set_param("svf_filter", "cutoff", cutoff);
                self.filter_pipeline.set_param("svf_filter", "mix", mix);

                self.input_sin_svf_l.set_cutoff(cutoff);
                self.input_sin_svf_r.set_cutoff(cutoff);
                self.input_sin_svf_l.set_mode(mode);
                self.input_sin_svf_r.set_mode(mode);
            }
            filters::params::SVFStereoMode::Stereo => {
                let res_l = self.params.filter_params.svf_res_l.smoothed.next();
                let res_r = self.params.filter_params.svf_res_r.smoothed.next();
                let cutoff_l = self.params.filter_params.svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.filter_params.svf_cutoff_r.smoothed.next();
                let mix_l = self.params.filter_params.svf_mix_l.smoothed.next();
                let mix_r = self.params.filter_params.svf_mix_r.smoothed.next();
                let mode_l = self.params.filter_params.svf_filter_mode_l.value();
                let mode_r = self.params.filter_params.svf_filter_mode_r.value();
                self.filter_pipeline.set_param_stereo("svf_filter", "res", (res_l ,res_r));
                self.filter_pipeline.set_param_stereo("svf_filter", "cutoff", (cutoff_l, cutoff_r));
                self.filter_pipeline.set_param_stereo("svf_filter", "mix", (mix_l, mix_r));

                self.input_sin_svf_l.set_res(res_l);
                self.input_sin_svf_r.set_res(res_r);
                self.input_sin_svf_l.set_cutoff(cutoff_l);
                self.input_sin_svf_r.set_cutoff(cutoff_r);
                self.input_sin_svf_l.set_mode(mode_l);
                self.input_sin_svf_r.set_mode(mode_r);
            }
        }
    }

    /// Run the current filter chain. Input is the stereo signal, output is the resulting stereo signal.
    fn run_bank_filters(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.filter_pipeline.process_stereo(input_l, input_r)
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
        //ClapFeature::Distortion,
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
