pub mod dattorro;
pub mod params;
pub mod simper;
pub mod peak_follower;

pub trait Filter: Send + Sync {
    fn process(&mut self, input: f32) -> f32;
}

pub trait StereoFilter: Send + Sync {
    fn process_stereo(&mut self, input_l: f32, input_r: f32) -> (f32, f32);
}

#[inline]
pub(crate) fn flush_denormal(x: f32) -> f32 {
    if x.abs() < 1e-30 { 0.0 } else { x }
}