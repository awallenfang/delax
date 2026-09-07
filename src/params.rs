use std::sync::Arc;

use nice_plug::prelude::*;

use crate::slint_ui::editor::EditorState;
use crate::{delay_engine::params::EngineParams, filters::params::FilterParams};

#[derive(Params)]
pub struct DelaxParams {
    #[nested(group = "Delay Parameters")]
    pub delay_params: EngineParams,
    #[nested(group = "Filter Parameters")]
    pub filter_params: FilterParams,
    #[id = "wetness"]
    pub wetness: FloatParam,
    #[persist = "editor-state"]
    pub editor_state: Arc<EditorState>,
}

impl Default for DelaxParams {
    fn default() -> Self {
        Self {
            delay_params: EngineParams::default(),
            filter_params: FilterParams::default(),
            wetness: FloatParam::new("Wetness", 0.5, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            editor_state: Arc::new(EditorState::new(550, 350)),
        }
    }
}
