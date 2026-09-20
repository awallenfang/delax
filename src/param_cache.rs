use rustc_hash::FxHashMap;

pub struct ParamCache {
    cache: FxHashMap<&'static str, f32>,
    epsilon: f32,
}

impl ParamCache {
    /// To avoid allocation make sure that the size is large enough for all cached params
    pub fn new(size: usize, epsilon: f32) -> Self {
        Self {
            cache: FxHashMap::with_capacity_and_hasher(size, Default::default()),
            epsilon,
        }
    }

    pub fn changed(&mut self, param_id: &'static str, val: f32) -> bool {
        if let Some(&cached) = self.cache.get(param_id) {
            if (cached - val).abs() < self.epsilon {
                return false;
            }
            self.cache.insert(param_id, val);
            true
        } else {
            self.cache.insert(param_id, val);
            true
        }
    }
}