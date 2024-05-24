pub mod dattorro;
pub mod params;
pub mod simper;
pub trait Filter: Send + Sync {
    fn process(&mut self, input: f32) -> f32;
}

pub trait StereoFilter: Send + Sync {
    fn process_stereo(&mut self, input_l: f32, input_r: f32) -> (f32, f32);
}
