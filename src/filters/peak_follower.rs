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
            smoothness: smooth,
        }
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
}
