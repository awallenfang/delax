use crate::filters::Filter;
use nice_plug::formatters::v2s_f32_hz_then_khz;
use nice_plug::params::FloatParam;
use nice_plug::prelude::*;

#[derive(Params)]
pub struct ShifterParams {
    #[id = "shimmer_stereo"]
    pub shimmer_stereo: BoolParam,
    #[id = "shimmer_shift_l"]
    pub shift_l: FloatParam,
    #[id = "shimmer_shift_r"]
    pub shift_r: FloatParam,
    #[id = "shimmer_mix"]
    pub shimmer_mix: FloatParam,
}

impl Default for ShifterParams {
    fn default() -> Self {
        Self {
            shimmer_stereo: BoolParam::new("Shimmer Stereo", false),
            shift_l: FloatParam::new(
                "Shift L",
                0.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 500.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.))
            .with_value_to_string(v2s_f32_hz_then_khz(0)),
            shift_r: FloatParam::new(
                "Shift R",
                0.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 500.,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.))
            .with_value_to_string(v2s_f32_hz_then_khz(0)),
            shimmer_mix: FloatParam::new("Mix", 1., FloatRange::Linear { min: 0., max: 1. })
                .with_smoother(SmoothingStyle::Linear(50.))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
        }
    }
}

impl Filter for FrequencyShifter {
    fn process(&mut self, input: f32) -> f32 {
        self.process(input)
    }

    fn set_param(&mut self, param_id: &'static str, val: f32) {
        match param_id {
            "shift" => self.set_frequency(val),
            "sample_rate" => self.set_sample_rate(val),
            "mix" => self.mix = val,
            _ => (),
        }
    }
}

#[derive(Clone)]
pub struct FrequencyShifter {
    hilbert: HilbertTransformer,
    phase: f32,
    frequency: f32,
    sample_rate: f32,
    mix: f32,
}

impl FrequencyShifter {
    pub(crate) fn new(sample_rate: f32, frequency: f32) -> Self {
        Self {
            phase: 0.,
            frequency,
            sample_rate,
            hilbert: HilbertTransformer::new(),
            mix: 1.
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        if self.frequency <= f32::EPSILON {
            return input;
        }
        let (i, q) = self.hilbert.process(input);

        let angle = 2. * std::f32::consts::PI * self.phase;
        // Upper sideband: i*cos - q*sin  → shifts up by +frequency
        // Lower sideband: i*cos + q*sin  → shifts down by -frequency
        let out = i * angle.cos() - q * angle.sin();

        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1. {
            self.phase -= 1.;
        }
        out * self.mix + input * (1. - self.mix)
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
}

#[derive(Clone)]
struct AllpassSection {
    x1: f32, // input delay
    y1: f32, // output delay
    coeff: f32,
}

impl AllpassSection {
    fn new(coeff: f32) -> Self {
        Self {
            x1: 0.,
            y1: 0.,
            coeff,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = self.coeff * (input - self.y1) + self.x1;
        self.x1 = input;
        self.y1 = out;
        out
    }
}

#[derive(Clone)]
struct HilbertTransformer {
    chain_i: [AllpassSection; 3],
    chain_q: [AllpassSection; 4],
}
// Coefficients generated with https://github.com/mattiadif/IIR-hilbert-transformer
// warp   = True
// filter = ap(44100, 500, 120, 20000, warping=warp)
impl HilbertTransformer {
    fn new() -> Self {
        Self {
            chain_i: [
                AllpassSection::new(-0.70717588),
                AllpassSection::new(0.17096207),
                AllpassSection::new(-0.34029589),
            ],
            chain_q: [
                AllpassSection::new(-0.89542539),
                AllpassSection::new(0.62902105),
                AllpassSection::new(-0.5283841),
                AllpassSection::new(-0.12036714),
            ],
        }
    }

    /// Returns (I, Q) — a quadrature pair
    fn process(&mut self, input: f32) -> (f32, f32) {
        let mut i = input;
        let mut q = input;

        for section in &mut self.chain_i {
            i = section.process(i);
        }
        for section in &mut self.chain_q {
            q = section.process(q);
        }

        (i, q)
    }
}

#[cfg(test)]
mod hilbert_tests {
    use super::*;
    use std::f32::consts::PI;
    fn wrap_phase(deg: f32) -> f32 {
        let mut d = deg % 360.;
        if d > 180. {
            d -= 360.;
        }
        if d < -180. {
            d += 360.;
        }
        d
    }
    fn measure_phase_difference(frequency_hz: f32, sample_rate: f32) -> f32 {
        let mut h = HilbertTransformer::new();

        let settle_samples = 16384;
        let cycles = 10.;
        let measure_samples = (cycles * sample_rate / frequency_hz).round() as usize;

        let mut i_out = vec![0f32; measure_samples];
        let mut q_out = vec![0f32; measure_samples];

        for n in 0..(settle_samples + measure_samples) {
            let input = (2. * PI * frequency_hz * n as f32 / sample_rate).sin();
            let (i, q) = h.process(input);
            if n >= settle_samples {
                let idx = n - settle_samples;
                i_out[idx] = i;
                q_out[idx] = q;
            }
        }

        // Compute phase of each output independently using DFT inner products
        // Phase = atan2(dot with sin, dot with cos) at the test frequency
        let phase_i = {
            let (s, c) = i_out
                .iter()
                .enumerate()
                .fold((0f32, 0f32), |(s, c), (n, &x)| {
                    let angle = 2. * PI * frequency_hz * n as f32 / sample_rate;
                    (s + x * angle.sin(), c + x * angle.cos())
                });
            s.atan2(c)
        };

        let phase_q = {
            let (s, c) = q_out
                .iter()
                .enumerate()
                .fold((0f32, 0f32), |(s, c), (n, &x)| {
                    let angle = 2. * PI * frequency_hz * n as f32 / sample_rate;
                    (s + x * angle.sin(), c + x * angle.cos())
                });
            s.atan2(c)
        };

        (phase_i - phase_q).to_degrees()
    }

    #[test]
    fn phase_difference_is_90_degrees_at_1khz() {
        let phase = wrap_phase(measure_phase_difference(1000., 44100.));

        assert!(
            (phase.abs() - 90.).abs() < 15.,
            "Expected ~90° phase difference at 1000 Hz, got {:.1}°",
            phase
        );
    }

    #[test]
    fn phase_difference_across_audio_band() {
        let sample_rate = 44100.;
        let test_frequencies = [1000., 2000., 5000., 10000., 15000., 18000.];

        for freq in test_frequencies {
            let phase = wrap_phase(measure_phase_difference(freq, sample_rate));
            assert!(
                (phase.abs() - 90.).abs() < 15.,
                "Expected ~90° phase difference at {freq}Hz, got {:.1}°",
                phase
            );
        }
    }

    #[test]
    fn outputs_are_not_silent() {
        let mut h = HilbertTransformer::new();
        let mut any_nonzero = false;

        for n in 0..1024 {
            let input = (2. * PI * 440. * n as f32 / 44100.).sin();
            let (i, q) = h.process(input);
            if i.abs() > 1e-6 || q.abs() > 1e-6 {
                any_nonzero = true;
                break;
            }
        }
        assert!(any_nonzero, "Both outputs were silent");
    }

    #[test]
    fn dc_input_produces_no_output() {
        let mut h = HilbertTransformer::new();
        for _ in 0..8192 {
            let (i, q) = h.process(1.);
            assert!(i.is_finite(), "I output diverged on DC input");
            assert!(q.is_finite(), "Q output diverged on DC input");
        }
    }

    #[test]
    fn silence_in_silence_out() {
        let mut h = HilbertTransformer::new();
        for _ in 0..1024 {
            let (i, q) = h.process(0.);
            assert_eq!(i, 0., "Expected silence on I channel");
            assert_eq!(q, 0., "Expected silence on Q channel");
        }
    }
}
