pub mod param_component;
pub mod connection;
pub mod elements;
pub mod gpu_context;
pub mod renderer;
pub mod uniforms;
mod baseview_con;
mod slint_con;
pub mod plug_con;
pub mod editor;

use crate::slint_ui::elements::{ElementId, GpuImageSink};
use crate::slint_ui::param_component::ParamComponent;
use nice_plug::params::Params;
use slint::SharedString;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use crate::slint_ui::editor::UiEvent;

slint::include_modules!();

// Impls on the slint window
impl GpuImageSink for AppWindow {
    fn set_element_image(&self, element: ElementId, image: slint::Image) {
        match element {
            ElementId::Spectrum => self.set_spectrum_tex(image),
            ElementId::Buffer => self.set_buffer_tex(image),
            ElementId::Decay => self.set_decay_tex(image),
            _ => {}
        }
    }
}

static PARAM_STORE: OnceLock<RwLock<HashMap<String, (f32, SharedString)>>> = OnceLock::new();

pub fn param_store() -> &'static RwLock<HashMap<String, (f32, SharedString)>> {
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
        {
            let tx1 = tx.clone();
            bus.on_param_changed(move |param_id, new_val| {
                let _ = tx1.send(UiEvent::ParamChanged {
                    id: param_id.to_string(),
                    value: new_val,
                });
            });
        }
        {
            let tx2 = tx.clone();
            bus.on_set_div(move |div_id, bpm_id, factor| {
                let _ = tx2.send(UiEvent::SetDiv {
                    div_id: div_id.to_string(),
                    bpm_id: bpm_id.to_string(),
                    factor,
                });
            });
        }
        {
            let tx3 = tx.clone();
            bus.on_set_time_mode(move |bpm_id| {
                let _ = tx3.send(UiEvent::SetTimeMode {
                    bpm_id: bpm_id.to_string(),
                });
            });
        }
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
            let b = value > 0.3;
            let ping_pong = value > 0.8;
            if self.get_is_stereo() != b {
                self.set_is_stereo(b);
            }
            if self.get_is_ping_pong() != ping_pong {
                self.set_is_ping_pong(ping_pong);
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
