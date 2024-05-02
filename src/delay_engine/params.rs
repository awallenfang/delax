use nih_plug::prelude::*;

#[derive(Enum, PartialEq)]
pub enum DelayMode {
    Mono,
    Stereo,
}

#[derive(Params)]
pub struct EngineParams {
    #[id = "delay_l"]
    pub delay_len_l: FloatParam,
    #[id = "delay_r"]
    pub delay_len_r: FloatParam,
    #[id = "feedback_l"]
    pub feedback_l: FloatParam,
    #[id = "feedback_r"]
    pub feedback_r: FloatParam,
    #[id = "stereo"]
    pub stereo_delay: EnumParam<DelayMode>,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            delay_len_l: FloatParam::new(
                "Delay",
                500.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 1000.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            delay_len_r: FloatParam::new(
                "Delay Channel 2",
                500.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 1000.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            feedback_l: FloatParam::new("Feedback", 0.5, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            feedback_r: FloatParam::new(
                "Feedback Channel 2",
                0.5,
                FloatRange::Linear { min: 0., max: 1. },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            stereo_delay: EnumParam::new("Seperate Delay", DelayMode::Mono),
        }
    }
}
