use std::f32::consts::PI;

use super::{Filter, flush_denormal, params::SVFFilterMode};

/// A SVF filter implemented using the paper by Andrew Simper from Cytomic
/// https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf
pub struct SimperTanSVF {
    ic1eq: f32,
    ic2eq: f32,
    cutoff: f32,
    sample_rate: f32,
    g: f32,
    res: f32,
    k: f32,
    a1: f32,
    a2: f32,
    mode: SVFFilterMode,
}

impl SimperTanSVF {
    /// Create a new filter given a sample rate. This rate can be updated later on.
    ///
    /// Usage:
    /// ```
    /// use delax::filters::simper::SimperTanSVF;
    ///
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample_full(0.4);
    /// ```
    pub fn new(sample_rate: f32) -> Self {
        let ic1eq = 0.;
        let ic2eq = 0.;

        let cutoff = 500.;
        let res = 0.2;

        let g = (PI * cutoff / sample_rate).tan();

        // The values in k could be fine-tuned
        let k = 2. - 2. * res;

        let a1 = 1. / (1. + g * (g * k));
        let a2 = g * a1;

        Self {
            ic1eq,
            ic2eq,
            cutoff,
            sample_rate,
            g,
            res,
            k,
            a1,
            a2,
            mode: SVFFilterMode::Low,
        }
    }

    /// Set the cutoff value
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff.clamp(10., 0.49 * self.sample_rate);
        self.reinit();
    }

    /// Set the sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reinit();
    }

    /// Set the resonance value
    pub fn set_res(&mut self, res: f32) {
        self.res = res;
        self.reinit();
    }

    /// Recalculate all the held values.
    /// This should be called after a value like the resonance is changed.
    fn reinit(&mut self) {
        self.g = (PI * self.cutoff / self.sample_rate).tan();

        self.k = 2. - 2. * self.res;

        self.a1 = 1. / (1. + self.g * (self.g * self.k));
        self.a2 = self.g * self.a1;
    }

    /// Run the filter on a sample.
    ///
    /// This returns the values as (low, band, high).
    /// Other filter types can be calculated based on these as follows:
    ///
    /// notch = low + high
    ///
    /// peak = low - high
    ///
    /// For an all-pass filter use [SimperTanSVF::tick_sample_allpass()]
    ///
    /// Usage:
    /// ```
    /// use delax::filters::simper::SimperTanSVF;
    ///
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample_full(0.4);
    ///
    /// let notch = low + high;
    /// let peak = low - high;
    /// ```
    pub fn tick_sample_full(&mut self, sample: f32) -> (f32, f32, f32) {
        let v1 = self.a1 * self.ic1eq + self.a2 * (sample - self.ic2eq);
        let v2 = self.ic2eq + self.g * v1;

        self.ic1eq = flush_denormal(2. * v1 - self.ic1eq);
        self.ic2eq = flush_denormal(2. * v2 - self.ic2eq);

        let low = v2;
        let band = v1;
        let high = sample - self.k * v1 - v2;

        (low, band, high)
    }

    /// Run the filter on a sample in allpass mode.
    ///
    /// For all the other filter modes use [SimperTanSVF::tick_sample()].
    /// Usage:
    /// ```
    /// use delax::filters::simper::SimperTanSVF;
    ///
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let all = filter.tick_sample_allpass(0.4);
    /// ```
    pub fn tick_sample_allpass(&mut self, sample: f32) -> f32 {
        let (low, band, high) = self.tick_sample_full(sample);
        low + high - self.k * band
    }

    /// Run the filter using the model that is set internally
    pub fn tick_sample(&mut self, sample: f32) -> f32 {
        match self.mode {
            SVFFilterMode::Low => {
                let (low, _, _) = self.tick_sample_full(sample);
                low
            }
            SVFFilterMode::Band => {
                let (_, band, _) = self.tick_sample_full(sample);
                band
            }
            SVFFilterMode::High => {
                let (_, _, high) = self.tick_sample_full(sample);
                high
            }
            SVFFilterMode::Notch => {
                let (low, _, high) = self.tick_sample_full(sample);
                low + high
            }
            SVFFilterMode::Peak => {
                let (low, _, high) = self.tick_sample_full(sample);
                low - high
            }
        }
    }
}

/// A SVF filter implemented using the paper by Andrew Simper from Cytomic
/// https://cytomic.com/files/dsp/SvfLinearTrapezoidalSin.pdf
#[derive(Debug, Clone)]
pub struct SimperSinSVF {
    res: f32,
    cutoff: f32,
    sample_rate: f32,

    ic1eq: f32,
    ic2eq: f32,

    k: f32,
    g0: f32,
    g1: f32,
    g2: f32,

    mode: SVFFilterMode,
}

impl SimperSinSVF {
    /// Create a new filter given a sample rate. This rate can be updated later on.
    ///
    /// Usage:
    /// ```
    /// use delax::filters::simper::SimperSinSVF;
    ///
    /// let mut filter = SimperSinSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample_full(0.4);
    /// ```
    pub fn new(sample_rate: f32) -> Self {
        let ic1eq = 0.;
        let ic2eq = 0.;

        let cutoff = 500.;
        let w = PI * cutoff / sample_rate;

        let res = 0.2;

        // The values for k could be fine-tuned
        let k = 2. - 2. * res;

        let s1 = w.sin();
        let s2 = (2. * w).sin();

        let nrm = 1. / (2. + k * s2);

        let g0 = s2 * nrm;
        let g1 = (-2. * s1 * s1 - k * s2) * nrm;
        let g2 = (2. * s1 * s1) * nrm;

        Self {
            ic1eq,
            ic2eq,
            cutoff,
            sample_rate,
            res,
            k,
            g0,
            g1,
            g2,
            mode: SVFFilterMode::Low,
        }
    }

    /// Set the cutoff value
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff.clamp(10., 0.49 * self.sample_rate);
        self.reinit();
    }

    /// Set the sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reinit();
    }

    /// Set the resonance value
    pub fn set_res(&mut self, res: f32) {
        self.res = res;
        self.reinit();
    }

    pub fn set_mode(&mut self, mode: SVFFilterMode) {
        self.mode = mode;
    }

    /// Recalculate all the held values.
    /// This should be called after a value like the resonance is changed.
    fn reinit(&mut self) {
        let w = PI * self.cutoff / self.sample_rate;

        // Note: A res of 1 is very unstable for this delay, so it's limited using the lower. At 1.45 it's just still stable with res = 1.
        // self.k = 2. - 2. * self.res
        self.k = 2. - 1.45 * self.res;

        let s1 = w.sin();
        let s2 = (2. * w).sin();

        let nrm = 1. / (2. + self.k * s2);

        self.g0 = s2 * nrm;
        self.g1 = (-2. * s1 * s1 - self.k * s2) * nrm;
        self.g2 = (2. * s1 * s1) * nrm;
    }

    /// Run the filter on a sample.
    ///
    /// This returns the values as (low, band, high).
    /// Other filter types can be calculated based on these as follows:
    ///
    /// notch = low + high
    ///
    /// peak = low - high
    ///
    /// Usage:
    /// ```
    /// use delax::filters::simper::SimperSinSVF;
    ///
    /// let mut filter = SimperSinSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample_full(0.4);
    ///
    /// let notch = low + high;
    /// let peak = low - high;
    /// ```
    pub fn tick_sample_full(&mut self, sample: f32) -> (f32, f32, f32) {
        let t0 = sample - self.ic2eq;
        let t1 = self.g0 * t0 + self.g1 * self.ic1eq;
        let t2 = self.g2 * t0 + self.g0 * self.ic1eq;
        let v1 = t1 + self.ic1eq;
        let v2 = t2 + self.ic2eq;

        self.ic1eq = flush_denormal(self.ic1eq + 2. * t1);
        self.ic2eq = flush_denormal(self.ic2eq + 2. * t2);

        let high = sample - self.k * v1 - v2;
        let band = v1;
        let low = v2;
        (low, band, high)
    }

    /// Run the filter using the model that is set internally
    pub fn tick_sample(&mut self, sample: f32) -> f32 {
        match self.mode {
            SVFFilterMode::Low => {
                let (low, _, _) = self.tick_sample_full(sample);
                low
            }
            SVFFilterMode::Band => {
                let (_, band, _) = self.tick_sample_full(sample);
                band
            }
            SVFFilterMode::High => {
                let (_, _, high) = self.tick_sample_full(sample);
                high
            }
            SVFFilterMode::Notch => {
                let (low, _, high) = self.tick_sample_full(sample);
                low + high
            }
            SVFFilterMode::Peak => {
                let (low, _, high) = self.tick_sample_full(sample);
                low - high
            }
        }
    }
}

impl Filter for SimperSinSVF {
    fn process(&mut self, input: f32) -> f32 {
        self.tick_sample(input)
    }
}

#[cfg(test)]
mod tests {
    use super::{SimperSinSVF, SimperTanSVF};
    use crate::filters::params::SVFFilterMode;

    fn assert_finite(samples: &[f32]) {
        for &s in samples {
            assert!(s.is_finite(), "non-finite sample {s}");
            assert!(!s.is_nan());
        }
    }

    #[test]
    fn sin_svf_extreme_cutoff_and_res_no_nan() {
        let sample_rates = [8000., 44100., 48000., 96000., 192000.];
        let cutoffs = [20., 500., 5000., 10000., 15000., 20000.];
        let ress = [0., 0.5, 0.9, 1.0];
        let modes = [
            SVFFilterMode::Low,
            SVFFilterMode::Band,
            SVFFilterMode::High,
            SVFFilterMode::Notch,
            SVFFilterMode::Peak,
        ];

        for &sr in &sample_rates {
            for &cutoff in &cutoffs {
                if cutoff >= sr * 0.49 {
                    continue;
                }
                for &res in &ress {
                    for &mode in &modes {
                        let mut f = SimperSinSVF::new(sr);
                        f.set_mode(mode);
                        f.set_cutoff(cutoff);
                        f.set_res(res);
                        let mut out = Vec::with_capacity(256);
                        for i in 0..256 {
                            let input = if i == 0 {
                                1.0
                            } else if i % 2 == 0 {
                                0.5
                            } else {
                                -0.5
                            };
                            out.push(f.tick_sample(input));
                        }
                        assert_finite(&out);
                        for &s in &out {
                            assert!(
                                s.abs() < 100.,
                                "explosion: sr={sr} cutoff={cutoff} res={res} mode={mode:?} sample={s}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sin_svf_long_run_stability_silence() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(1000.);
        f.set_res(0.9);
        f.set_mode(SVFFilterMode::Low);
        f.tick_sample(1.0);
        let mut max = 0f32;
        for _ in 0..1_000_000 {
            let s = f.tick_sample(0.0);
            assert!(s.is_finite(), "NaN/Inf after long silence");
            max = max.max(s.abs());
        }
        assert!(max < 10., "max {max} too large");
        let tail = f.tick_sample(0.0);
        assert!(tail.abs() < 1e-3, "tail should decay, got {tail}");
    }

    #[test]
    fn sin_svf_zero_cutoff_freezes_but_stays_finite() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(0.);
        f.set_res(0.2);
        for _ in 0..100 {
            let s = f.tick_sample(1.0);
            assert!(s.is_finite());
        }
        let s1 = f.tick_sample(1.0);
        let s2 = f.tick_sample(1.0);
        assert!(s1.is_finite() && s2.is_finite());
    }

    #[test]
    fn sin_svf_nyquist_adjacent_stays_finite() {
        for sr in [44100., 48000., 96000.] {
            let mut f = SimperSinSVF::new(sr);
            f.set_cutoff(sr * 0.45);
            f.set_res(1.0);
            for _ in 0..1024 {
                let s = f.tick_sample(0.7);
                assert!(
                    s.is_finite(),
                    "nyquist blowup sr={sr} cutoff={} res=1 got {s}",
                    sr * 0.45
                );
            }
        }
    }

    #[test]
    fn sin_svf_high_res_does_not_explode_on_dc() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(500.);
        f.set_res(1.0);
        f.set_mode(SVFFilterMode::Low);
        for _ in 0..10000 {
            let s = f.tick_sample(1.0);
            assert!(s.is_finite());
            assert!(s.abs() < 5., "DC explosion at res=1: {s}");
        }
    }

    #[test]
    fn sin_svf_mode_switch_mid_stream_finite() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(1000.);
        f.set_res(0.5);
        for mode in [
            SVFFilterMode::Low,
            SVFFilterMode::High,
            SVFFilterMode::Band,
            SVFFilterMode::Notch,
            SVFFilterMode::Peak,
        ] {
            f.set_mode(mode);
            for _ in 0..64 {
                assert!(f.tick_sample(0.3).is_finite());
            }
        }
    }

    #[test]
    fn sin_svf_sample_rate_change_reinit_finite() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(5000.);
        f.set_res(0.7);
        for _ in 0..100 {
            assert!(f.tick_sample(0.5).is_finite());
        }
        f.set_sample_rate(96000.);
        for _ in 0..100 {
            assert!(f.tick_sample(0.5).is_finite());
        }
        f.set_sample_rate(8000.);
        for _ in 0..100 {
            assert!(f.tick_sample(0.5).is_finite());
        }
    }

    #[test]
    fn tan_svf_basic_finite_for_reference() {
        let mut f = SimperTanSVF::new(44100.);
        for _ in 0..256 {
            let (l, b, h) = f.tick_sample_full(0.4);
            assert!(l.is_finite() && b.is_finite() && h.is_finite());
        }
        let all = f.tick_sample_allpass(0.4);
        assert!(all.is_finite());
    }

    #[test]
    fn sin_svf_impulse_decay_low_res() {
        let mut f = SimperSinSVF::new(44100.);
        f.set_cutoff(1000.);
        f.set_res(0.2);
        f.set_mode(SVFFilterMode::Low);
        let mut energy = 0f32;
        f.tick_sample(1.0);
        for _ in 0..44100 {
            energy += f.tick_sample(0.0).abs();
        }
        assert!(energy < 1000., "low-res impulse energy too high: {energy}");
    }

    fn sin_svf_gain(sr: f32, cutoff: f32, mode: SVFFilterMode, freq: f32) -> f32 {
        let mut f = SimperSinSVF::new(sr);
        f.set_mode(mode);
        f.set_cutoff(cutoff);
        f.set_res(0.2);
        let n = (sr * 0.2) as usize;
        let mut out_power = 0f64;
        let mut in_power = 0f64;
        for i in 0..n {
            let s = (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin();
            let o = f.tick_sample(s);
            if i > 1000 {
                out_power += (o as f64) * (o as f64);
                in_power += (s as f64) * (s as f64);
            }
        }
        ((out_power / in_power).sqrt()) as f32
    }

    #[test]
    fn sin_svf_cutoff_extremes_mode_correctness() {
        let sr = 44100.;
        let freq = 1000.;
        assert!(
            sin_svf_gain(sr, 10., SVFFilterMode::Low, freq) < 0.05,
            "low at 10Hz should cut 1k"
        );
        assert!(
            sin_svf_gain(sr, 20000., SVFFilterMode::Low, freq) > 0.9,
            "low at 20k should pass 1k"
        );
        assert!(
            sin_svf_gain(sr, 10., SVFFilterMode::High, freq) > 0.9,
            "high at 10Hz should pass 1k"
        );
        assert!(
            sin_svf_gain(sr, 20000., SVFFilterMode::High, freq) < 0.05,
            "high at 20k should cut 1k"
        );
        assert!(sin_svf_gain(sr, 10., SVFFilterMode::Band, freq) < 0.05);
        assert!(sin_svf_gain(sr, 20000., SVFFilterMode::Band, freq) < 0.05);
        assert!(
            sin_svf_gain(sr, 1000., SVFFilterMode::Band, freq) > 0.3,
            "band at 1k should pass 1k"
        );
        assert!(sin_svf_gain(sr, 10., SVFFilterMode::Notch, freq) > 0.9);
        assert!(sin_svf_gain(sr, 20000., SVFFilterMode::Notch, freq) > 0.9);
    }
}
