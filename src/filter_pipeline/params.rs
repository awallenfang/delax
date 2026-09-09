use nice_plug::prelude::*;
#[derive(Params)]
pub struct PipelineParams {
    #[id = "eq_active"]
    pub eq_active: BoolParam,
    #[id = "diffusor_active"]
    pub diffusor_active: BoolParam,
}

impl Default for PipelineParams {
    fn default() -> Self {
        Self {
            eq_active: BoolParam::new("eq_active", true),
            diffusor_active: BoolParam::new("diffusor_active", true),
        }
    }
}