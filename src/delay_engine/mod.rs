pub mod engine;
pub mod params;

pub fn delay_time_from_bpm_and_16th(note_val: f32, bpm: f32) -> f32 {
    // note_val is in units of 16th notes. One 16th = 1/4 beat.
    // Beat duration = 60/BPM seconds; multiply by note_val/4 and convert to ms.
    note_val * 60. / bpm.clamp(0.1, 10000.) / 4. * 1000.
}
#[allow(dead_code)]
pub fn notes_from_bpm_and_delay_time(delay_time: f32, bpm: f32) -> f32 {
    // delay_time is in ms – convert to 16th-note count.
    let bpm = bpm.clamp(0.1, 10000.);
    delay_time / (60. / bpm / 4. * 1000.)
}

#[cfg(test)]
mod tests {
    use super::{delay_time_from_bpm_and_16th, notes_from_bpm_and_delay_time};

    const EPS: f32 = 1e-4;

    #[test]
    fn delay_time_basic_values() {
        assert!((delay_time_from_bpm_and_16th(4., 120.) - 500.).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(1., 120.) - 125.).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(2., 120.) - 250.).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(16., 120.) - 2000.).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(4., 60.) - 1000.).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(1., 60.) - 250.).abs() < EPS);
    }

    #[test]
    fn delay_time_zero_note_is_zero() {
        assert!((delay_time_from_bpm_and_16th(0., 120.)).abs() < EPS);
        assert!((delay_time_from_bpm_and_16th(0., 60.)).abs() < EPS);
    }

    #[test]
    fn bpm_clamping_low() {
        let t = delay_time_from_bpm_and_16th(4., 0.);
        let expected = 4. * 60. / 0.1 / 4. * 1000.;
        assert!((t - expected).abs() < EPS);
        assert!(t.is_finite());

        let t2 = delay_time_from_bpm_and_16th(4., -100.);
        assert!((t2 - expected).abs() < EPS);
    }

    #[test]
    fn bpm_clamping_high() {
        let t = delay_time_from_bpm_and_16th(4., 20000.);
        let expected = 4. * 60. / 10000. / 4. * 1000.;
        assert!((t - expected).abs() < EPS);

        let t2 = delay_time_from_bpm_and_16th(4., 10000.);
        assert!((t - t2).abs() < EPS);
    }

    #[test]
    fn notes_from_delay_time_basic() {
        assert!((notes_from_bpm_and_delay_time(500., 120.) - 4.).abs() < EPS);
        assert!((notes_from_bpm_and_delay_time(125., 120.) - 1.).abs() < EPS);
        assert!((notes_from_bpm_and_delay_time(1000., 60.) - 4.).abs() < EPS);
    }

    #[test]
    fn round_trip_delay_to_notes_and_back() {
        let bpms = [60., 90., 120., 140., 180.];
        let notes = [0.5, 1., 2., 4., 8., 16., 32.];
        for bpm in bpms {
            for note in notes {
                let delay = delay_time_from_bpm_and_16th(note, bpm);
                let back = notes_from_bpm_and_delay_time(delay, bpm);
                assert!(
                    (back - note).abs() < 1e-3,
                    "round-trip failed for note={note} bpm={bpm}: delay={delay} back={back}"
                );
                let back2 = delay_time_from_bpm_and_16th(back, bpm);
                assert!(
                    (back2 - delay).abs() < 1e-3,
                    "round-trip 2 failed for note={note} bpm={bpm}"
                );
            }
        }
    }

    #[test]
    fn notes_clamping_consistent() {
        let delay_low_bpm = delay_time_from_bpm_and_16th(4., 0.);
        let notes_low_bpm = notes_from_bpm_and_delay_time(delay_low_bpm, 0.);
        assert!((notes_low_bpm - 4.).abs() < EPS);

        let delay_high_bpm = delay_time_from_bpm_and_16th(4., 20000.);
        let notes_high_bpm = notes_from_bpm_and_delay_time(delay_high_bpm, 20000.);
        assert!((notes_high_bpm - 4.).abs() < EPS);
    }

    #[test]
    fn delay_time_monotonic_in_note_and_inverse_in_bpm() {
        let a = delay_time_from_bpm_and_16th(2., 120.);
        let b = delay_time_from_bpm_and_16th(4., 120.);
        assert!(b > a);

        let c = delay_time_from_bpm_and_16th(4., 60.);
        let d = delay_time_from_bpm_and_16th(4., 120.);
        assert!(c > d);
    }
}
