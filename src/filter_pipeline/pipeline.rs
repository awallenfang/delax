use std::sync::{Arc, Mutex};

use crate::filters::{Filter, StereoFilter};

/// A pipeline to send the signal through different filters in different orders
pub struct FilterPipeline {
    /// This holds the filter instances so that they can be called in order.
    /// As well as an identifier for bypassing and if they're active
    registered_filters: Vec<(FilterPipelineElement, &'static str, bool)>,
    /// The order of the filters to be called.
    order: Vec<usize>,
}

impl FilterPipeline {
    /// Create a new filter pipeline without any filters
    pub fn new() -> Self {
        FilterPipeline {
            registered_filters: Vec::new(),
            order: Vec::new(),
        }
    }

    /// Register a stereo pair of filter instances
    pub fn register_stereo_pair(
        &mut self,
        filter_l: Box<dyn Filter>,
        filter_r: Box<dyn Filter>,
        id: &'static str,
    ) {
        self.registered_filters
            .push((FilterPipelineElement::StereoMonoFilter(filter_l, filter_r), id, true));
        self.order.push(self.registered_filters.len() - 1);
    }

    /// Register a stereo filter that's combined
    #[allow(dead_code)]
    pub fn register_stereo(&mut self, filter: Box<dyn StereoFilter>, id: &'static str) {
        self.registered_filters
            .push((FilterPipelineElement::StereoStereoFilter(filter), id, true));
        self.order.push(self.registered_filters.len() - 1);
    }

    /// Register a mono filter
    /// Running this in stereo will cast all audio data to mono
    #[allow(dead_code)]
    pub fn register_mono(&mut self, filter: Box<dyn Filter>,  id: &'static str) {
        self.registered_filters
            .push((FilterPipelineElement::Mono(filter), id, true));
        self.order.push(self.registered_filters.len() - 1);
    }

    /// Process a stereo signal through the stack of filters
    pub fn process_stereo(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let mut l = input_l;
        let mut r = input_r;

        for i in &self.order {
            match self.registered_filters[*i] {
                (FilterPipelineElement::StereoMonoFilter(ref mut filter_l, ref mut filter_r), _, active) => {
                    if active {
                        l = filter_l.process(l);
                        r = filter_r.process(r);
                    }
                }
                (FilterPipelineElement::StereoStereoFilter(ref mut filter), _, active) => {
                    if active {
                        let (new_l, new_r) = filter.process_stereo(l, r);
                        l = new_l;
                        r = new_r;
                    }
                }
                (FilterPipelineElement::Mono(ref mut filter), _, active) => {
                    if active {
                        // Running a mono filter in stereo will lose all stereo info
                        let mono = (l + r) / 2.0;
                        let out = filter.process(mono);
                        l = out;
                        r = out;
                    }
                }
            }
        }

        (l, r)
    }

    pub fn set_active(&mut self, id: &'static str, active: bool) {
        for e in self.registered_filters.iter_mut() {
            if e.1 == id {
                e.2 = active;
            }
        }
    }

    pub fn set_param(&mut self, id: &'static str, param: &'static str, val: f32) {
        for e in self.registered_filters.iter_mut() {
            if e.1 == id {
                match e.0 {
                    FilterPipelineElement::StereoMonoFilter(ref mut f_l, ref mut f_r) => {
                        f_l.set_param(param, val);
                        f_r.set_param(param, val);
                    }
                    FilterPipelineElement::StereoStereoFilter(ref mut f) => {
                        f.set_param(param, (val, val));
                    }
                    FilterPipelineElement::Mono(ref mut f) => {
                        f.set_param(param, val);
                    }
                }
            }
        }
    }
    pub fn set_param_stereo(&mut self, id: &'static str, param: &'static str, val: (f32, f32)) {
        for e in self.registered_filters.iter_mut() {
            if e.1 == id {
                match e.0 {
                    FilterPipelineElement::StereoMonoFilter(ref mut f_l, ref mut f_r) => {
                        f_l.set_param(param, val.0);
                        f_r.set_param(param, val.1);
                    }
                    FilterPipelineElement::StereoStereoFilter(ref mut f) => {
                        f.set_param(param, val);
                    }
                    FilterPipelineElement::Mono(ref mut f) => {
                        f.set_param(param, val.0);
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
/// A bundle of filter instances to be used in the pipeline
pub enum FilterPipelineElement {
    StereoMonoFilter(Box<dyn Filter>, Box<dyn Filter>),
    StereoStereoFilter(Box<dyn StereoFilter>),
    Mono(Box<dyn Filter>),
}
