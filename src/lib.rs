use crate::filters::shifter::FrequencyShifter;
use crate::delay_engine::delay_time_from_bpm_and_16th;
use crate::filter_pipeline::pipeline::FilterPipeline;
use crate::slint_ui::editor::DelaxSlintHost;
use crate::slint_ui::plug_con::editor::SlintEditor;
use delay_engine::{
    engine::{DelayEngine, DelayInterpolationMode},
    params::DelayMode,
};
use filters::peak_follower::PeakFollower;
use filters::simper::SimperSinSVF;
use nice_plug::prelude::*;
use params::DelaxParams;
use slint_ui::connection::InputData;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use crate::filters::dattorro::DattorroReverb;

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
    decay_time_s_l: f32,
    decay_time_s_r: f32,
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
        filter_pipeline.register_stereo_pair(
            Box::new(SimperSinSVF::new(44100.)),
            Box::new(SimperSinSVF::new(44100.)),
            "svf_filter",
        );
        filter_pipeline.register_stereo_pair(
            Box::new(FrequencyShifter::new(44100., 0.)),
            Box::new(FrequencyShifter::new(44100., 0.)),
            "shimmer",
        );
        filter_pipeline.register_stereo(
            Box::new(DattorroReverb::new(0.5, 44100., 0.2, 0.0, 0.7, 0.8, 0.65, 0.8, 8., 1.1)),
            "dattorro"
        );

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
            filter_pipeline,
            decay_time_s_l: 0.5,
            decay_time_s_r: 0.5,
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
        let host = Arc::new(DelaxSlintHost::new(
            self.params.clone(),
            self.input_data.clone(),
        ));
        let (w, h) = self.params.editor_state.size();
        Some(SlintEditor::new(
            host,
            baseview::dpi::PhysicalSize::new(w, h),
            self.params.editor_state.title.clone(),
        ))
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

        self.filter_pipeline
            .set_param("svf_filter", "sample_rate", self.sample_rate);
        self.filter_pipeline
            .set_param("shimmer", "sample_rate", self.sample_rate);
        self.filter_pipeline
            .set_param("dattorro", "sample_rate", self.sample_rate);
        self.input_sin_svf_l.set_sample_rate(self.sample_rate);
        self.input_sin_svf_r.set_sample_rate(self.sample_rate);

        self.peak_in_l.set_sample_rate(self.sample_rate);
        self.peak_in_r.set_sample_rate(self.sample_rate);
        self.peak_out_l.set_sample_rate(self.sample_rate);
        self.peak_out_r.set_sample_rate(self.sample_rate);

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
            let (pop_left, pop_right) =
                self.run_bank_filters(pop_left, pop_right);
            // ####### Feedback loop #########
            // The feedback values, used for the feedback loop.
            let feedbacked_left;
            let feedbacked_right;
            let (fb_l, fb_r);

            match self.params.delay_params.stereo_delay.value() {
                DelayMode::Mono => {
                    fb_l = self.params.delay_params.feedback_l.smoothed.next();
                    fb_r = fb_l;
                    feedbacked_left = fb_l * pop_left;
                    feedbacked_right = fb_l * pop_right;
                }
                DelayMode::Stereo => {
                    fb_l = self.params.delay_params.feedback_l.smoothed.next();
                    fb_r = self.params.delay_params.feedback_r.smoothed.next();
                    feedbacked_left = fb_l * pop_left;
                    feedbacked_right = fb_r * pop_right;
                }
                DelayMode::PingPong => {
                    fb_l = self.params.delay_params.feedback_l.smoothed.next();
                    fb_r = self.params.delay_params.feedback_r.smoothed.next();
                    feedbacked_left = fb_r * pop_left;
                    feedbacked_right = fb_l * pop_right;
                }
            }

            let stereo_mode = self.params.delay_params.stereo_delay.value();
            let is_stereo = stereo_mode != DelayMode::Mono;
            let is_ping_pong = stereo_mode == DelayMode::PingPong;
            self.input_data.set_decay_state(
                fb_l,
                fb_r,
                self.decay_time_s_l,
                self.decay_time_s_r,
                is_stereo,
                is_ping_pong,
                self.params.delay_params.bpm_bound_l.value(),
                self.params.delay_params.bpm_bound_r.value(),
            );

            // ############ Filtering ###############

            // Run the signal through the filters

            // Mix the feedback and filtered signal together
            // Make the filtered output more stable by using the feedback param as well
            let (input_left, input_right) = self.run_input_filters(*left_sample, *right_sample);
            self.left_delay_engine
                .write_sample(input_left + feedbacked_left);
            self.right_delay_engine
                .write_sample(input_right + feedbacked_right);

            // ########### Output ##########
            let wetness = self.params.wetness.smoothed.next();

            *left_sample = *left_sample * (1. - wetness) + pop_left * wetness;
            *right_sample = *right_sample * (1. - wetness) + pop_right * wetness;

            let wet_l = *left_sample;
            let wet_r = *right_sample;
            self.input_data.push_wet(pop_left, pop_right);
            self.input_data
                .push_spectrum((pop_left * wetness + pop_right * wetness) * 0.5);

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
                let ms_l = self.params.delay_params.delay_len_l.smoothed.next();
                let ms_r = self.params.delay_params.delay_len_r.smoothed.next();
                let count_l = self.params.delay_params.delay_note_l.smoothed.next();
                let _count_r = self.params.delay_params.delay_note_r.smoothed.next();
                let _ = (ms_r, _count_r);
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

                self.decay_time_s_l = delay_amt / 1000.;
                self.decay_time_s_r = delay_amt / 1000.;

                let res = self.params.svf_params.input_svf_res_l.smoothed.next();
                let cutoff = self.params.svf_params.input_svf_cutoff_l.smoothed.next();
                let mode = self.params.svf_params.input_svf_filter_mode_l.value();

                self.input_sin_svf_l.set_res(res);
                self.input_sin_svf_r.set_res(res);
                self.input_sin_svf_l.set_cutoff(cutoff);
                self.input_sin_svf_l.set_mix(1.);
                self.input_sin_svf_r.set_cutoff(cutoff);
                self.input_sin_svf_l.set_mode(mode);
                self.input_sin_svf_r.set_mode(mode);
                self.input_sin_svf_r.set_mix(1.);
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

                self.decay_time_s_l = delay_amt_l / 1000.;
                self.decay_time_s_r = delay_amt_r / 1000.;

                let res_l = self.params.svf_params.input_svf_res_l.smoothed.next();
                let res_r = self.params.svf_params.input_svf_res_r.smoothed.next();
                let cutoff_l = self.params.svf_params.input_svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.svf_params.input_svf_cutoff_r.smoothed.next();
                let mode_l = self.params.svf_params.input_svf_filter_mode_l.value();
                let mode_r = self.params.svf_params.input_svf_filter_mode_r.value();

                self.input_sin_svf_l.set_res(res_l);
                self.input_sin_svf_r.set_res(res_r);
                self.input_sin_svf_l.set_cutoff(cutoff_l);
                self.input_sin_svf_r.set_cutoff(cutoff_r);
                self.input_sin_svf_l.set_mode(mode_l);
                self.input_sin_svf_r.set_mode(mode_r);
                self.input_sin_svf_l.set_mix(1.);
                self.input_sin_svf_r.set_mix(1.);
            }
        }
        let dattorro_mix = self.params.dattorro_params.mix.smoothed.next();
        let dattorro_size = self.params.dattorro_params.size.smoothed.next();
        let dattorro_decay = self.params.dattorro_params.decay.smoothed.next();
        let dattorro_pre_delay = self.params.dattorro_params.pre_delay.smoothed.next();
        let dattorro_damping = self.params.dattorro_params.damping.smoothed.next();
        let dattorro_brightness = self.params.dattorro_params.brightness.smoothed.next();
        let dattorro_lushness = self.params.dattorro_params.lushness.smoothed.next();
        let dattorro_input_smear = self.params.dattorro_params.input_smear.smoothed.next();
        let dattorro_tank_smear = self.params.dattorro_params.tank_smear.smoothed.next();

        self.filter_pipeline.set_param("dattorro", "mix", dattorro_mix);
        self.filter_pipeline.set_param("dattorro", "size", dattorro_size);
        self.filter_pipeline.set_param("dattorro", "decay", dattorro_decay);
        self.filter_pipeline.set_param("dattorro", "pre_delay", dattorro_pre_delay);
        self.filter_pipeline.set_param("dattorro", "damping", dattorro_damping);
        self.filter_pipeline.set_param("dattorro", "brightness", dattorro_brightness);
        self.filter_pipeline.set_param("dattorro", "lushness", dattorro_lushness);
        self.filter_pipeline.set_param("dattorro", "input_smear", dattorro_input_smear);
        self.filter_pipeline.set_param("dattorro", "tank_smear", dattorro_tank_smear);
        match self.params.svf_params.svf_stereo_mode.value() {
            filters::params::SVFStereoMode::Mono => {
                // `smoothed.next()` once for mono – keeps L/R smoothers in sync.
                let res = self.params.svf_params.svf_res_l.smoothed.next();
                let cutoff = self.params.svf_params.svf_cutoff_l.smoothed.next();
                let mode = self.params.svf_params.svf_filter_mode_l.modulated_normalized_value();
                let mix = self.params.svf_params.svf_mix_l.value();
                self.filter_pipeline.set_param("svf_filter", "res", res);
                self.filter_pipeline
                    .set_param("svf_filter", "cutoff", cutoff);
                self.filter_pipeline.set_param("svf_filter", "mix", mix);
                self.filter_pipeline.set_param("svf_filter", "mode", mode);

            }
            filters::params::SVFStereoMode::Stereo => {
                let res_l = self.params.svf_params.svf_res_l.smoothed.next();
                let res_r = self.params.svf_params.svf_res_r.smoothed.next();
                let cutoff_l = self.params.svf_params.svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.svf_params.svf_cutoff_r.smoothed.next();
                let mix_l = self.params.svf_params.svf_mix_l.smoothed.next();
                let mix_r = self.params.svf_params.svf_mix_r.smoothed.next();
                let mode_l = self.params.svf_params.svf_filter_mode_l.modulated_normalized_value();
                let mode_r = self.params.svf_params.svf_filter_mode_r.modulated_normalized_value();
                self.filter_pipeline
                    .set_param_stereo("svf_filter", "res", (res_l, res_r));
                self.filter_pipeline
                    .set_param_stereo("svf_filter", "cutoff", (cutoff_l, cutoff_r));
                self.filter_pipeline
                    .set_param_stereo("svf_filter", "mix", (mix_l, mix_r));
                self.filter_pipeline
                    .set_param_stereo("svf_filter", "mode", (mode_l, mode_r));

            }
        }
        self.filter_pipeline.set_active("svf_filter", self.params.pipeline_params.eq_active.value());
        self.filter_pipeline.set_active("diffusor", self.params.pipeline_params.diffusor_active.value());

        if self.params.shimmer_params.shimmer_stereo.value() {
            let shift_l = self.params.shimmer_params.shift_l.value();
            let shift_r = self.params.shimmer_params.shift_r.value();

            self.filter_pipeline.set_param_stereo("shimmer", "shift", (shift_l, shift_r));
        } else {
            let shift_l = self.params.shimmer_params.shift_l.value();

            self.filter_pipeline.set_param("shimmer", "shift", shift_l);
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
        self.input_data.wetness.store(self.params.wetness.value(), Relaxed);
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
