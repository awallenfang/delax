use delay_engine::{engine::DelayEngine, params::DelayMode};
use filters::{params::SVFFilterMode, simper::SimperSinSVF};
use nih_plug::prelude::*;
use params::DelaxParams;
use std::sync::Arc;

mod delay_engine;
pub mod filters;
mod params;

pub struct Delax {
    params: Arc<DelaxParams>,
    left_delay_engine: DelayEngine,
    right_delay_engine: DelayEngine,
    sample_rate: f32,
    sin_svf_l: SimperSinSVF,
    sin_svf_r: SimperSinSVF,
}

impl Default for Delax {
    fn default() -> Self {
        let mut left_delay_engine = DelayEngine::new(44100);
        left_delay_engine.set_delay_amount(0);
        let mut right_delay_engine = DelayEngine::new(44100);
        right_delay_engine.set_delay_amount(0);

        let sin_svf_l = SimperSinSVF::new(44100.);
        let sin_svf_r = SimperSinSVF::new(44100.);

        Self {
            params: Arc::new(DelaxParams::default()),
            left_delay_engine,
            right_delay_engine,
            sample_rate: 44100.,
            sin_svf_l,
            sin_svf_r,
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

        let mut left_delay_engine = DelayEngine::new(self.sample_rate as usize);
        left_delay_engine.set_delay_amount(0);
        let mut right_delay_engine = DelayEngine::new(self.sample_rate as usize);
        right_delay_engine.set_delay_amount(0);

        self.left_delay_engine = left_delay_engine;
        self.right_delay_engine = right_delay_engine;

        self.sin_svf_l.set_sample_rate(self.sample_rate);
        self.sin_svf_r.set_sample_rate(self.sample_rate);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
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
            let pop_left = self.left_delay_engine.pop_sample();
            let pop_right = self.right_delay_engine.pop_sample();

            // ####### Feedback loop #########
            // The feedback values, used for the feedback loop.
            let feedbacked_left;
            let feedbacked_right;

            match self.params.delay_params.stereo_delay.value() {
                DelayMode::Mono => {
                    let feedback_l = self.params.delay_params.feedback_l.smoothed.next();
                    feedbacked_left = *left_sample + feedback_l * pop_left;
                    feedbacked_right = *right_sample + feedback_l * pop_right;
                }
                DelayMode::Stereo => {
                    let feedback_l = self.params.delay_params.feedback_l.smoothed.next();
                    let feedback_r = self.params.delay_params.feedback_r.smoothed.next();
                    feedbacked_left = *left_sample + feedback_l * pop_left;
                    feedbacked_right = *right_sample + feedback_r * pop_right;
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
            self.left_delay_engine
                .write_sample(feedbacked_left * (1. - mix_left) + filtered_output_l * mix_left);
            self.right_delay_engine
                .write_sample(feedbacked_right * (1. - mix_right) + filtered_output_r * mix_right);

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
                let delay_amt = seconds_to_samples(
                    self.params.delay_params.delay_len_l.smoothed.next(),
                    self.sample_rate,
                );

                self.left_delay_engine.set_delay_amount(delay_amt);
                self.right_delay_engine.set_delay_amount(delay_amt);
            }
            DelayMode::Stereo => {
                let delay_amt_l = seconds_to_samples(
                    self.params.delay_params.delay_len_l.smoothed.next(),
                    self.sample_rate,
                );
                let delay_amt_r = seconds_to_samples(
                    self.params.delay_params.delay_len_r.smoothed.next(),
                    self.sample_rate,
                );
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

                let cutoff = self.params.filter_params.svf_cutoff_l.smoothed.next();
                self.sin_svf_l.set_cutoff(cutoff);
                self.sin_svf_r.set_cutoff(cutoff);
            }
            filters::params::SVFStereoMode::Stereo => {
                self.sin_svf_l
                    .set_res(self.params.filter_params.svf_res_l.smoothed.next());
                self.sin_svf_r
                    .set_res(self.params.filter_params.svf_res_r.smoothed.next());

                self.sin_svf_l
                    .set_cutoff(self.params.filter_params.svf_cutoff_l.smoothed.next());
                self.sin_svf_r
                    .set_cutoff(self.params.filter_params.svf_cutoff_r.smoothed.next());
            }
        }
    }

    /// Run the current filter chain. Input is the stereo signal, output is the resulting stereo signal.
    fn run_filters(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let filtered_output_l;
        let filtered_output_r;

        match self.params.filter_params.svf_stereo_mode.value() {
            // In mono mode, set both to the left value
            filters::params::SVFStereoMode::Mono => {
                match self.params.filter_params.svf_filter_mode_l.value() {
                    SVFFilterMode::Low => {
                        (filtered_output_l, _, _) = self.sin_svf_l.tick_sample(input_left);
                        (filtered_output_r, _, _) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::Band => {
                        (_, filtered_output_l, _) = self.sin_svf_l.tick_sample(input_left);
                        (_, filtered_output_r, _) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::High => {
                        (_, _, filtered_output_l) = self.sin_svf_l.tick_sample(input_left);
                        (_, _, filtered_output_r) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::Notch => {
                        let (low_l, _, high_l) = self.sin_svf_l.tick_sample(input_left);
                        let (low_r, _, high_r) = self.sin_svf_r.tick_sample(input_right);
                        filtered_output_l = low_l + high_l;
                        filtered_output_r = low_r + high_r;
                    }
                    SVFFilterMode::Peak => {
                        let (low_l, _, high_l) = self.sin_svf_l.tick_sample(input_left);
                        let (low_r, _, high_r) = self.sin_svf_r.tick_sample(input_right);
                        filtered_output_l = low_l - high_l;
                        filtered_output_r = low_r - high_r;
                    }
                }
            }
            // In stereo mode set the seperately
            filters::params::SVFStereoMode::Stereo => {
                match self.params.filter_params.svf_filter_mode_l.value() {
                    SVFFilterMode::Low => {
                        (filtered_output_l, _, _) = self.sin_svf_l.tick_sample(input_left);
                    }
                    SVFFilterMode::Band => {
                        (_, filtered_output_l, _) = self.sin_svf_l.tick_sample(input_left);
                    }

                    SVFFilterMode::High => {
                        (_, _, filtered_output_l) = self.sin_svf_l.tick_sample(input_left);
                    }

                    SVFFilterMode::Notch => {
                        let (low, _, high) = self.sin_svf_l.tick_sample(input_left);
                        filtered_output_l = low + high;
                    }
                    SVFFilterMode::Peak => {
                        let (low, _, high) = self.sin_svf_l.tick_sample(input_left);
                        filtered_output_l = low - high;
                    }
                }
                match self.params.filter_params.svf_filter_mode_r.value() {
                    SVFFilterMode::Low => {
                        (filtered_output_r, _, _) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::Band => {
                        (_, filtered_output_r, _) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::High => {
                        (_, _, filtered_output_r) = self.sin_svf_r.tick_sample(input_right);
                    }
                    SVFFilterMode::Notch => {
                        let (low, _, high) = self.sin_svf_l.tick_sample(input_right);
                        filtered_output_r = low + high;
                    }
                    SVFFilterMode::Peak => {
                        let (low, _, high) = self.sin_svf_l.tick_sample(input_right);
                        filtered_output_r = low - high;
                    }
                }
            }
        }

        (filtered_output_l, filtered_output_r)
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

pub fn seconds_to_samples(ms: f32, sample_rate: f32) -> usize {
    ((ms / 1000.) * sample_rate).floor() as usize
}
