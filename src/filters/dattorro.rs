use super::StereoFilter;

impl StereoFilter for DattorroReverb {
    fn process_stereo(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.process_stereo(input_l, input_r)
    }
}

/// A reverb network implemented from the Dattorro Reverb design paper:
/// https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf
#[derive( Clone)]
pub struct DattorroReverb {
    pre_delay: DelayLine,
    bandwith_damper: Damper,
    input_diffusor_1: InputDiffusor,
    input_diffusor_2: InputDiffusor,
    input_diffusor_3: InputDiffusor,
    input_diffusor_4: InputDiffusor,
    decay_diffusor_l: DecayDiffusor,
    decay_diffusor_r: DecayDiffusor,
    input_diffusor_l: InputDiffusor,
    input_diffusor_r: InputDiffusor,
    damper_l: Damper,
    damper_r: Damper,
    delay_line_1_l: DelayLine,
    delay_line_2_l: DelayLine,
    delay_line_1_r: DelayLine,
    delay_line_2_r: DelayLine,
    recursive_l: f32,
    recursive_r: f32,
    decay: f32,
    tap_l_1: DelayLine,
    tap_l_2: DelayLine,
    tap_l_3: DelayLine,
    tap_r_1: DelayLine,
    tap_r_2: DelayLine,
    tap_r_3: DelayLine,
    gain: f32,
}

impl DattorroReverb {
    pub fn new(sample_rate: f32, decay: f32) -> Self {
        let mut pre_delay = DelayLine::new(sample_rate as usize);
        pre_delay.set_delay(0);
        Self {
            pre_delay,
            bandwith_damper: Damper::new(0.9995),
            input_diffusor_1: InputDiffusor::new(142, 0.75),
            input_diffusor_2: InputDiffusor::new(107, 0.75),
            input_diffusor_3: InputDiffusor::new(379, 0.625),
            input_diffusor_4: InputDiffusor::new(277, 0.625),
            decay_diffusor_l: DecayDiffusor::new(sample_rate, 672, 0.75),
            decay_diffusor_r: DecayDiffusor::new(sample_rate, 908, 0.75),
            input_diffusor_l: InputDiffusor::new(1800, 0.625),
            input_diffusor_r: InputDiffusor::new(2656, 0.625),
            damper_l: Damper::new(0.0005),
            damper_r: Damper::new(0.0005),
            delay_line_1_l: DelayLine::new(4453),
            delay_line_2_l: DelayLine::new(3720),
            delay_line_1_r: DelayLine::new(4217),
            delay_line_2_r: DelayLine::new(3163),
            recursive_l: 0.,
            recursive_r: 0.,
            decay,
            tap_l_1: DelayLine::new(sample_rate as usize / 4),
            tap_l_2: DelayLine::new(sample_rate as usize / 4),
            tap_l_3: DelayLine::new(sample_rate as usize / 4),
            tap_r_1: DelayLine::new(sample_rate as usize / 4),
            tap_r_2: DelayLine::new(sample_rate as usize / 4),
            tap_r_3: DelayLine::new(sample_rate as usize / 4),
            gain: 1.
        }
    }

    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let input = (l + r) / 2.;
        let pre_delayed = self.pre_delay.process(input);
        let bandwith_damped = self.bandwith_damper.process(pre_delayed);

        let mut signal = bandwith_damped;
        signal = self.input_diffusor_1.process(signal);
        signal = self.input_diffusor_2.process(signal);
        signal = self.input_diffusor_3.process(signal);
        signal = self.input_diffusor_4.process(signal);

        self.recursive_l += signal + self.recursive_r * self.decay;
        self.recursive_r += signal + self.recursive_l * self.decay;

        self.recursive_l = self.decay_diffusor_l.process(self.recursive_l);
        self.recursive_r = self.decay_diffusor_r.process(self.recursive_r);

        let left_init_tap: f32 = self.recursive_l;
        let right_init_tap: f32 = self.recursive_r;
        
        self.recursive_l = self.delay_line_1_l.process(self.recursive_l);
        self.recursive_r = self.delay_line_1_r.process(self.recursive_r);
        
        self.tap_l_1.insert(self.recursive_l);
        self.tap_r_1.insert(self.recursive_r);
        
        self.recursive_l = self.damper_l.process(self.recursive_l) * self.decay;
        self.recursive_r = self.damper_r.process(self.recursive_r) * self.decay;

        self.recursive_l = self.input_diffusor_l.process(self.recursive_l);
        self.recursive_r = self.input_diffusor_r.process(self.recursive_r);

        self.tap_l_2.insert(self.input_diffusor_l.tap());
        self.tap_r_2.insert(self.input_diffusor_r.tap());

        self.tap_l_3.insert(self.recursive_l);
        self.tap_r_3.insert(self.recursive_r);

        self.recursive_l = self.delay_line_2_l.process(self.recursive_l);
        self.recursive_r = self.delay_line_2_r.process(self.recursive_r);

        self.output(left_init_tap, right_init_tap)
    }

    pub fn output(&self, left_init: f32, right_init: f32) -> (f32, f32) {
        let mut y_l = left_init + self.tap_r_1.get_with_delay(266) + self.tap_r_1.get_with_delay(2974)
            - self.tap_r_2.get_with_delay(1913)
            + self.tap_r_3.get_with_delay(1996)
            - self.tap_l_1.get_with_delay(1990)
            - self.tap_l_2.get_with_delay(187)
            - self.tap_l_3.get_with_delay(1066);
        let mut y_r = right_init + self.tap_l_1.get_with_delay(353) + self.tap_l_1.get_with_delay(3627)
            - self.tap_l_2.get_with_delay(1228)
            + self.tap_l_3.get_with_delay(2673)
            - self.tap_r_1.get_with_delay(2111)
            - self.tap_r_2.get_with_delay(335)
            - self.tap_r_3.get_with_delay(121);
        y_l *= self.gain * 2.;
        y_r *= self.gain * 2.;
        (y_l, y_r)
    }

    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay;
    }

    /// Update the sample rate of everything.
    /// Important: This will reset the delay lines, since their maximum size is based on the sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.pre_delay = DelayLine::new(sample_rate as usize);
        self.decay_diffusor_l.set_sample_rate(sample_rate);
        self.decay_diffusor_r.set_sample_rate(sample_rate);
        self.tap_l_1 = DelayLine::new(sample_rate as usize / 4);
        self.tap_l_2 = DelayLine::new(sample_rate as usize / 4);
        self.tap_l_3 = DelayLine::new(sample_rate as usize / 4);
        self.tap_r_1 = DelayLine::new(sample_rate as usize / 4);
        self.tap_r_2 = DelayLine::new(sample_rate as usize / 4);
        self.tap_r_3 = DelayLine::new(sample_rate as usize / 4);
    }
}

#[derive(Debug, Clone)]
struct DelayLine {
    buffer: Vec<f32>,
    delay: usize,
    write_index: usize,
}

impl DelayLine {
    fn new(max_delay: usize) -> Self {
        Self {
            buffer: vec![0.0; (max_delay) as usize],
            delay: max_delay,
            write_index: 0,
        }
    }

    fn set_delay(&mut self, delay: usize) {
        self.delay = delay;
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed_index = (self.write_index as i32 - self.delay as i32)
            .rem_euclid(self.buffer.len() as i32) as usize;
        let delayed = self.buffer[delayed_index];

        self.buffer[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.buffer.len();

        delayed
    }

    fn get(&self) -> f32 {
        let delayed_index = (self.write_index as i32 - self.delay as i32)
            .rem_euclid(self.buffer.len() as i32) as usize;
        self.buffer[delayed_index]
    }

    fn get_with_delay(&self, delay: usize) -> f32 {
        let delayed_index =
            (self.write_index as i32 - delay as i32).rem_euclid(self.buffer.len() as i32) as usize;
        self.buffer[delayed_index]
    }

    fn insert(&mut self, input: f32) {
        self.buffer[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.buffer.len();
    }
}

#[derive(Clone)]

struct InputDiffusor {
    delay_line: DelayLine,
    gain: f32,
}

impl InputDiffusor {
    fn new(delay: usize, gain: f32) -> Self {
        Self {
            delay_line: DelayLine::new(delay),
            gain: gain,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay_line.get();
        let in_changed = input + delayed * self.gain * -1.;

        self.delay_line.insert(in_changed);

        delayed + in_changed * self.gain
    }

    fn tap(&self) -> f32 {
        self.delay_line.get_with_delay(0)
    }
}

#[derive(Clone)]

struct DecayDiffusor {
    delay_line: DelayLine,
    delay: usize,
    gain: f32,
    sample_rate: f32,
    excursion: f32,
    excursion_tick: f32,
    excursion_rate: f32,
    excursion_depth: f32,
}

impl DecayDiffusor {
    fn new(sample_rate: f32, delay: usize, gain: f32) -> Self {
        Self {
            delay_line: DelayLine::new(delay + 16),
            delay: delay,
            gain: gain,
            excursion: 0.,
            excursion_tick: 0.,
            excursion_rate: 1.,
            excursion_depth: 8.,
            sample_rate,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        // Update excursion and delay length
        self.modulate_excursion();

        let delayed = self
            .delay_line
            .get_with_delay(self.delay + self.excursion.floor() as usize);
        let in_changed = input + delayed * self.gain;

        self.delay_line.insert(in_changed);

        delayed + in_changed * self.gain * -1.
    }

    /// Modulates the excursion for each sample at a specific rate
    fn modulate_excursion(&mut self) {
        self.excursion = (self.excursion_tick * self.excursion_rate).sin() * self.excursion_depth;
        self.excursion_tick += 1. / self.sample_rate;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
}

#[derive(Clone)]
struct Damper {
    last_sample: f32,
    damping: f32,
}

impl Damper {
    fn new(damping: f32) -> Self {
        Self {
            last_sample: 0.,
            damping,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = input * (1. - self.damping) + self.last_sample * self.damping;
        self.last_sample = out;
        out
    }
}

#[cfg(test)]
mod DattorroTests {
    use super::*;
    #[test]
    fn delay_line() {
        let mut delay_line = DelayLine::new(4);
        delay_line.set_delay(2);
        assert_eq!(delay_line.process(1.), 0.);
        assert_eq!(delay_line.process(2.), 0.);
        assert_eq!(delay_line.process(3.), 1.);
        assert_eq!(delay_line.process(4.), 2.);
        assert_eq!(delay_line.process(5.), 3.);
        assert_eq!(delay_line.process(6.), 4.);
        assert_eq!(delay_line.process(7.), 5.);
        assert_eq!(delay_line.process(8.), 6.);
    }

    #[test]
    fn input_diffusor() {
        let mut input_diffusor = InputDiffusor::new(2, 0.5);

        // Values are calculated by hand based on the paper structure
        assert_eq!(input_diffusor.process(1.), 0.5);
        assert_eq!(input_diffusor.process(2.), 1.);
        assert_eq!(input_diffusor.process(3.), 2.25);
    }
}