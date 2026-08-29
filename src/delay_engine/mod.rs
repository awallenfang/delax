pub mod engine;
pub mod params;

pub fn delay_time_from_bpm_and_16th(note_val: f32, bpm: f32) -> f32 {
    // note_val is in units of 16th notes. One 16th = 1/4 beat.
    // Beat duration = 60/BPM seconds; multiply by note_val/4 and convert to ms.
    note_val * 60. / bpm / 4. * 1000.
}
pub fn notes_from_bpm_and_delay_time(delay_time: f32, bpm: f32) -> f32 {
    // delay_time is in ms – convert to 16th-note count.
    delay_time / (60. / bpm / 4. * 1000.)
}
