pub struct PeakFollower {
    pub release: f32,
    pub peak: f32,
    pub hold: f32,
    pub hold_counter: f32,
    peak_smoother: PeakSmoother,
}

impl PeakFollower {
    pub fn new(release: f32, hold: f32, smoothing: f32) -> Self {
        Self {
            release,
            peak: 0.,
            hold,
            hold_counter: 0.,
            peak_smoother: PeakSmoother::new(smoothing),
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input = self.peak_smoother.process(input.abs());
        if input.abs() > self.peak {
            self.peak = input;
            self.hold_counter = self.hold;
        } else {
            self.hold_counter -= 1.;
            if self.hold_counter < 0. {
                self.peak -= self.release;
            }
        }

        self.peak
    }
}

struct PeakSmoother {
    prev: f32,
    smoothness: f32
}

impl PeakSmoother {
    pub fn new(smooth: f32) -> Self {
        Self {
            prev: 0.,
            smoothness: smooth
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let smoothed = self.prev + (input - self.prev) * self.smoothness;
        self.prev = smoothed;
        smoothed
    }
}
