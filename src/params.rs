use std::sync::Arc;

use nice_plug::prelude::*;

use crate::slint_ui::editor::EditorState;
use crate::{delay_engine::params::EngineParams, filters::params::SVFParams};
use crate::filter_pipeline::params::PipelineParams;
use crate::filters::dattorro::DattorroParams;
use crate::filters::shifter::ShifterParams;

#[derive(Params)]
pub struct DelaxParams {
    #[nested(group = "Delay Parameters")]
    pub delay_params: EngineParams,
    #[nested(group = "EQ Parameters")]
    pub svf_params: SVFParams,
    #[nested(group = "Diffusor Parameters")]
    pub dattorro_params: DattorroParams,
    #[nested(group = "Pipeline Parameters")]
    pub pipeline_params: PipelineParams,
    #[nested(group = "Shimmer Parameters")]
    pub shimmer_params: ShifterParams,
    #[id = "wetness"]
    pub wetness: FloatParam,
    #[persist = "editor-state"]
    pub editor_state: Arc<EditorState>,
}

impl Default for DelaxParams {
    fn default() -> Self {
        Self {
            delay_params: EngineParams::default(),
            dattorro_params: DattorroParams::default(),
            svf_params: SVFParams::default(),
            pipeline_params: PipelineParams::default(),
            shimmer_params: ShifterParams::default(),
            wetness: FloatParam::new("Wetness", 0.5, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            editor_state: Arc::new(EditorState::new(550, 350)),
        }
    }
}
