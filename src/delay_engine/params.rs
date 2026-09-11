use nice_plug::prelude::*;

use crate::delay_engine::engine::MAX_DELAY_SECS;

#[derive(Enum, PartialEq)]
pub enum DelayMode {
    Mono,
    Stereo,
    PingPong
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum NoteDiv {
    Sixteenth,
    Eighth,
    Quarter,
    Half,
}

impl NoteDiv {
    pub fn factor(self) -> f32 {
        match self {
            NoteDiv::Sixteenth => 1.0,
            NoteDiv::Eighth => 2.0,
            NoteDiv::Quarter => 4.0,
            NoteDiv::Half => 8.0,
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            NoteDiv::Sixteenth => "16th",
            NoteDiv::Eighth => "8th",
            NoteDiv::Quarter => "1/4",
            NoteDiv::Half => "1/2",
        }
    }
    pub fn from_factor(f: f32) -> Self {
        if f < 1.5 {
            NoteDiv::Sixteenth
        } else if f < 3.0 {
            NoteDiv::Eighth
        } else if f < 6.0 {
            NoteDiv::Quarter
        } else {
            NoteDiv::Half
        }
    }
    pub fn from_norm(norm: f32) -> Self {
        if norm < 0.166 {
            NoteDiv::Sixteenth
        } else if norm < 0.5 {
            NoteDiv::Eighth
        } else if norm < 0.833 {
            NoteDiv::Quarter
        } else {
            NoteDiv::Half
        }
    }
    pub fn to_norm(self) -> f32 {
        match self {
            NoteDiv::Sixteenth => 0.0,
            NoteDiv::Eighth => 0.333_333_34,
            NoteDiv::Quarter => 0.666_666_7,
            NoteDiv::Half => 1.0,
        }
    }
    pub fn to_norm_factor(f: f32) -> f32 {
        Self::from_factor(f).to_norm()
    }
}

#[derive(Params)]
pub struct EngineParams {
    #[id = "delay_l"]
    pub delay_len_l: FloatParam,
    #[id = "delay_r"]
    pub delay_len_r: FloatParam,
    #[id = "delay_note_l"]
    pub delay_note_l: FloatParam,
    #[id = "delay_note_r"]
    pub delay_note_r: FloatParam,
    #[id = "feedback_l"]
    pub feedback_l: FloatParam,
    #[id = "feedback_r"]
    pub feedback_r: FloatParam,
    #[id = "stereo"]
    pub stereo_delay: EnumParam<DelayMode>,
    #[id = "bpm_bound_l"]
    pub bpm_bound_l: BoolParam,
    #[id = "bpm_bound_r"]
    pub bpm_bound_r: BoolParam,
    #[id = "delay_div_l"]
    pub delay_div_l: EnumParam<NoteDiv>,
    #[id = "delay_div_r"]
    pub delay_div_r: EnumParam<NoteDiv>,
    #[id = "buffer_len_l"]
    pub buffer_len_l: FloatParam,
    #[id = "buffer_len_r"]
    pub buffer_len_r: FloatParam,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            delay_len_l: FloatParam::new(
                "Delay",
                500.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 5000.,
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
                    max: 5000.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            delay_note_l: FloatParam::new(
                "Delay Note L",
                2.,
                FloatRange::Linear { min: 0., max: 8. },
            )
            .with_step_size(0.5)
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" 16th")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            delay_note_r: FloatParam::new(
                "Delay Note R",
                2.,
                FloatRange::Linear { min: 0., max: 8. },
            )
            .with_step_size(0.5)
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" 16th")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

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
            bpm_bound_l: BoolParam::new("BPM Bound", false),
            bpm_bound_r: BoolParam::new("BPM Bound Channel 2", false),
            delay_div_l: EnumParam::new("Div L", NoteDiv::Sixteenth),
            delay_div_r: EnumParam::new("Div R", NoteDiv::Sixteenth),
            buffer_len_l: FloatParam::new(
                "Buffer Len L",
                5.,
                FloatRange::Linear {
                    min: 1.,
                    max: MAX_DELAY_SECS,
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            buffer_len_r: FloatParam::new(
                "Buffer Len R",
                5.,
                FloatRange::Linear {
                    min: 1.,
                    max: MAX_DELAY_SECS,
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
        }
    }
}
