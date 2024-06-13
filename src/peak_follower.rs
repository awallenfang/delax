pub struct PeakFollower {
    pub release: f32,
    pub peak: f32,
    pub hold: f32,
    pub hold_counter: f32,
    sample_rate: f32,
    peak_smoother: PeakSmoother,
}

impl PeakFollower {
    pub fn new(release: f32, hold: f32, sample_rate: f32, smoothing: usize) -> Self {
        Self {
            release,
            peak: 0.,
            hold,
            hold_counter: 0.,
            sample_rate,
            peak_smoother: PeakSmoother::new(smoothing),
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input = self.peak_smoother.process(input.abs());
        if input.abs() > self.peak {
            self.peak = input;
            self.hold_counter = self.hold;
        } else {
            self.hold_counter -= 1. / self.sample_rate;
            if self.hold_counter < 0. {
                self.peak -= self.release / self.sample_rate;
            }
        }

        self.peak
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
}

struct PeakSmoother {
    buffer: Vec<f32>,
}

impl PeakSmoother {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.; size],
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.buffer.remove(0);
        self.buffer.push(input);
        self.buffer.iter().sum::<f32>() / self.buffer.len() as f32
    }
}
