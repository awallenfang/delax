/// The entry of the delay engine for Delax. It holds the buffers and handles the input and output of samples for specific parameters.
///
/// Usage:
/// ```rs
/// let mut engine = DelayEngine::new(44100);
/// engine.write_sample(0.5);
/// let out = engine.pop_sample();
/// assert_eq!(out, 0.5);
/// ```
pub const MAX_DELAY_SECS: f32 = 32.0;

const MIN_ACTIVE_LEN: usize = 1;

pub struct DelayEngine {
    /// The internal mono buffer
    buffer: Vec<f32>,
    /// The length of the actively used buffer part. Changes using the write/read jumps
    active_len: usize,
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
        let size = size.max(MIN_ACTIVE_LEN);
        Self {
            buffer: vec![0.; size],
            active_len: size,
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
                index = index.rem_euclid(self.active_len as i32);

                self.buffer[index as usize]
            }
            DelayInterpolationMode::Linear => {
                let upper_index = (self.write_head as i32
                    - ms_to_samples(self.delay_time, self.sample_rate) as i32)
                    .rem_euclid(self.active_len as i32);
                let lower_index = (upper_index - 1).rem_euclid(self.active_len as i32);

                let lower_sample = self.buffer[lower_index as usize];
                let upper_sample = self.buffer[upper_index as usize];

                let interpolation_factor = ((self.delay_time / 1000.) * self.sample_rate).fract();

                // upper is the sample closest to write_head (least delayed),
                // lower is one sample older. For integer delay fract==0 we must
                // return upper to match Nearest.
                upper_sample * (1. - interpolation_factor) + lower_sample * interpolation_factor
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
        } else {
            self.write_head += 1;
        }
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
    /// The delay is clamped to `active_len - 1` so it never reads into the
    /// inactive tail. Callers needing UI feedback should compare against
    /// [DelayEngine::max_delay_ms()] to detect clamping.
    pub fn set_delay_amount(&mut self, delay_time: f32) {
        let delay_samples =
            ms_to_samples(delay_time, self.sample_rate).clamp(0, self.active_len - 1);
        self.read_head = ((self.write_head as i32 - delay_samples as i32)
            .rem_euclid(self.active_len as i32)) as usize;
        self.delay_time = delay_time;
    }

    /// Maximum delay in ms that fits into the current active length.
    pub fn max_delay_ms(&self) -> f32 {
        ((self.active_len.saturating_sub(1)) as f32 / self.sample_rate) * 1000.
    }

    #[allow(dead_code)]
    /// Changes the buffer size.
    ///
    /// This allocates and must only be called from `activate()`/init, never
    /// on the audio thread. Resets the active length to the full capacity.
    pub fn set_buffer_size(&mut self, size: usize) {
        let size = size.max(MIN_ACTIVE_LEN);
        self.buffer = vec![0.; size];
        self.active_len = size;
        self.read_jumps = vec![Jump(size - 1, 0)];
        self.write_jumps = vec![Jump(size - 1, 0)];
        self.write_head = 0;
        self.read_head = 0;
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

    /// Set the raw write jump vector. Symmetric to [DelayEngine::set_raw_read_jumps].
    #[allow(dead_code)]
    pub fn set_raw_write_jumps(&mut self, jumps: &[Jump]) {
        self.write_jumps = jumps.to_owned();
    }

    /// Reset the internal buffers to zero.
    ///
    /// Only clears the active region so large capacities stay cheap on the
    /// audio thread. Full clears happen in `activate()` via fresh allocation.
    pub fn reset(&mut self) {
        self.buffer[..self.active_len]
            .iter_mut()
            .for_each(|sample| *sample = 0.);
    }

    /// Sets the effective length without reallocating.
    ///
    /// Installs a single wrap jump `Jump(len-1, 0)` for both heads so the
    /// engine only cycles `0..len`. Heads are wrapped into range; call
    /// [DelayEngine::set_delay_amount()] afterwards to rebase `read_head`
    /// for the current delay (done by the caller in `update_params`).
    /// Allocation-free and safe on the audio thread.
    pub fn set_active_len(&mut self, length: usize) {
        let clamp_len = length.clamp(MIN_ACTIVE_LEN, self.buffer.len());
        if clamp_len == self.active_len {
            return;
        }
        self.active_len = clamp_len;
        self.read_jumps = vec![Jump(clamp_len - 1, 0)];
        self.write_jumps = vec![Jump(clamp_len - 1, 0)];
        self.write_head %= clamp_len;
        self.read_head %= clamp_len;
    }

    pub fn write_head(&self) -> usize {
        return self.write_head
    }

    pub fn read_head(&self) -> usize {
        self.read_head
    }

    pub fn active_len(&self) -> usize {
        self.active_len
    }

    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    pub fn read_jumps(&self) -> Vec<Jump> {
        self.read_jumps.clone()
    }

    pub fn write_jumps(&self) -> Vec<Jump> {
        self.write_jumps.clone()
    }

    pub fn get_active_ptr(&self) -> &[f32] {
        &self.buffer[..self.active_len]
    }
}

/// A jump inside of the banks. Currently this holds `Jump(from, to)`.
/// Both are inclusive, so with `Jump(10,100)` the read order will be 8,9,10,100
#[derive(Clone, Copy)]
pub struct Jump(pub usize, pub usize);

#[allow(dead_code)]
pub enum DelayInterpolationMode {
    Nearest,
    Linear,
}

pub fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    ((ms / 1000.) * sample_rate).floor() as usize
}

#[cfg(test)]
mod interpolation_tests {
    use super::{DelayEngine, DelayInterpolationMode, ms_to_samples};

    fn make_ramp_engine(size: usize, sample_rate: f32) -> DelayEngine {
        let mut e = DelayEngine::new(size, sample_rate);
        for i in 0..size {
            e.write_sample(i as f32);
        }
        e
    }

    #[test]
    fn nearest_integer_delay_no_wrap() {
        let mut e = make_ramp_engine(10, 1000.);
        e.set_delay_amount(2.);
        let s = e.interpolate_sample(DelayInterpolationMode::Nearest);
        assert_eq!(s, 8.);
    }

    #[test]
    fn nearest_delay_wraps_at_zero() {
        let mut e = DelayEngine::new(10, 1000.);
        for i in 0..1 {
            e.write_sample(i as f32);
        }
        e.set_delay_amount(5.);
        let s = e.interpolate_sample(DelayInterpolationMode::Nearest);
        assert_eq!(s, 0.);

        for i in 1..10 {
            e.write_sample(i as f32);
        }
        e.set_delay_amount(3.);
        assert_eq!(e.interpolate_sample(DelayInterpolationMode::Nearest), 7.);
    }

    #[test]
    fn linear_no_panic_when_write_head_less_than_delay() {
        let mut e = DelayEngine::new(10, 1000.);
        e.write_sample(1.);
        e.set_delay_amount(5.);
        let s = e.interpolate_sample(DelayInterpolationMode::Linear);
        assert!(s.is_finite());
    }

    #[test]
    fn linear_integer_delay_matches_nearest() {
        let mut e = make_ramp_engine(10, 1000.);
        e.set_delay_amount(3.);
        let nearest = e.interpolate_sample(DelayInterpolationMode::Nearest);
        let linear = e.interpolate_sample(DelayInterpolationMode::Linear);
        assert_eq!(linear, nearest);
        let mut e2 = DelayEngine::new(10, 1000.);
        for i in 0..5 {
            e2.write_sample(i as f32 * 10.);
        }
        e2.set_delay_amount(2.);
        assert_eq!(e2.interpolate_sample(DelayInterpolationMode::Nearest), 30.);
        assert_eq!(e2.interpolate_sample(DelayInterpolationMode::Linear), 30.);
    }

    #[test]
    fn linear_fractional_interpolation() {
        let mut e = make_ramp_engine(10, 1000.);
        e.set_delay_amount(1.5);
        let s = e.interpolate_sample(DelayInterpolationMode::Linear);
        assert!((s - 8.5).abs() < 1e-5, "got {s}");

        let mut e2 = make_ramp_engine(10, 10000.);
        e2.set_delay_amount(0.15); // 1.5 samples
        assert!((e2.interpolate_sample(DelayInterpolationMode::Linear) - 8.5).abs() < 1e-5);
    }

    #[test]
    fn linear_wrap_across_boundary_interpolates_correctly() {
        let mut e = make_ramp_engine(10, 1000.);
        e.set_delay_amount(0.5);
        let s = e.interpolate_sample(DelayInterpolationMode::Linear);
        assert!((s - 4.5).abs() < 1e-5, "wrap interpolation got {s}");
    }

    #[test]
    fn clamping_prevents_zero_delay_garbage_at_max_delay() {
        let mut e = DelayEngine::new(44100, 44100.);
        for i in 0..100 {
            e.write_sample(i as f32);
        }
        e.set_delay_amount(1000.);
        let s = e.interpolate_sample(DelayInterpolationMode::Nearest);
        assert!(s.is_finite());
        e.set_delay_amount(5000.);
        let s2 = e.interpolate_sample(DelayInterpolationMode::Nearest);
        assert!(s2.is_finite());
    }

    #[test]
    fn ms_to_samples_floor() {
        assert_eq!(ms_to_samples(1., 1000.), 1);
        assert_eq!(ms_to_samples(1.5, 1000.), 1);
        assert_eq!(ms_to_samples(1., 44100.), 44);
        assert_eq!(ms_to_samples(1000., 44100.), 44100);
    }

    #[test]
    fn sample_rate_scaling() {
        let e1 = DelayEngine::new(48000, 48000.);
        let e2 = DelayEngine::new(96000, 96000.);
        assert_eq!(ms_to_samples(500., 48000.), 24000);
        assert_eq!(ms_to_samples(500., 96000.), 48000);

        let mut eng = DelayEngine::new(96000, 96000.);
        for i in 0..100 {
            eng.write_sample(i as f32);
        }
        eng.set_delay_amount(500.);
        assert!(
            eng.interpolate_sample(DelayInterpolationMode::Nearest)
                .is_finite()
        );
        let _ = e1;
        let _ = e2;
    }
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
