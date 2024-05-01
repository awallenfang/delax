
pub struct DelayEngine {
    buffer: Vec<f32>,
    read_jumps: Vec<Jump>,
    write_jumps: Vec<Jump>,
    write_head: usize,
    read_head: usize,
}

impl DelayEngine {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.; size],
            read_jumps: vec![Jump(size, 0)],
            write_jumps: vec![Jump(size, 0)],
            write_head: 0,
            read_head: 0,
        }
    }

    pub fn pop_sample(&mut self) -> f32 {
        let sample = self.buffer[self.read_head];

        self.read_head += 1;
        if let Some(jump) = self.check_jumps(self.read_head, &self.read_jumps) {
            self.read_head = jump.1;
        }


        sample
    }

    pub fn write_sample_unchecked(&mut self, sample: f32) {
        self.buffer[self.write_head] = sample;
        if let Some(jump) = self.check_jumps(self.write_head, &self.write_jumps) {
            self.write_head = jump.1;
        }

        self.write_head += 1;
    }

    #[allow(dead_code)]
    pub fn get_buffer_ptr(&self) -> &[f32] {
        &self.buffer
    }

    pub fn set_delay_amount(&mut self, delay_samples: usize) {
        self.write_head = (self.read_head + delay_samples) % self.buffer.len();
    }

    #[allow(dead_code)]
    /// Changes the buffer size.
    ///
    /// This resets the whole buffer to zero.
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer = vec![0.; size];
        self.write_head = self.write_head % size;
        self.read_head = self.read_head % size;
    }

    fn check_jumps(&self, index: usize, jumps: &Vec<Jump>) -> Option<Jump> {
        for j in jumps {
            if index == j.0 {
                return Some(j.clone());
            }
        }
        None
    }
}

#[derive(Clone)]
struct Jump(usize, usize);

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
