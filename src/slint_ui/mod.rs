pub mod editor;
pub mod param_component;
mod window_state;

use crate::slint_ui::editor::{EditorState, UIEditor, UiEvent};
use crate::slint_ui::param_component::ParamComponent;
use nice_plug::params::Params;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
slint::include_modules!();
thread_local! {
    static PARAM_STORE: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}
pub fn create_editor<P: Params + 'static>(
    editor_state: Arc<EditorState>,
    params: Arc<P>,
) -> UIEditor<AppWindow, P> {
    let params_for_loop = params.clone();

    UIEditor::<AppWindow, P>::new(
        editor_state,
        Arc::new({
            let params = params.clone();
            move |event_tx| {
                let app = AppWindow::new()?;
                app.bind_param_changed(event_tx, params.clone());
                Ok(app)
            }
        }),
        params.clone(),
    )
    .on_frame(move |app| {
        for (p_id, param_ptr, _) in params_for_loop.param_map().iter() {
            let val = unsafe { param_ptr.unmodulated_normalized_value() };
            <AppWindow as ParamComponent<P>>::set_param_from_host(app, p_id, val);
        }
    })
}

impl<P> ParamComponent<P> for AppWindow
where
    P: Params + 'static,
{
    fn bind_param_changed(&self, tx: crossbeam::channel::Sender<UiEvent>, params: Arc<P>) {
        let bus = self.global::<ParamBus>();
        PARAM_STORE.with(|store| {
            let mut map = store.borrow_mut();
            for (p_id, param_ptr, _) in params.param_map().iter() {
                map.insert(p_id.to_string(), unsafe {
                    param_ptr.unmodulated_normalized_value()
                });
            }
        });
        bus.on_param_changed(move |param_id, new_val| {
            let _ = tx.send(UiEvent::ParamChanged {
                id: param_id.to_string(),
                value: new_val,
            });
        });
        bus.on_get_val_by_key(|key, _version| {
            PARAM_STORE.with(|store| store.borrow().get(key.as_str()).copied().unwrap_or(0.0))
        });
    }

    fn set_param_from_host(&self, param_id: &str, value: f32) {
        let mut changed = false;

        PARAM_STORE.with(|store| {
            let mut map = store.borrow_mut();
            if map.get(param_id).copied() != Some(value) {
                map.insert(param_id.to_string(), value);
                changed = true;
            }
        });

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
        }

        if changed {
            // Update version to force UI update
            let bus = self.global::<ParamBus>();
            bus.set_version(bus.get_version().wrapping_add(1));
        }
    }
}
