use super::StereoFilter;
use nice_plug::prelude::*;

const INPUT_DIFFUSOR_SIZE_1: usize = 142;
const INPUT_DIFFUSOR_SIZE_2: usize = 107;
const INPUT_DIFFUSOR_SIZE_3: usize = 379;
const INPUT_DIFFUSOR_SIZE_4: usize = 277;
const INPUT_DIFFUSOR_SIZE_L: usize = 1800;
const INPUT_DIFFUSOR_SIZE_R: usize = 2656;
const DECAY_DIFFUSOR_SIZE_L: usize = 672;
const DECAY_DIFFUSOR_SIZE_R: usize = 908;
const DELAY_LINE_SIZE_L_1: usize = 4453;
const DELAY_LINE_SIZE_L_2: usize = 3720;
const DELAY_LINE_SIZE_R_1: usize = 4217;
const DELAY_LINE_SIZE_R_2: usize = 3163;

const TAP_LEFT_1: usize = 266;
const TAP_LEFT_2: usize = 2974;
const TAP_LEFT_3: usize = 1913;
const TAP_LEFT_4: usize = 1996;
const TAP_LEFT_5: usize = 1990;
const TAP_LEFT_6: usize = 187;
const TAP_LEFT_7: usize = 1066;

const TAP_RIGHT_1: usize = 353;
const TAP_RIGHT_2: usize = 3627;
const TAP_RIGHT_3: usize = 1228;
const TAP_RIGHT_4: usize = 2673;
const TAP_RIGHT_5: usize = 2111;
const TAP_RIGHT_6: usize = 335;
const TAP_RIGHT_7: usize = 121;

const MAX_SIZE: f32 = 2.;
#[derive(Params)]
pub struct DattorroParams {
    #[id = "dattorro_size"]
    pub size: FloatParam,
    #[id = "dattorro_decay"]
    pub decay: FloatParam,
    #[id = "dattorro_pre_delay"]
    pub pre_delay: FloatParam,
    #[id = "dattorro_damping"]
    pub damping: FloatParam,
    #[id = "dattorro_brightness"]
    pub brightness: FloatParam,
    #[id = "dattorro_lushness"]
    pub lushness: FloatParam,
    #[id = "dattorro_input_smear"]
    pub input_smear: FloatParam,
    #[id = "dattorro_tank_smear"]
    pub tank_smear: FloatParam,
    #[id = "dattorro_mix"]
    pub mix: FloatParam,
}

impl Default for DattorroParams {
    fn default() -> Self {
        Self {
            size: FloatParam::new(
                "Size",
                1.,
                FloatRange::Linear {
                    min: 0.1,
                    max: MAX_SIZE,
                },
            )
            .with_smoother(SmoothingStyle::Linear(0.2)),
            decay: FloatParam::new("Decay", 0.5, FloatRange::Linear { min: 0.01, max: 1. })
                .with_smoother(SmoothingStyle::Linear(0.2)),
            pre_delay: FloatParam::new(
                "Pre Delay",
                0.5,
                FloatRange::Skewed {
                    min: 0.01,
                    max: 1.,
                    factor: 2.,
                },
            )
            .with_smoother(SmoothingStyle::Exponential(2.)),
            damping: FloatParam::new("Damping", 0.7, FloatRange::Linear { min: 0.0, max: 1. })
                .with_smoother(SmoothingStyle::Linear(0.2)),
            brightness: FloatParam::new(
                "Brightness",
                0.8,
                FloatRange::Linear {
                    min: 0.5,
                    max: 0.95,
                },
            )
            .with_smoother(SmoothingStyle::Linear(0.2)),
            lushness: FloatParam::new("Lushness", 8., FloatRange::Linear { min: 4., max: 16. })
                .with_smoother(SmoothingStyle::Linear(1.)),
            input_smear: FloatParam::new(
                "Input Smear",
                0.65,
                FloatRange::Linear {
                    min: 0.5,
                    max: 0.95,
                },
            )
            .with_smoother(SmoothingStyle::Linear(0.2)),
            tank_smear: FloatParam::new(
                "Tank Smear",
                0.8,
                FloatRange::Linear {
                    min: 0.5,
                    max: 0.95,
                },
            )
            .with_smoother(SmoothingStyle::Linear(0.2)),
            mix: FloatParam::new("Mix", 0.25, FloatRange::Linear { min: 0., max: 0.5 })
                .with_smoother(SmoothingStyle::Linear(50.))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
        }
    }
}
impl StereoFilter for DattorroReverb {
    fn process_stereo(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let (out_l, out_r) = self.process_stereo(input_l, input_r);
         (out_l * self.mix + input_l * (1.0 - self.mix), out_r * self.mix + input_r * (1.0 - self.mix))
    }

    fn set_param(&mut self, param_id: &'static str, val: (f32, f32)) {
        match param_id {
            "sample_rate" => self.set_sample_rate(val.0),
            "decay" => self.set_decay(val.0),
            "pre_delay" => self.set_pre_delay(val.0),
            "damping" => self.set_damping(val.0),
            "brightness" => self.set_brightness(val.0),
            "input_smear" => self.set_input_smear(val.0),
            "tank_smear" => self.set_tank_smear(val.0),
            "lushness" => self.set_lushness(val.0),
            "size" => self.set_size(val.0),
            "mix" => self.set_mix(val.0),
            _ => (),
        }
    }
}
/// A reverb network implemented from the Dattorro Reverb design paper:
/// https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf
///
/// Usage:
/// ```
/// use revvex::filters::dattorro::DattorroReverb;
///
/// let mut reverb = DattorroReverb::new(44100., 0.5, 0.1, 0.7, 0.8, 0.65, 0.8, 8., 2.);
/// let (l, r) = reverb.process_stereo(0.5, 0.5);
///
/// ```
#[derive(Clone)]
pub struct DattorroReverb {
    mix: f32,
    sample_rate: f32,
    pre_delay_line: DelayLine,
    bandwidth_damper: Damper,
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
    // Parameters
    brightness: f32,
    pre_delay: f32,
    decay: f32,
    input_smear: f32,
    tank_smear: f32,
    damping: f32,
    lushness: f32,
    size: f32,
}

impl DattorroReverb {
    /// Create a new reverb instance with a sample rate and an initial decay factor
    pub fn new(
        mix: f32,
        sample_rate: f32,
        decay: f32,
        pre_delay: f32,
        damping: f32,
        mut brightness: f32,
        input_smear: f32,
        tank_smear: f32,
        lushness: f32,
        size: f32,
    ) -> Self {
        if brightness > 0.95 {
            brightness = 0.95;
        }
        let mut pre_delay_line = DelayLine::new(
            (sample_rate * MAX_SIZE) as usize,
            (pre_delay * sample_rate) as usize,
        );
        pre_delay_line.set_delay(pre_delay, sample_rate);

        Self {
            mix,
            sample_rate,
            pre_delay_line,
            bandwidth_damper: Damper::new(brightness),
            input_diffusor_1: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_1 as f32 * size) as usize,
                input_smear,
            ),
            input_diffusor_2: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_2 as f32 * size) as usize,
                input_smear,
            ),
            input_diffusor_3: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_3 as f32 * size) as usize,
                input_smear,
            ),
            input_diffusor_4: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_4 as f32 * size) as usize,
                input_smear,
            ),
            decay_diffusor_l: DecayDiffusor::new(
                sample_rate,
                (DECAY_DIFFUSOR_SIZE_L as f32 * size) as usize,
                tank_smear,
                lushness,
            ),
            decay_diffusor_r: DecayDiffusor::new(
                sample_rate,
                (DECAY_DIFFUSOR_SIZE_R as f32 * size) as usize,
                tank_smear,
                lushness,
            ),
            input_diffusor_l: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_L as f32 * size) as usize,
                input_smear,
            ),
            input_diffusor_r: InputDiffusor::new(
                (INPUT_DIFFUSOR_SIZE_R as f32 * size) as usize,
                input_smear,
            ),
            damper_l: Damper::new(damping),
            damper_r: Damper::new(damping),
            delay_line_1_l: DelayLine::new(
                (DELAY_LINE_SIZE_L_1 as f32 * MAX_SIZE) as usize,
                (DELAY_LINE_SIZE_L_1 as f32 * size) as usize,
            ),
            delay_line_2_l: DelayLine::new(
                (DELAY_LINE_SIZE_L_2 as f32 * MAX_SIZE) as usize,
                (DELAY_LINE_SIZE_L_2 as f32 * size) as usize,
            ),
            delay_line_1_r: DelayLine::new(
                (DELAY_LINE_SIZE_R_1 as f32 * MAX_SIZE) as usize,
                (DELAY_LINE_SIZE_R_1 as f32 * size) as usize,
            ),
            delay_line_2_r: DelayLine::new(
                (DELAY_LINE_SIZE_R_2 as f32 * MAX_SIZE) as usize,
                (DELAY_LINE_SIZE_R_2 as f32 * size) as usize,
            ),
            brightness,
            pre_delay,
            decay,
            input_smear,
            tank_smear,
            damping,
            lushness,
            size,
        }
    }

    /// Process a stereo signal through the reverb
    ///
    /// It will return the processed signal as a stereo pair.
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let input = (l + r) / 2.;
        let pre_delayed = self.pre_delay_line.process(input);
        let bandwith_damped = self.bandwidth_damper.process(pre_delayed);

        // Mono block
        let mut signal = bandwith_damped;
        signal = self.input_diffusor_1.process(signal);
        signal = self.input_diffusor_2.process(signal);
        signal = self.input_diffusor_3.process(signal);
        signal = self.input_diffusor_4.process(signal);

        // Start of stereo tank
        let feedback_l = self.delay_line_2_r.get();
        let feedback_r = self.delay_line_2_l.get();

        let mut tank_l = signal * 0.5 + feedback_l;
        let mut tank_r = signal * 0.5 + feedback_r;

        tank_l = self.decay_diffusor_l.process(tank_l);
        tank_r = self.decay_diffusor_r.process(tank_r);

        let left_init_tap: f32 = tank_l;
        let right_init_tap: f32 = tank_r;

        tank_l = self.delay_line_1_l.process(tank_l);
        tank_r = self.delay_line_1_r.process(tank_r);

        tank_l = self.damper_l.process(tank_l) * self.decay;
        tank_r = self.damper_r.process(tank_r) * self.decay;

        tank_l = self.input_diffusor_l.process(tank_l);
        tank_r = self.input_diffusor_r.process(tank_r);

        self.delay_line_2_l.process(tank_l);
        self.delay_line_2_r.process(tank_r);

        self.output(left_init_tap, right_init_tap)
    }

    /// Calculate the output from the taps with two inital taps
    pub fn output(&self, left_init: f32, right_init: f32) -> (f32, f32) {
        // The delay lengths are all from the Dattorro paper
        let y_l = left_init
            + self
                .delay_line_1_r
                .get_with_delay((TAP_LEFT_1 as f32 * self.size) as usize)
            + self
                .delay_line_1_r
                .get_with_delay((TAP_LEFT_2 as f32 * self.size) as usize)
            - self
                .input_diffusor_r
                .tap_at((TAP_LEFT_3 as f32 * self.size) as usize)
            + self
                .delay_line_2_r
                .get_with_delay((TAP_LEFT_4 as f32 * self.size) as usize)
            - self
                .delay_line_1_l
                .get_with_delay((TAP_LEFT_5 as f32 * self.size) as usize)
            - self
                .input_diffusor_l
                .tap_at((TAP_LEFT_6 as f32 * self.size) as usize)
            - self
                .delay_line_2_l
                .get_with_delay((TAP_LEFT_7 as f32 * self.size) as usize);

        let y_r = right_init
            + self
                .delay_line_1_l
                .get_with_delay((TAP_RIGHT_1 as f32 * self.size) as usize)
            + self
                .delay_line_1_l
                .get_with_delay((TAP_RIGHT_2 as f32 * self.size) as usize)
            - self
                .input_diffusor_l
                .tap_at((TAP_RIGHT_3 as f32 * self.size) as usize)
            + self
                .delay_line_2_l
                .get_with_delay((TAP_RIGHT_4 as f32 * self.size) as usize)
            - self
                .delay_line_1_r
                .get_with_delay((TAP_RIGHT_5 as f32 * self.size) as usize)
            - self
                .input_diffusor_r
                .tap_at((TAP_RIGHT_6 as f32 * self.size) as usize)
            - self
                .delay_line_2_r
                .get_with_delay((TAP_RIGHT_7 as f32 * self.size) as usize);

        (y_l, y_r)
    }

    /// Set the decay factor of the reverb
    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay.clamp(0.0, 0.98);
    }
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.);
    }
    pub fn set_pre_delay(&mut self, pre_delay: f32) {
        self.pre_delay = pre_delay;
        self.pre_delay_line.set_delay(pre_delay, self.sample_rate);
    }

    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping;
        self.damper_l.set_damping(damping);
        self.damper_r.set_damping(damping);
    }

    pub fn set_brightness(&mut self, mut brightness: f32) {
        if brightness > 0.95 {
            brightness = 0.95;
        }
        self.brightness = brightness;
        self.bandwidth_damper.set_damping(brightness);
    }

    pub fn set_input_smear(&mut self, input_smear: f32) {
        self.input_smear = input_smear;
        self.input_diffusor_1.set_gain(input_smear);
        self.input_diffusor_2.set_gain(input_smear);
        self.input_diffusor_3.set_gain(input_smear);
        self.input_diffusor_4.set_gain(input_smear);
        self.input_diffusor_l.set_gain(input_smear);
        self.input_diffusor_r.set_gain(input_smear);
    }

    pub fn set_tank_smear(&mut self, tank_smear: f32) {
        self.tank_smear = tank_smear;
        self.decay_diffusor_l.set_gain(tank_smear);
        self.decay_diffusor_r.set_gain(tank_smear);
    }

    pub fn set_lushness(&mut self, lushness: f32) {
        self.lushness = lushness;
        self.decay_diffusor_l.set_excursion_depth(lushness);
        self.decay_diffusor_r.set_excursion_depth(lushness);
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size;
        /*self.pre_delay_line
        .set_size((size * self.sample_rate) as usize);*/
        self.delay_line_1_l
            .set_size((DELAY_LINE_SIZE_L_1 as f32 * size) as usize);
        self.delay_line_1_r
            .set_size((DELAY_LINE_SIZE_R_1 as f32 * size) as usize);
        self.delay_line_2_l
            .set_size((DELAY_LINE_SIZE_L_2 as f32 * size) as usize);
        self.delay_line_2_r
            .set_size((DELAY_LINE_SIZE_R_2 as f32 * size) as usize);

        self.input_diffusor_1
            .set_size((INPUT_DIFFUSOR_SIZE_1 as f32 * size) as usize);
        self.input_diffusor_2
            .set_size((INPUT_DIFFUSOR_SIZE_2 as f32 * size) as usize);
        self.input_diffusor_3
            .set_size((INPUT_DIFFUSOR_SIZE_3 as f32 * size) as usize);
        self.input_diffusor_4
            .set_size((INPUT_DIFFUSOR_SIZE_4 as f32 * size) as usize);
        self.input_diffusor_l
            .set_size((INPUT_DIFFUSOR_SIZE_L as f32 * size) as usize);
        self.input_diffusor_r
            .set_size((INPUT_DIFFUSOR_SIZE_R as f32 * size) as usize);

        self.decay_diffusor_l
            .set_size((DECAY_DIFFUSOR_SIZE_L as f32 * size) as usize);
        self.decay_diffusor_r
            .set_size((DECAY_DIFFUSOR_SIZE_R as f32 * size) as usize);
    }

    /// Update the sample rate of everything.
    /// Important: This will reset the delay lines, since their maximum size is based on the sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.pre_delay_line =
            DelayLine::new((sample_rate * MAX_SIZE) as usize, sample_rate as usize);
        self.pre_delay_line.set_delay(self.pre_delay, sample_rate);
        self.decay_diffusor_l.set_sample_rate(sample_rate);
        self.decay_diffusor_r.set_sample_rate(sample_rate);
    }
}

#[derive(Debug, Clone)]
/// A general purpose delay line that only supports delay lengths as samples
pub struct DelayLine {
    buffer: Vec<f32>,
    current_capacity: usize,
    write_index: usize,
    max_capacity: usize,
}

impl DelayLine {
    /// Create a new delay line with a maximum delay length
    pub fn new(max_capacity: usize, current_capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; max_capacity],
            current_capacity: current_capacity.min(max_capacity),
            write_index: 0,
            max_capacity,
        }
    }

    /// Set the delay length of the delay line
    pub fn set_delay(&mut self, delay_s: f32, sample_rate: f32) {
        self.current_capacity = ((delay_s * sample_rate) as usize)
            .min(self.max_capacity)
            .max(1);
    }

    /// Process a sample through the delay line
    ///
    /// This is the same as get() and then insert()
    pub fn process(&mut self, input: f32) -> f32 {
        // let delayed_index = (self.write_index as i32 - self.current_capacity as i32)
        //     .rem_euclid(self.current_capacity as i32) as usize;
        let delayed = self.buffer[self.write_index];

        self.buffer[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.current_capacity;

        delayed
    }

    /// Get the delayed sample at the current delay length
    ///
    /// get() and insert() together are the same as process()
    pub fn get(&self) -> f32 {
        // let delayed_index = (self.write_index as i32 - self.current_capacity as i32)
        //     .rem_euclid(self.current_capacity as i32) as usize;
        self.buffer[self.write_index]
    }

    /// Get the delayed sample at a specific delay length
    pub fn get_with_delay(&self, delay: usize) -> f32 {
        let delay = delay % self.current_capacity;
        let delayed_index = (self.write_index as i32 - delay as i32)
            .rem_euclid(self.current_capacity as i32) as usize;
        self.buffer[delayed_index]
    }

    /// Insert a sample into the delay line
    pub fn insert(&mut self, input: f32) {
        self.buffer[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.current_capacity;
    }

    pub fn set_size(&mut self, size: usize) {
        self.current_capacity = size.min(self.max_capacity).max(1);
        if self.write_index >= self.current_capacity {
            self.write_index = 0;
        }
    }
}

#[derive(Clone)]
/// An input diffusor with a structure taken from the Dattorro paper. It acts as an all pass filter.
pub struct InputDiffusor {
    delay_line: DelayLine,
    gain: f32,
}

impl InputDiffusor {
    /// Create a new input diffusor with a delay length and gain
    pub fn new(delay: usize, gain: f32) -> Self {
        Self {
            delay_line: DelayLine::new((delay as f32 * MAX_SIZE) as usize, delay),
            gain,
        }
    }

    /// Process a sample through the input diffusor
    pub fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay_line.get();
        let in_changed = input + -(delayed * self.gain);

        self.delay_line.insert(in_changed);

        delayed + in_changed * self.gain
    }

    /// Tap the delay line at position 0
    #[allow(dead_code)]
    pub fn tap(&self) -> f32 {
        self.delay_line.get_with_delay(0)
    }

    pub fn tap_at(&self, pos: usize) -> f32 {
        self.delay_line.get_with_delay(pos)
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn set_size(&mut self, size: usize) {
        self.delay_line.set_size(size);
    }
}

#[derive(Clone)]
/// A diffusor that allows modulation of the delay length and has a slightly different structure from [InputDiffusor]
pub struct DecayDiffusor {
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
    /// Create a new decay diffusor with a delay length, gain, and sample rate
    pub fn new(sample_rate: f32, delay: usize, gain: f32, excursion_depth: f32) -> Self {
        Self {
            delay_line: DelayLine::new(((delay + 16) as f32 * MAX_SIZE) as usize, delay + 16),
            delay,
            gain,
            excursion: 0.,
            excursion_tick: 0.,
            excursion_rate: 1.,
            excursion_depth,
            sample_rate,
        }
    }

    /// Process a sample through the decay diffusor
    pub fn process(&mut self, input: f32) -> f32 {
        // Update excursion and delay length
        self.modulate_excursion();

        let delayed = self
            .delay_line
            .get_with_delay(self.delay + self.excursion.floor() as usize);
        let in_changed = input - delayed * self.gain;

        self.delay_line.insert(in_changed);

        delayed + (in_changed * self.gain)
    }

    /// Modulates the excursion for each sample at a specific rate
    pub fn modulate_excursion(&mut self) {
        self.excursion = (self.excursion_tick * self.excursion_rate * std::f32::consts::TAU).sin()
            * self.excursion_depth;
        self.excursion_tick += 1. / self.sample_rate;
    }

    /// Set the sample rate of the decay diffusor
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn set_excursion_depth(&mut self, depth: f32) {
        self.excursion_depth = depth;
    }

    pub fn set_size(&mut self, size: usize) {
        self.delay_line.set_size(size + 16);
        self.delay = size;
    }
}

#[derive(Clone)]
/// A simple damper that smooths the signal using a damping factor.
///
/// Structure is from the Dattorro paper.
pub struct Damper {
    last_sample: f32,
    damping: f32,
}

impl Damper {
    /// Create a new damper with a damping factor
    pub fn new(damping: f32) -> Self {
        Self {
            last_sample: 0.,
            damping,
        }
    }

    /// Process a sample through the damper
    pub fn process(&mut self, input: f32) -> f32 {
        let out = input * (1. - self.damping) + self.last_sample * self.damping;
        self.last_sample = out;
        out
    }

    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping;
    }
}

#[cfg(test)]
mod dattorro_tests {
    use super::*;
    #[test]
    fn delay_line() {
        let mut delay_line = DelayLine::new((4_f32 * MAX_SIZE) as usize, 4);
        delay_line.set_delay(2., 1.);
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
