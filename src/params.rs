use nih_plug::prelude::*;

use crate::{delay_engine::params::EngineParams, filters::params::FilterParams};

#[derive(Params)]
pub struct DelaxParams {
    #[nested(group = "Delay Parameters")]
    pub delay_params: EngineParams,
    #[nested(group = "Filter Parameters")]
    pub filter_params: FilterParams,
    #[id = "wetness"]
    pub wetness: FloatParam,
}

impl Default for DelaxParams {
    fn default() -> Self {
        Self {
            delay_params: Default::default(),
            filter_params: Default::default(),
            wetness: FloatParam::new("Wetness", 0.5, FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.)),
        }
    }
}
