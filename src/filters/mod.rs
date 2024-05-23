pub mod params;
pub mod simper;

pub trait Filter: Send + Sync {
    fn process(&mut self, input: f32) -> f32;
}
