pub struct DelayEngine {
    buffer: Vec<f32>,
    write_head: usize,
    read_head: usize
}

impl DelayEngine {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.; size],
            write_head: 0,
            read_head: 0
        }
    }

    pub fn pop_sample(&mut self) -> f32 {
        let sample = self.buffer[self.read_head];

        self.read_head += 1;
        if self.read_head >= self.buffer.len() {
            self.read_head = 0;
        }

        sample
    }

    pub fn write_sample_unchecked(&mut self, sample: f32) {
        self.buffer[self.write_head] = sample;
        self.write_head += 1;

        if self.write_head >= self.buffer.len() {
            self.write_head = 0;
        }
    }
    
    #[allow(dead_code)]
    pub fn get_buffer_ptr(&self) -> &[f32] {
        &self.buffer
    }

    pub fn set_delay_amount(&mut self, delay_samples: usize) {
        self.read_head = (self.write_head + delay_samples) % self.buffer.len();
    }
}

#[cfg(test)]
mod tests {
    use super::DelayEngine;

    #[test]
    fn init() {
        let mut engine = DelayEngine::new(44100);

        assert_eq!(engine.pop_sample(), 0.);
        assert_eq!(engine.get_buffer_ptr().len(), 44100);
    }

    #[test]
    fn check_sample_inout() {
        let mut engine = DelayEngine::new(5);

        engine.write_sample_unchecked(1.);
        engine.write_sample_unchecked(2.);
        engine.write_sample_unchecked(3.);
        engine.write_sample_unchecked(4.);
        engine.write_sample_unchecked(5.);

        assert_eq!(engine.pop_sample(), 1.);
        assert_eq!(engine.pop_sample(), 2.);
        assert_eq!(engine.pop_sample(), 3.);
        assert_eq!(engine.pop_sample(), 4.);
        assert_eq!(engine.pop_sample(), 5.);
        assert_eq!(engine.pop_sample(), 1.);

    }
}