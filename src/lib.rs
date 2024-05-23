use delay_engine::{
    engine::{DelayEngine, DelayInterpolationMode},
    params::DelayMode,
};
use filter_pipeline::pipeline::FilterPipeline;
use filters::simper::SimperSinSVF;
use nih_plug::prelude::*;
use params::DelaxParams;
use std::sync::{Arc, Mutex};

mod delay_engine;
mod filter_pipeline;
pub mod filters;
mod params;

pub struct Delax {
    params: Arc<DelaxParams>,
    left_delay_engine: DelayEngine,
    right_delay_engine: DelayEngine,
    sample_rate: f32,
    sin_svf_l: SimperSinSVF,
    sin_svf_r: SimperSinSVF,
    input_sin_svf_l: SimperSinSVF,
    input_sin_svf_r: SimperSinSVF,
    filter_pipeline: FilterPipeline,
    initial_filter_pipeline: FilterPipeline,
}

impl Default for Delax {
    fn default() -> Self {
        let mut left_delay_engine = DelayEngine::new(44100, 44100.);
        left_delay_engine.set_delay_amount(0.);
        let mut right_delay_engine = DelayEngine::new(44100, 44100.);
        right_delay_engine.set_delay_amount(0.);

        let input_sin_svf_l = SimperSinSVF::new(44100.);
        let input_sin_svf_r = SimperSinSVF::new(44100.);

        let sin_svf_l = SimperSinSVF::new(44100.);
        let sin_svf_r = SimperSinSVF::new(44100.);

        Self {
            params: Arc::new(DelaxParams::default()),
            left_delay_engine,
            right_delay_engine,
            sample_rate: 44100.,
            sin_svf_l,
            sin_svf_r,
            input_sin_svf_l,
            input_sin_svf_r,
            filter_pipeline: FilterPipeline::new(),
            initial_filter_pipeline: FilterPipeline::new(),
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

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
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

        self.filter_pipeline.register_stereo_pair(
            Arc::new(Mutex::new(self.sin_svf_l.clone())),
            Arc::new(Mutex::new(self.sin_svf_r.clone())),
        );
        self.initial_filter_pipeline.register_stereo_pair(
            Arc::new(Mutex::new(self.input_sin_svf_l.clone())),
            Arc::new(Mutex::new(self.input_sin_svf_r.clone())),
        );
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.left_delay_engine.reset();
        self.right_delay_engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            // Update all the elements to the current params
            self.update_params();

            // ########## Input ###########
            // Read the values sample by sample for now
            let mut channel_iter = channel_samples.into_iter();

            // If for some reason the iterator is empty something went very wrong and ig a panic is in order
            let left_sample = channel_iter.next().unwrap();
            let right_sample = channel_iter.next().unwrap();

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
        }

        ProcessStatus::Normal
    }
}

impl Delax {
    fn update_params(&mut self) {
        match self.params.delay_params.stereo_delay.value() {
            DelayMode::Mono => {
                let delay_amt = self.params.delay_params.delay_len_l.smoothed.next();

                self.left_delay_engine.set_delay_amount(delay_amt);
                self.right_delay_engine.set_delay_amount(delay_amt);
            }
            DelayMode::Stereo => {
                let delay_amt_l = self.params.delay_params.delay_len_l.smoothed.next();
                let delay_amt_r = self.params.delay_params.delay_len_r.smoothed.next();
                self.left_delay_engine.set_delay_amount(delay_amt_l);
                self.right_delay_engine.set_delay_amount(delay_amt_r);
            }
        }

        // Update the filter params
        match self.params.filter_params.svf_stereo_mode.value() {
            // For mono params it's important to just call the params function once. Otherwise the smoothing is out of sync
            filters::params::SVFStereoMode::Mono => {
                let res = self.params.filter_params.svf_res_l.smoothed.next();
                self.sin_svf_l.set_res(res);
                self.sin_svf_r.set_res(res);
                self.input_sin_svf_l.set_res(res);
                self.input_sin_svf_r.set_res(res);

                let cutoff = self.params.filter_params.svf_cutoff_l.smoothed.next();
                self.sin_svf_l.set_cutoff(cutoff);
                self.sin_svf_r.set_cutoff(cutoff);
                self.input_sin_svf_l.set_cutoff(cutoff);
                self.input_sin_svf_r.set_cutoff(cutoff);

                let mode = self.params.filter_params.svf_filter_mode_l.value();
                self.sin_svf_l.set_mode(mode);
                self.sin_svf_r.set_mode(mode);
            }
            filters::params::SVFStereoMode::Stereo => {
                let res_l = self.params.filter_params.svf_res_l.smoothed.next();
                let res_r = self.params.filter_params.svf_res_r.smoothed.next();

                self.sin_svf_l.set_res(res_l);
                self.sin_svf_r.set_res(res_r);

                let cutoff_l = self.params.filter_params.svf_cutoff_l.smoothed.next();
                let cutoff_r = self.params.filter_params.svf_cutoff_r.smoothed.next();
                self.sin_svf_l.set_cutoff(cutoff_l);
                self.sin_svf_r.set_cutoff(cutoff_r);

                let mode_l = self.params.filter_params.svf_filter_mode_l.value();
                let mode_r = self.params.filter_params.svf_filter_mode_r.value();
                self.sin_svf_l.set_mode(mode_l);
                self.sin_svf_r.set_mode(mode_r);
            }
        }
    }

    /// Run the current filter chain. Input is the stereo signal, output is the resulting stereo signal.
    fn run_filters(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.filter_pipeline.process_stereo(input_l, input_r)
    }

    /// Run the filter chain on the input signal. This can probably be refactored out down the line. But for now it doesn't work correctly without
    fn run_input_filters(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.initial_filter_pipeline.process_stereo(input_l, input_r)
    }
}

impl ClapPlugin for Delax {
    const CLAP_ID: &'static str = "com.ritzin-dev.delax";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A short description of your plugin");
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

nih_export_clap!(Delax);
// nih_export_vst3!(Delax);
