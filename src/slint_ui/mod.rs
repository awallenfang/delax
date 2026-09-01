pub mod editor;
pub mod param_component;
mod window_state;

use crate::slint_ui::editor::UiEvent;
use crate::slint_ui::param_component::ParamComponent;
use nice_plug::params::Params;
use slint::SharedString;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

slint::include_modules!();

static PARAM_STORE: OnceLock<RwLock<HashMap<String, (f32, SharedString)>>> = OnceLock::new();

fn param_store() -> &'static RwLock<HashMap<String, (f32, SharedString)>> {
    PARAM_STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

impl<P> ParamComponent<P> for AppWindow
where
    P: Params + 'static,
{
    fn bind_param_changed(&self, tx: crossbeam::channel::Sender<UiEvent>, params: Arc<P>) {
        let bus = self.global::<ParamBus>();
        {
            let mut map = param_store().write().unwrap();
            for (p_id, param_ptr, _) in params.param_map().iter() {
                let val = unsafe { param_ptr.unmodulated_normalized_value() };
                let string = unsafe {
                    param_ptr
                        .normalized_value_to_string(param_ptr.unmodulated_normalized_value(), true)
                };
                map.insert(p_id.to_string(), (val, SharedString::from(string)));
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
                .read()
                .unwrap()
                .get(key.as_str())
                .cloned()
                .unwrap_or((0.0, SharedString::from("0.0")))
                .0
        });
        bus.on_get_display_val_by_key(|key, _version| {
            param_store()
                .read()
                .unwrap()
                .get(key.as_str())
                .cloned()
                .unwrap_or((0.0, SharedString::from("0.0")))
                .1
        });
    }

    fn set_param_from_host(&self, param_id: &str, value: f32, display: SharedString) {
        let mut changed = false;

        {
            let mut map = param_store().write().unwrap();
            if !map
                .get(param_id)
                .map(|(v, s)| *v == value && *s == display)
                .unwrap_or(false)
            {
                map.insert(param_id.to_string(), (value, display));
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
