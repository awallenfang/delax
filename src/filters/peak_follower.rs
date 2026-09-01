use crate::filters::flush_denormal;

pub struct PeakFollower {
    /// Release amount per sample (linear fall, 0..1.5 range). E.g. 0.0008 at 44.1k ≈ 35/sec.
    pub release: f32,
    pub peak: f32,
    /// Hold time in seconds.
    pub hold: f32,
    pub hold_counter: f32,
    sample_rate: f32,
    peak_smoother: PeakSmoother,
}

impl PeakFollower {
    pub fn new(release: f32, hold: f32, sample_rate: f32, smoothing: f32) -> Self {
        Self {
            release,
            peak: 0.,
            hold,
            hold_counter: 0.,
            sample_rate,
            peak_smoother: PeakSmoother::new(smoothing),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() < f32::EPSILON || !sample_rate.is_finite() || sample_rate <= 0. {
            self.sample_rate = sample_rate;
            return;
        }
        let old_sr = self.sample_rate;
        self.hold_counter = self.hold_counter * sample_rate / old_sr;
        self.release = self.release * old_sr / sample_rate;
        self.peak_smoother.rescale(old_sr, sample_rate);
        self.sample_rate = sample_rate;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input = self.peak_smoother.process(input.abs());
        if input > self.peak {
            self.peak = input;
            self.hold_counter = self.hold * self.sample_rate;
        } else if self.hold_counter > 0. {
            self.hold_counter -= 1.;
        } else {
            self.peak = (self.peak - self.release).max(0.);
        }

        self.peak
    }
}

struct PeakSmoother {
    prev: f32,
    smoothness: f32,
}

impl PeakSmoother {
    pub fn new(smooth: f32) -> Self {
        Self {
            prev: 0.,
            smoothness: smooth.clamp(0., 1.),
        }
    }

    fn rescale(&mut self, old_sr: f32, new_sr: f32) {
        if (old_sr - new_sr).abs() < f32::EPSILON {
            return;
        }
        let d_old = (1.0 - self.smoothness).clamp(0., 1.);
        if d_old == 0. || d_old == 1. {
            return;
        }
        let ratio = old_sr / new_sr;
        let d_new = d_old.powf(ratio);
        self.smoothness = (1.0 - d_new).clamp(0., 1.);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if input < self.prev {
            let smoothed = self.prev + (input - self.prev) * self.smoothness;
            self.prev = flush_denormal(smoothed);
            smoothed
        } else {
            self.prev = input;
            input
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PeakFollower, PeakSmoother};

    #[test]
    fn peak_follower_holds_then_releases() {
        let mut pf = PeakFollower::new(0.0008, 0.1, 44100., 0.2);
        let peak = pf.process(1.0);
        assert!((peak - 1.0).abs() < 1e-5);

        for _ in 0..4400 {
            let p = pf.process(0.0);
            assert!((p - 1.0).abs() < 1e-5, "should hold, got {p}");
        }
        for _ in 0..20 {
            pf.process(0.0);
        }
        let after = pf.process(0.0);
        assert!(after < 1.0, "should have decayed, got {after}");
        assert!(after > 0.98, "decay too fast, got {after}");
    }

    #[test]
    fn peak_follower_release_rate() {
        let mut pf = PeakFollower::new(0.01, 0., 44100., 1.0); // smoothness 1.0 = no smoothing
        pf.process(1.0);
        let mut last = 1.0;
        for i in 0..10 {
            let p = pf.process(0.0);
            assert!(p < last, "should decay each sample after hold, iter {i}: {p} >= {last}");
            assert!((last - p - 0.01).abs() < 1e-5, "release step 0.01, got {}", last - p);
            last = p;
        }
        // After 100 steps should be ~0
        for _ in 0..90 {
            pf.process(0.0);
        }
        assert!(pf.peak < 1e-5, "should decay to 0, got {}", pf.peak);
    }

    #[test]
    fn peak_follower_larger_input_resets_hold() {
        let mut pf = PeakFollower::new(0.0008, 0.05, 44100., 1.0);
        pf.process(0.5);
        for _ in 0..1000 {
            pf.process(0.0);
        }
        assert!(pf.peak > 0.49, "should still hold around 0.5");
        let p2 = pf.process(1.0);
        assert!((p2 - 1.0).abs() < 1e-5);
        assert!((pf.hold_counter - 0.05 * 44100.).abs() < 1e-3);
        for _ in 0..2000 {
            let p = pf.process(0.0);
            assert!((p - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn peak_follower_abs_input() {
        let mut pf = PeakFollower::new(0.01, 0., 44100., 1.0);
        let p_pos = pf.process(0.5);
        let mut pf2 = PeakFollower::new(0.01, 0., 44100., 1.0);
        let p_neg = pf2.process(-0.5);
        assert!((p_pos - p_neg).abs() < 1e-5, "should handle negative via abs");
    }

    #[test]
    fn peak_follower_hold_scales_with_sample_rate() {
        let sr_low = 44100.;
        let sr_high = 96000.;
        let hold = 0.1;
        let mut pf_low = PeakFollower::new(0.0008, hold, sr_low, 1.0);
        let mut pf_high = PeakFollower::new(0.0008, hold, sr_high, 1.0);
        pf_low.process(1.0);
        pf_high.process(1.0);
        assert_eq!(pf_low.hold_counter, hold * sr_low);
        assert_eq!(pf_high.hold_counter, hold * sr_high);
        assert!((pf_high.hold_counter / pf_low.hold_counter - sr_high / sr_low).abs() < 1e-5);

        let mut count_low = 0;
        while pf_low.hold_counter > 0. {
            pf_low.process(0.0);
            count_low += 1;
        }
        let mut count_high = 0;
        while pf_high.hold_counter > 0. {
            pf_high.process(0.0);
            count_high += 1;
        }
        assert_eq!(count_low, (hold * sr_low) as i32);
        assert_eq!(count_high, (hold * sr_high) as i32);
    }

    #[test]
    fn peak_smoother_only_smooths_falling() {
        let mut sm = PeakSmoother::new(0.5);
        assert_eq!(sm.process(0.0), 0.0);
        assert_eq!(sm.process(1.0), 1.0);
        assert!((sm.process(0.0) - 0.5).abs() < 1e-5);
        assert!((sm.process(0.0) - 0.25).abs() < 1e-5);
        assert_eq!(sm.process(1.0), 1.0);
    }

    #[test]
    fn peak_smoother_smoothness_zero_or_one() {
        let mut sm_zero = PeakSmoother::new(0.0);
        sm_zero.process(1.0);
        assert!((sm_zero.process(0.0) - 1.0).abs() < 1e-5);

        let mut sm_one = PeakSmoother::new(1.0);
        sm_one.process(1.0);
        assert!((sm_one.process(0.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn peak_follower_set_sample_rate_preserves_hold_wall_time() {
        let mut pf = PeakFollower::new(0.0008, 0.1, 44100., 1.0);
        pf.process(1.0);
        assert_eq!(pf.hold_counter, 4410.);
        for _ in 0..2205 {
            pf.process(0.0);
        }
        assert!((pf.hold_counter - 2205.).abs() < 1e-3);
        pf.set_sample_rate(96000.);
        assert!((pf.hold_counter - 4800.).abs() < 1e-3, "hold_counter should rescale to 4800, got {}", pf.hold_counter);
        pf.peak = 0.;
        pf.hold_counter = 0.;
        pf.process(1.0);
        assert_eq!(pf.hold_counter, 9600.);
    }

    #[test]
    fn peak_follower_set_sample_rate_preserves_release_wall_time() {
        let mut pf = PeakFollower::new(0.0008, 0., 44100., 1.0);
        let release_per_sec_old = pf.release * 44100.;
        pf.set_sample_rate(96000.);
        let release_per_sec_new = pf.release * 96000.;
        assert!((release_per_sec_old - release_per_sec_new).abs() < 1e-5, "release_per_sec should be invariant: {release_per_sec_old} vs {release_per_sec_new}");
        let mut pf_low = PeakFollower::new(0.0008, 0., 44100., 1.0);
        let mut pf_high = PeakFollower::new(0.0008, 0., 96000., 1.0);
        pf_low.process(1.0);
        pf_high.process(1.0);
        for _ in 0..4410 { pf_low.process(0.0); }
        for _ in 0..9600 { pf_high.process(0.0); }
        assert!((pf_low.peak - pf_high.peak).abs() < 0.01, "release wall-time mismatch low={} high={}", pf_low.peak, pf_high.peak);

        let mut pf_chain = PeakFollower::new(0.0008, 0., 44100., 1.0);
        pf_chain.set_sample_rate(96000.);
        pf_chain.set_sample_rate(48000.);
        let mut pf_direct = PeakFollower::new(0.0008, 0., 44100., 1.0);
        pf_direct.set_sample_rate(48000.);
        assert!((pf_chain.release - pf_direct.release).abs() < 1e-7);
    }

    #[test]
    fn peak_smoother_rescale_preserves_wall_time() {
        let mut sm_low = PeakSmoother::new(0.2);
        sm_low.process(1.0);
        for _ in 0..4410 { sm_low.process(0.0); }
        let low_tail = sm_low.prev;

        let mut sm_high = PeakSmoother::new(0.2);
        sm_high.rescale(44100., 96000.);
        sm_high.process(1.0);
        for _ in 0..9600 { sm_high.process(0.0); }
        let high_tail = sm_high.prev;

        assert!((low_tail - high_tail).abs() < 1e-3, "smoother tails differ low={low_tail} high={high_tail}");
        let mut sm_chain = PeakSmoother::new(0.2);
        sm_chain.rescale(44100., 96000.);
        sm_chain.rescale(96000., 48000.);
        let mut sm_direct = PeakSmoother::new(0.2);
        sm_direct.rescale(44100., 48000.);
        assert!((sm_chain.smoothness - sm_direct.smoothness).abs() < 1e-6);
    }
}
