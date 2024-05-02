use std::f32::consts::PI;

/// A SVF filter implemented using the paper by Andrew Simper from Cytomic
/// https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf
pub struct SimperTanSVF {
    ic1eq: f32,
    ic2eq: f32,
    cutoff: f32,
    sample_rate: f32,
    g: f32,
    q: f32,
    res: f32,
    k: f32,
    a1: f32,
    a2: f32,
}

impl SimperTanSVF {
    pub fn new(sample_rate: f32) -> Self {
        let ic1eq = 0.;
        let ic2eq = 0.;

        let cutoff = 500.;
        let g = (PI * cutoff / sample_rate).tan();

        // These can be fine-tuned
        // Q is taken from the paper
        // res is guessed for now
        let q = 0.5;
        let res = 0.9;

        let k = 2. - 2. * res;
        let a1 = 1. / (1. + g * (g * k));
        let a2 = g * a1;

        Self {
            ic1eq,
            ic2eq,
            cutoff,
            sample_rate,
            g,
            q,
            res,
            k,
            a1,
            a2,
        }
    }

    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff;
        self.reinit();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reinit();
    }

    fn reinit(&mut self) {
        self.g = (PI * self.cutoff / self.sample_rate).tan();

        self.a1 = 1. / (1. + self.g * (self.g * self.k));
        self.a2 = self.g * self.a2;
    }

    pub fn tick_sample(&mut self, sample: f32) -> f32 {
        let v1 = self.a1 * self.ic1eq + self.a2 * (sample - self.ic2eq);
        let v2 = self.ic2eq + self.g * v1;

        self.ic1eq = 2. * v1 - self.ic1eq;
        self.ic2eq = 2. * v2 - self.ic2eq;

        let low = v2;
        let band = v1;
        let high = sample - self.k * v1 - v2;
        let notch = sample - self.k * v1;
        let peak = 2. * v2 - sample + self.k * v1;
        let all = sample - 2. * self.k * v1;

        low
    }
}


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
    pub fn new(sample_rate: f32) -> Self {
        let ic1eq = 0.;
        let ic2eq = 0.;

        let cutoff = 500.;
        let w = PI * cutoff / sample_rate;

        // These can be fine-tuned
        // Q is taken from the paper
        // res is guessed for now
        let q = 0.5;
        let res = 0.9;

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

    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff;
        self.reinit();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reinit();
    }

    fn reinit(&mut self) {
        let w = PI * self.cutoff / self.sample_rate;
        
        let s1 = w.sin();
        let s2 = (2. * w).sin();

        let nrm = 1. / (2. + self.k *s2);

        self.g0 = s2 * nrm;
        self.g1 = (-2. * s1 * s1 - self.k * s2) * nrm;
        self.g2 = (2. * s1 * s1) * nrm;
    }

    pub fn tick_sample(&mut self, sample: f32) -> f32 {
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
        let notch = sample - self.k * v1;
        let all =sample - self.k * v1 - 2. * v2;
        low
    }
}
