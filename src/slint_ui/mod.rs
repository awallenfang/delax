pub mod editor;
pub mod param_component;
mod window_state;

use crate::slint_ui::param_component::ParamComponent;
use crate::slint_ui::editor::UiEvent;
use nice_plug::params::Params;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
slint::include_modules!();

static PARAM_STORE: OnceLock<Mutex<HashMap<String, f32>>> = OnceLock::new();

fn param_store() -> &'static Mutex<HashMap<String, f32>> {
    PARAM_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl<P> ParamComponent<P> for AppWindow
where
    P: Params + 'static,
{
    fn bind_param_changed(&self, tx: crossbeam::channel::Sender<UiEvent>, params: Arc<P>) {
        let bus = self.global::<ParamBus>();
        {
            let mut map = param_store().lock().unwrap();
            for (p_id, param_ptr, _) in params.param_map().iter() {
                map.insert(p_id.to_string(), unsafe {
                    param_ptr.unmodulated_normalized_value()
                });
            }
        }
        bus.on_param_changed(move |param_id, new_val| {
            let _ = tx.send(UiEvent::ParamChanged {
                id: param_id.to_string(),
                value: new_val,
            });
        });
        bus.on_get_val_by_key(|key, _version| {
            param_store()
                .lock()
                .unwrap()
                .get(key.as_str())
                .copied()
                .unwrap_or(0.0)
        });
    }

    fn set_param_from_host(&self, param_id: &str, value: f32) {
        let mut changed = false;

        {
            let mut map = param_store().lock().unwrap();
            if map.get(param_id).copied() != Some(value) {
                map.insert(param_id.to_string(), value);
                changed = true;
            }
        }

        // Keep boolean switch properties in sync with host (EnumParam normalizes to 0.0 / 1.0)
        if param_id == "stereo" {
            let b = value > 0.5;
            if self.get_is_stereo() != b {
                self.set_is_stereo(b);
            }
        } else if param_id == "svf_stereo_mode" {
            let b = value > 0.5;
            if self.get_is_filter_stereo() != b {
                self.set_is_filter_stereo(b);
            }
        } else if param_id == "bpm_bound_l" {
            let b = value > 0.5;
            if self.get_is_bpm_bound_l() != b {
                self.set_is_bpm_bound_l(b);
            }
        } else if param_id == "bpm_bound_r" {
            let b = value > 0.5;
            if self.get_is_bpm_bound_r() != b {
                self.set_is_bpm_bound_r(b);
            }
        }

        if changed {
            // Update version to force UI update
            let bus = self.global::<ParamBus>();
            bus.set_version(bus.get_version().wrapping_add(1));
        }
    }
}
