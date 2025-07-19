pub mod engine;
pub mod params;


pub fn delay_time_from_bpm_and_16th(note_val: f32, bpm: f32) -> f32 {
    // beats per second * beats in noteval = length in seconds?
    (bpm / 60.) * (note_val / 4.)
}
pub fn notes_from_bpm_and_delay_time(delay_time: f32, bpm: f32) -> f32 {
    // delay_time / length of 16th note
    delay_time / ((bpm/60.) / 4.)
}

 