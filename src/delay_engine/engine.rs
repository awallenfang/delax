/// The entry of the delay engine for Delax. It holds the buffers and handles the input and output of samples for specific parameters.
///
/// Usage:
/// ```rs
/// let mut engine = DelayEngine::new(44100);
/// engine.write_sample(0.5);
/// let out = engine.pop_sample();
/// assert_eq!(out, 0.5);
/// ```
pub struct DelayEngine {
    buffer: Vec<f32>,
    read_jumps: Vec<Jump>,
    write_jumps: Vec<Jump>,
    write_head: usize,
    read_head: usize,
}

impl DelayEngine {
    /// Initialize the engine using the size.
    /// The given size is the maximum size of the buffer and describes the maximum amount of data that can be held per bank.
    ///
    /// The buffer size can later be changed using [DelayEngine::set_buffer_size()].
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.; size],
            read_jumps: vec![Jump(size, 0)],
            write_jumps: vec![Jump(size, 0)],
            write_head: 0,
            read_head: 0,
        }
    }

    /// Outputs a sample and advances the read position in the internal banks.
    /// Usage:
    /// ```rs
    /// let mut engine = DelayEngine::new(44100);
    /// engine.write_sample(0.5);
    /// let out = engine.pop_sample();
    /// assert_eq!(out, 0.5);
    /// ```
    pub fn pop_sample(&mut self) -> f32 {
        let sample = self.buffer[self.read_head];
        self.read_head += 1;

        if let Some(jump) = self.check_jumps(self.read_head, &self.read_jumps) {
            self.read_head = jump.1;
        }

        sample
    }

    /// Writes a sample into the internal banks and advances the write position in the internal banks.
    /// Usage:
    /// ```rs
    /// let mut engine = DelayEngine::new(44100);
    /// engine.write_sample(0.5);
    /// let out = engine.pop_sample();
    /// assert_eq!(out, 0.5);
    /// ```
    pub fn write_sample(&mut self, sample: f32) {
        self.buffer[self.write_head] = sample;

        if let Some(jump) = self.check_jumps(self.write_head, &self.write_jumps) {
            self.write_head = jump.1;
        }

        self.write_head += 1;
    }

    /// Returns the state of the internal buffer banks as an immutable pointer.
    #[allow(dead_code)]
    pub fn get_buffer_ptr(&self) -> &[f32] {
        &self.buffer
    }

    /// Changes the delay duration in samples.
    ///
    /// For now this changes the position of the write head relative to the read head.
    ///
    /// Values larger than the bank size will simply result in a duration of `samples % bank_size``
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

    /// Check if there is a jump in the current index. If there is a jump, return it.
    fn check_jumps(&self, index: usize, jumps: &Vec<Jump>) -> Option<Jump> {
        for j in jumps {
            if index == j.0 {
                return Some(j.clone());
            }
        }
        None
    }

    /// Set the raw read jump vector. This assumes that the vector of jumps is valid and covers the whole buffer.
    #[allow(dead_code)]
    pub fn set_raw_read_jumps(&mut self, jumps: &Vec<Jump>) {
        self.read_jumps = jumps.clone();
    }
}

/// A jump inside of the banks. Currently this holds `Jump(from, to)`
#[derive(Clone)]
pub struct Jump(usize, usize);

#[cfg(test)]
mod tests {
    use super::{DelayEngine, Jump};

    #[test]
    fn init() {
        let mut engine = DelayEngine::new(44100);

        assert_eq!(engine.pop_sample(), 0.);
        assert_eq!(engine.get_buffer_ptr().len(), 44100);
    }

    #[test]
    fn check_sample_inout() {
        let mut engine = DelayEngine::new(5);

        engine.write_sample(1.);
        engine.write_sample(2.);
        engine.write_sample(3.);
        engine.write_sample(4.);
        engine.write_sample(5.);

        assert_eq!(engine.pop_sample(), 1.);
        assert_eq!(engine.pop_sample(), 2.);
        assert_eq!(engine.pop_sample(), 3.);
        assert_eq!(engine.pop_sample(), 4.);
        assert_eq!(engine.pop_sample(), 5.);
        assert_eq!(engine.pop_sample(), 1.);
    }

    #[test]
    fn internal_buffer() {
        let mut engine = DelayEngine::new(5);

        engine.write_sample(1.);
        engine.write_sample(2.);
        engine.write_sample(3.);
        engine.write_sample(4.);
        engine.write_sample(5.);

        let buffer = engine.get_buffer_ptr();

        assert_eq!(buffer, [1., 2., 3., 4., 5.])
    }

    #[test]
    fn buffer_size() {
        let mut engine = DelayEngine::new(5);

        let buffer = engine.get_buffer_ptr();

        assert_eq!(buffer.len(), 5);

        engine.set_buffer_size(10);

        let buffer = engine.get_buffer_ptr();

        assert_eq!(buffer.len(), 10);
    }

    #[test]
    fn read_jumps() {
        let mut engine = DelayEngine::new(10);
        engine.set_raw_read_jumps(&vec![Jump(10, 0), Jump(2, 5), Jump(8, 2), Jump(5, 8)]);

        engine.write_sample(1.);
        engine.write_sample(2.);
        engine.write_sample(3.);
        engine.write_sample(4.);
        engine.write_sample(5.);
        engine.write_sample(6.);
        engine.write_sample(7.);
        engine.write_sample(8.);
        engine.write_sample(9.);
        engine.write_sample(10.);

        assert_eq!(engine.pop_sample(), 1.);
        assert_eq!(engine.pop_sample(), 2.);
        assert_eq!(engine.pop_sample(), 6.);
        assert_eq!(engine.pop_sample(), 7.);
        assert_eq!(engine.pop_sample(), 8.);
        assert_eq!(engine.pop_sample(), 3.);
        assert_eq!(engine.pop_sample(), 4.);
        assert_eq!(engine.pop_sample(), 5.);
        assert_eq!(engine.pop_sample(), 9.);
        assert_eq!(engine.pop_sample(), 10.);
    }
}
