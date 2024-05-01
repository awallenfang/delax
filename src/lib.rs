use delay_engine::engine::DelayEngine;
use nih_plug::prelude::*;
use std::sync::Arc;

mod delay_engine;

pub struct Delax {
    params: Arc<DelaxParams>,
    left_delay_engine: DelayEngine,
    right_delay_engine: DelayEngine,
    sample_rate: f32
}

#[derive(Params)]
struct DelaxParams {
    #[id = "delay"]
    pub delay_len: FloatParam,
    #[id = "feedback"]
    pub feedback: FloatParam
}

impl Default for Delax {
    fn default() -> Self {
        let mut left_delay_engine = DelayEngine::new(44100);
        left_delay_engine.set_delay_amount(500);
        let mut right_delay_engine = DelayEngine::new(44100);
        right_delay_engine.set_delay_amount(400);

        Self {
            params: Arc::new(DelaxParams::default()),
            left_delay_engine,
            right_delay_engine,
            sample_rate: 44100.
        }
    }
}


impl Default for DelaxParams {
    fn default() -> Self {
        Self {
            delay_len: FloatParam::new(
                "Delay",
                5.,
                FloatRange::Skewed { min: 0., max: 1000., factor: 0.5},
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            feedback: FloatParam::new(
                "Feedback",
                0.5,
                FloatRange::Linear { min: 0., max: 1. },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
        }
    }
}

impl Plugin for Delax {
    const NAME: &'static str = "Delax";
    const VENDOR: &'static str = "Ava Wallenfang";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
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
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = _buffer_config.sample_rate;
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
            let delay_amt = seconds_to_samples(self.params.delay_len.smoothed.next(), self.sample_rate);

            self.left_delay_engine.set_delay_amount(delay_amt);
            self.right_delay_engine.set_delay_amount(delay_amt);


            // Read it sample by sample for now
            let mut channel_iter = channel_samples.into_iter();

            let left_sample = channel_iter.next().unwrap();
            let right_sample = channel_iter.next().unwrap();

            

            *left_sample = *left_sample + self.params.feedback.smoothed.next() * self.left_delay_engine.pop_sample();
            *right_sample = *right_sample + self.params.feedback.smoothed.next() * self.right_delay_engine.pop_sample();

            self.left_delay_engine.write_sample_unchecked(*left_sample);
            self.right_delay_engine.write_sample_unchecked(*right_sample);
        }


        ProcessStatus::Normal
    }
}

impl ClapPlugin for Delax {
    const CLAP_ID: &'static str = "com.your-domain.delax";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A short description of your plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for Delax {
    const VST3_CLASS_ID: [u8; 16] = *b"Exactly16Chars!!";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(Delax);
nih_export_vst3!(Delax);


pub fn seconds_to_samples(ms: f32, sample_rate: f32) -> usize {
    ((ms / 1000.) * sample_rate).floor() as usize
}