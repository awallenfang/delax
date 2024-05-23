use std::sync::{Arc, Mutex};

use crate::filters::Filter;

/// A pipeline to send the signal through different filters in different orders
pub struct FilterPipeline {
    /// This holds the filter instances so that they can be called in order.
    registered_filters: Vec<FilterPipelineElement>,
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
        filter_l: Arc<Mutex<dyn Filter>>,
        filter_r: Arc<Mutex<dyn Filter>>,
    ) {
        self.registered_filters
            .push(FilterPipelineElement::Stereo(filter_l, filter_r));
        self.order.push(self.registered_filters.len() - 1);
    }

    /// Process a stereo signal through the stack of filters
    pub fn process_stereo(&self, input_l: f32, input_r: f32) -> (f32, f32) {
        let mut l = input_l;
        let mut r = input_r;

        for i in &self.order {
            match &self.registered_filters[*i] {
                FilterPipelineElement::Stereo(filter_l, filter_r) => {
                    l = filter_l.lock().unwrap().process(l);
                    r = filter_r.lock().unwrap().process(r);
                }
                FilterPipelineElement::Mono(_) => {}
            }
        }

        (l, r)
    }
}

#[allow(dead_code)]
/// A bundle of filter instances to be used in the pipeline
pub enum FilterPipelineElement {
    Stereo(Arc<Mutex<dyn Filter>>, Arc<Mutex<dyn Filter>>),

    Mono(Arc<Mutex<dyn Filter>>),
}
