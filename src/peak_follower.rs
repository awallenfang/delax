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
            self.prev = smoothed;
            smoothed
        } else {
            self.prev = input;
            input
        }
    }
}
