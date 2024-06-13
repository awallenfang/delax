use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::{delay_engine::params::EngineParams, filters::params::FilterParams, ui};

#[derive(Params)]
pub struct DelaxParams {
    #[nested(group = "Delay Parameters")]
    pub delay_params: EngineParams,
    #[nested(group = "Filter Parameters")]
    pub filter_params: FilterParams,
    #[id = "wetness"]
    pub wetness: FloatParam,

    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,
}

impl Default for DelaxParams {
    fn default() -> Self {
        Self {
            delay_params: EngineParams::default(),
            filter_params: FilterParams::default(),
            wetness: FloatParam::new("Wetness", 0.5, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.)),
            editor_state: ui::default_state(),
        }
    }
}
