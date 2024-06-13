use nih_plug::prelude::*;

#[derive(Debug, Enum, PartialEq, Clone, Copy)]
pub enum SVFFilterMode {
    Low,
    Band,
    High,
    Notch,
    Peak,
}

#[derive(Enum, PartialEq)]
pub enum SVFStereoMode {
    Mono,
    Stereo,
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "svf_cutoff_l"]
    pub svf_cutoff_l: FloatParam,
    #[id = "svf_cutoff_r"]
    pub svf_cutoff_r: FloatParam,
    #[id = "svf_res_l"]
    pub svf_res_l: FloatParam,
    #[id = "svf_res_r"]
    pub svf_res_r: FloatParam,
    #[id = "svf_filter_mode_l"]
    pub svf_filter_mode_l: EnumParam<SVFFilterMode>,
    #[id = "svf_filter_mode_r"]
    pub svf_filter_mode_r: EnumParam<SVFFilterMode>,
    #[id = "svf_stereo_mode"]
    pub svf_stereo_mode: EnumParam<SVFStereoMode>,
    #[id = "svf_mix_l"]
    pub svf_mix_l: FloatParam,
    #[id = "svf_mix_r"]
    pub svf_mix_r: FloatParam,
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            svf_cutoff_l: FloatParam::new(
                "SVF Cutoff",
                500.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 20000.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(2))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            svf_cutoff_r: FloatParam::new(
                "SVF Cutoff Channel 2",
                500.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 20000.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(2))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            svf_res_l: FloatParam::new("SVF Res", 0.2, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.)),
            svf_res_r: FloatParam::new(
                "SVF Res Channel 2",
                0.2,
                FloatRange::Linear { min: 0., max: 1. },
            )
            .with_smoother(SmoothingStyle::Linear(50.)),
            svf_filter_mode_l: EnumParam::new("SVF Filter Mode", SVFFilterMode::Band),
            svf_filter_mode_r: EnumParam::new("SVF Filter Mode Channel 2", SVFFilterMode::Band),
            svf_stereo_mode: EnumParam::new("SVF Seperated", SVFStereoMode::Mono),
            svf_mix_l: FloatParam::new("Mix", 1., FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.)),
            svf_mix_r: FloatParam::new(
                "Mix Channel 2",
                1.,
                FloatRange::Linear { min: 0., max: 1. },
            )
            .with_smoother(SmoothingStyle::Linear(50.)),
        }
    }
}
