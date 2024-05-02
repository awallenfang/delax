use std::f32::consts::PI;

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
}

impl SimperTanSVF {
    /// Create a new filter given a sample rate. This rate can be updated later on.
    /// 
    /// Usage:
    /// ```
    /// use delax::filters::SimperTanSVF;
    /// 
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample(0.4);
    /// ```
    pub fn new(sample_rate: f32) -> Self {
        let ic1eq = 0.;
        let ic2eq = 0.;

        let cutoff = 1000.;
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
        }
    }

    /// Set the cutoff value
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff;
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
        self.a2 = self.g * self.a2;
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
    /// use delax::filters::SimperTanSVF;
    /// 
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample(0.4);
    /// 
    /// let notch = low + high;
    /// let peak = low - high;
    /// ```
    pub fn tick_sample(&mut self, sample: f32) -> (f32, f32, f32) {
        let v1 = self.a1 * self.ic1eq + self.a2 * (sample - self.ic2eq);
        let v2 = self.ic2eq + self.g * v1;

        self.ic1eq = 2. * v1 - self.ic1eq;
        self.ic2eq = 2. * v2 - self.ic2eq;

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
    /// use delax::filters::SimperTanSVF;
    /// 
    /// let mut filter = SimperTanSVF::new(44100.);
    /// let all = filter.tick_sample_allpass(0.4);
    /// ```
    pub fn tick_sample_allpass(&mut self, sample: f32) -> f32 {
        let (low, band, high) = self.tick_sample(sample);
        low + high - self.k * band
    }

}

/// A SVF filter implemented using the paper by Andrew Simper from Cytomic
/// https://cytomic.com/files/dsp/SvfLinearTrapezoidalSin.pdf
pub struct SimperSinSVF {
    res: f32,
    cutoff: f32,
    sample_rate: f32,

    ic1eq: f32,
    ic2eq: f32,

    k: f32,
    g0: f32,
    g1: f32,
    g2: f32
}

impl SimperSinSVF {
    /// Create a new filter given a sample rate. This rate can be updated later on.
    /// 
    /// Usage:
    /// ```
    /// use delax::filters::SimperSinSVF;
    /// 
    /// let mut filter = SimperSinSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample(0.4);
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

        let nrm = 1. / (2. + k *s2);

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
            g2
        }
    }

    /// Set the cutoff value
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff;
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
        let w = PI * self.cutoff / self.sample_rate;

        self.k = 2. - 2. * self.res;
        
        let s1 = w.sin();
        let s2 = (2. * w).sin();

        let nrm = 1. / (2. + self.k *s2);

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
    /// use delax::filters::SimperSinSVF;
    /// 
    /// let mut filter = SimperSinSVF::new(44100.);
    /// let (low, band, high) = filter.tick_sample(0.4);
    /// 
    /// let notch = low + high;
    /// let peak = low - high;
    /// ```
    pub fn tick_sample(&mut self, sample: f32) -> (f32, f32, f32) {
        let t0 = sample - self.ic2eq;
        let t1 = self.g0 * t0 + self.g1*self.ic1eq;
        let t2 = self.g2 * t0 + self.g0 * self.ic1eq;
        let v1 = t1 + self.ic1eq;
        let v2 = t2 + self.ic2eq;

        self.ic1eq = self.ic1eq + 2. * t1;
        self.ic2eq = self.ic2eq + 2. * t2;

        let high = sample - self.k * v1 - v2;
        let band = v1;
        let low = v2;
        (low, band, high)
    }
}
