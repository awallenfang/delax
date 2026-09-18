use crate::delay_engine::jump_builder::Jump;

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
            read_jumps: vec![Jump(size - 1, 0, 0)],
            write_jumps: vec![Jump(size - 1, 0, 0)],
            write_head: 0,
            read_head: 0,
        }
    }

    pub fn pop_sample(&mut self) -> f32 {
        let sample = self.buffer[self.read_head];
        self.step_read_head();
        sample
    }

    pub fn interpolate_sample(&mut self, interpolation_mode: DelayInterpolationMode) -> f32 {
        let sample = match interpolation_mode {
            DelayInterpolationMode::Nearest => self.buffer[self.read_head],
            DelayInterpolationMode::Linear => {
                let upper_sample = self.buffer[self.read_head];
                let lower_sample = self.buffer[self.prev_in_cycle()];
                let interpolation_factor = ((self.delay_time / 1000.) * self.sample_rate).fract();
                upper_sample * (1. - interpolation_factor) + lower_sample * interpolation_factor
            }
        };
        self.step_read_head();
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
        } else {
            self.write_head += 1;
        }
    }

    /// Returns the state of the internal buffer banks as an immutable pointer.
    #[allow(dead_code)]
    pub fn get_buffer_ptr(&self) -> &[f32] {
        &self.buffer
    }

    pub fn set_delay_amount(&mut self, delay_time: f32) {
        if delay_time == self.delay_time {
            return;
        }
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
        self.read_jumps = vec![Jump(size - 1, 0, 0)];
        self.write_jumps = vec![Jump(size - 1, 0, 0)];
        self.write_head = 0;
        self.read_head = 0;
    }

    fn check_jumps(&self, index: usize, jumps: &Vec<Jump>) -> Option<Jump> {
        for j in jumps {
            if index == j.0 {
                return Some(j.clone());
            }
        }
        None
    }

    fn step_read_head(&mut self) {
        if let Some(jump) = self.check_jumps(self.read_head, &self.read_jumps) {
            self.read_head = jump.1;
        } else {
            self.read_head += 1;
        }
    }

    fn prev_in_cycle(&self) -> usize {
        for j in &self.read_jumps {
            if j.1 == self.read_head {
                return j.0;
            }
        }
        (self.read_head + self.active_len - 1) % self.active_len
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
        self.read_jumps = vec![Jump(clamp_len - 1, 0, 0)];
        self.write_jumps = vec![Jump(clamp_len - 1, 0, 0)];
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
        let mut a = make_ramp_engine(10, 1000.);
        let mut b = make_ramp_engine(10, 1000.);
        a.set_delay_amount(3.);
        b.set_delay_amount(3.);
        assert_eq!(
            a.interpolate_sample(DelayInterpolationMode::Nearest),
            b.interpolate_sample(DelayInterpolationMode::Linear)
        );
        let mut e2 = DelayEngine::new(10, 1000.);
        let mut e3 = DelayEngine::new(10, 1000.);
        for i in 0..5 {
            e2.write_sample(i as f32 * 10.);
            e3.write_sample(i as f32 * 10.);
        }
        e2.set_delay_amount(2.);
        e3.set_delay_amount(2.);
        assert_eq!(e2.interpolate_sample(DelayInterpolationMode::Nearest), 30.);
        assert_eq!(e3.interpolate_sample(DelayInterpolationMode::Linear), 30.);
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
    use super::{DelayEngine, DelayInterpolationMode, Jump};
    use crate::delay_engine::jump_builder::JumpBuilder;

    #[test]
    fn prev_in_cycle_follows_jumps() {
        let mut engine = DelayEngine::new(10, 1000.);
        engine.set_raw_read_jumps(&[Jump(9, 0, 0), Jump(2, 5, 1)]);
        engine.set_delay_amount(0.);
        assert_eq!(engine.prev_in_cycle(), 9);
        for _ in 0..4 {
            engine.pop_sample();
        }
        assert_eq!(engine.read_head(), 6);
        assert_eq!(engine.prev_in_cycle(), 5);
    }

    #[test]
    fn interpolate_nearest_matches_pop_over_shuffled_loop() {
        let jumps = [Jump(9, 0, 0), Jump(2, 5, 1), Jump(7, 3, 2), Jump(4, 8, 4)];
        let mut a = DelayEngine::new(10, 44100.);
        let mut b = DelayEngine::new(10, 44100.);
        for i in 0..10 {
            a.write_sample(i as f32);
            b.write_sample(i as f32);
        }
        a.set_raw_read_jumps(&jumps);
        b.set_raw_read_jumps(&jumps);
        for _ in 0..20 {
            assert_eq!(
                a.interpolate_sample(DelayInterpolationMode::Nearest),
                b.pop_sample()
            );
        }
    }

    #[test]
    fn interpolate_linear_matches_nearest_at_integer_delay_with_jumps() {
        let mut a = DelayEngine::new(10, 1000.);
        let mut b = DelayEngine::new(10, 1000.);
        for i in 0..10 {
            a.write_sample(i as f32);
            b.write_sample(i as f32);
        }
        a.set_raw_read_jumps(&[Jump(9, 0, 0), Jump(4, 5, 1)]);
        b.set_raw_read_jumps(&[Jump(9, 0, 0), Jump(4, 5, 1)]);
        a.set_delay_amount(2.);
        b.set_delay_amount(2.);
        assert_eq!(
            a.interpolate_sample(DelayInterpolationMode::Nearest),
            b.interpolate_sample(DelayInterpolationMode::Linear)
        );
    }

    #[test]
    fn interpolate_linear_blend_across_jump() {
        let mut engine = DelayEngine::new(10, 1000.);
        for i in 0..10 {
            engine.write_sample(i as f32);
        }
        engine.set_raw_read_jumps(&[Jump(9, 0, 0), Jump(2, 5, 1), Jump(7, 3, 2), Jump(4, 8, 3)]);
        engine.set_delay_amount(2.5);
        let s = engine.interpolate_sample(DelayInterpolationMode::Linear);
        assert!((s - 6.).abs() < 1e-5);
    }

    #[test]
    fn set_delay_amount_unchanged_keeps_stepping_head() {
        let mut engine = DelayEngine::new(12, 1000.);
        for i in 0..12 {
            engine.write_sample(i as f32);
        }
        engine.set_raw_read_jumps(&[Jump(11, 0, 0), Jump(2, 6, 1), Jump(8, 3, 2), Jump(5, 9,3)]);
        engine.set_delay_amount(0.);

        let mut got = Vec::with_capacity(12);
        for _ in 0..12 {
            engine.set_delay_amount(0.);
            got.push(engine.pop_sample() as usize);
        }
        assert_eq!(got, vec![0, 1, 2, 6, 7, 8, 3, 4, 5, 9, 10, 11]);
    }

    #[test]
    fn set_delay_amount_changed_reanchors_head() {
        let mut engine = DelayEngine::new(12, 1000.);
        for i in 0..12 {
            engine.write_sample(i as f32);
        }
        engine.set_raw_read_jumps(&[Jump(11, 0, 0), Jump(2, 6, 1), Jump(8, 3,2), Jump(5, 9,3)]);
        engine.set_delay_amount(0.);
        for _ in 0..12 {
            engine.set_delay_amount(0.);
            engine.pop_sample();
        }
        engine.set_delay_amount(3.);
        assert_eq!(engine.pop_sample() as usize, 9);
    }

    #[test]
    fn reinstalled_jumps_after_length_change_cover_new_length() {
        let mut engine = DelayEngine::new(12, 1000.);
        for i in 0..12 {
            engine.write_sample(i as f32);
        }
        engine.set_active_len(6);
        let jumps = JumpBuilder::split_evenly(engine.active_len(), 2)
            .shuffle_seeded(7)
            .build();
        engine.set_raw_read_jumps(&jumps);

        let first = engine.pop_sample() as usize;
        let mut got = vec![first];
        for _ in 1..6 {
            got.push(engine.pop_sample() as usize);
        }
        got.sort_unstable();
        assert_eq!(got, (0..6).collect::<Vec<_>>());
        assert_eq!(engine.pop_sample() as usize, first);
    }

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
        engine.set_raw_read_jumps(&vec![Jump(9, 0,0), Jump(2, 5,1), Jump(7, 3,2), Jump(4, 8,3)]);

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
