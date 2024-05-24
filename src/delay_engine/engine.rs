use nih_plug::nih_dbg;

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
    /// The internal mono buffer
    buffer: Vec<f32>,
    /// The sample rate to be used for internal conversions
    sample_rate: f32,
    /// The delay time in ms
    delay_time: f32,
    /// The positions at which the read head should jump
    read_jumps: Vec<Jump>,
    /// The positions at which the write head should jump
    write_jumps: Vec<Jump>,
    /// The current write head position
    write_head: usize,
    /// The current read head position
    read_head: usize,
}

impl DelayEngine {
    /// Initialize the engine using the size.
    /// The given size is the maximum size of the buffer and describes the maximum amount of data that can be held per bank.
    ///
    /// The buffer size can later be changed using [DelayEngine::set_buffer_size()].
    pub fn new(size: usize, sample_rate: f32) -> Self {
        Self {
            buffer: vec![0.; size],
            sample_rate,
            delay_time: 0.,
            read_jumps: vec![Jump(size - 1, 0)],
            write_jumps: vec![Jump(size - 1, 0)],
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
    #[allow(dead_code)]
    pub fn pop_sample(&mut self) -> f32 {
        let sample = self.buffer[self.read_head];
        if let Some(jump) = self.check_jumps(self.read_head, &self.read_jumps) {
            self.read_head = jump.1;
        } else {
            self.read_head += 1;
        }

        sample
    }

    /// Interpolate the buffer at the current delay time using the method specified as interpolation mode.
    pub fn interpolate_sample(&self, interpolation_mode: DelayInterpolationMode) -> f32 {
        match interpolation_mode {
            DelayInterpolationMode::Nearest => {
                let mut index = self.write_head as i32
                    - ms_to_samples(self.delay_time, self.sample_rate) as i32;
                index = index.rem_euclid(self.buffer.len() as i32);

                self.buffer[index as usize]
            }
            DelayInterpolationMode::Linear => {
                let upper_index =
                    ((self.write_head - ms_to_samples(self.delay_time, self.sample_rate)) as i32)
                        .rem_euclid(self.buffer.len() as i32) as i32;
                let lower_index = (upper_index - 1).rem_euclid(self.buffer.len() as i32) as i32;

                let lower_sample = self.buffer[lower_index as usize];
                let upper_sample = self.buffer[upper_index as usize];

                let interpolation_factor = (self.delay_time * self.sample_rate) % 1.;

                lower_sample * (1. - interpolation_factor) + upper_sample * interpolation_factor
            }
        }
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
    /// Input: Delay time in ms
    ///
    /// For now this changes the position of the write head relative to the read head.
    ///
    /// Values larger than the bank size will simply result in a duration of `samples % bank_size``
    pub fn set_delay_amount(&mut self, delay_time: f32) {
        let delay_samples = ms_to_samples(delay_time, self.sample_rate);
        self.read_head = (self.write_head + delay_samples) % self.buffer.len();
        self.delay_time = delay_time;
        nih_dbg!(delay_samples);
    }

    #[allow(dead_code)]
    /// Changes the buffer size.
    ///
    /// This resets the whole buffer to zero.
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer = vec![0.; size];
        self.write_head %= size;
        self.read_head %= size;
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
    pub fn set_raw_read_jumps(&mut self, jumps: &[Jump]) {
        self.read_jumps = jumps.to_owned();
    }

    /// Reset the internal buffers to zero.
    pub fn reset(&mut self) {
        self.buffer.iter_mut().for_each(|sample| *sample = 0.);
    }
}

/// A jump inside of the banks. Currently this holds `Jump(from, to)`.
/// Both are inclusive, so with `Jump(10,100)` the read order will be 8,9,10,100
#[derive(Clone)]
pub struct Jump(usize, usize);

#[allow(dead_code)]
pub enum DelayInterpolationMode {
    Nearest,
    Linear,
}

pub fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    ((ms / 1000.) * sample_rate).floor() as usize
}

#[cfg(test)]
mod tests {
    use super::{DelayEngine, Jump};

    #[test]
    fn init() {
        let mut engine = DelayEngine::new(44100, 44100.);

        assert_eq!(engine.pop_sample(), 0.);
        assert_eq!(engine.get_buffer_ptr().len(), 44100);
    }

    #[test]
    fn check_sample_inout() {
        let mut engine = DelayEngine::new(5, 44100.);

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
        let mut engine = DelayEngine::new(5, 44100.);

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
        let mut engine = DelayEngine::new(5, 44100.);

        let buffer = engine.get_buffer_ptr();

        assert_eq!(buffer.len(), 5);

        engine.set_buffer_size(10);

        let buffer = engine.get_buffer_ptr();

        assert_eq!(buffer.len(), 10);
    }

    #[test]
    fn read_jumps() {
        let mut engine = DelayEngine::new(10, 44100.);
        engine.set_raw_read_jumps(&vec![Jump(9, 0), Jump(2, 5), Jump(7, 3), Jump(4, 8)]);

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
        assert_eq!(engine.pop_sample(), 3.);
        assert_eq!(engine.pop_sample(), 6.);
        assert_eq!(engine.pop_sample(), 7.);
        assert_eq!(engine.pop_sample(), 8.);
        assert_eq!(engine.pop_sample(), 4.);
        assert_eq!(engine.pop_sample(), 5.);
        assert_eq!(engine.pop_sample(), 9.);
        assert_eq!(engine.pop_sample(), 10.);

        assert_eq!(engine.pop_sample(), 1.);
        assert_eq!(engine.pop_sample(), 2.);
        assert_eq!(engine.pop_sample(), 3.);
        assert_eq!(engine.pop_sample(), 6.);
        assert_eq!(engine.pop_sample(), 7.);
        assert_eq!(engine.pop_sample(), 8.);
        assert_eq!(engine.pop_sample(), 4.);
        assert_eq!(engine.pop_sample(), 5.);
        assert_eq!(engine.pop_sample(), 9.);
        assert_eq!(engine.pop_sample(), 10.);
    }
}
