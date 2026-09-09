use crate::params::DelaxParams;
use crate::slint_ui;
use crate::slint_ui::connection::InputData;
use crate::slint_ui::elements::{ElementId, GpuElementData};
use crate::slint_ui::param_component::ParamComponent;
use crate::slint_ui::plug_con::host::SlintHost;
use crate::slint_ui::renderer::WgpuRegistry;
use baseview::dpi::PhysicalSize;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{Receiver, Sender, unbounded};
use nice_plug::context::gui::GuiContext;
use nice_plug::params::Params;
use nice_plug::params::persist::PersistentField;
use serde::{Deserialize, Serialize};
use slint::{PlatformError, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use nice_plug::prelude::ParamPtr;
use crate::slint_ui::param_store;
use slint::private_unstable_api::re_exports::ApproxEq;

pub enum UiEvent {
    ParamChanged {
        id: String,
        value: f32,
    },
    SetDiv {
        div_id: String,
        bpm_id: String,
        factor: f32,
    },
    SetTimeMode {
        bpm_id: String,
    },
}
#[derive(Deserialize, Serialize)]
pub struct EditorState {
    #[serde(with = "nice_plug::params::persist::serialize_atomic_cell")]
    pub editor_size: AtomicCell<(u32, u32)>,
    pub title: String,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            editor_size: AtomicCell::new((550, 350)),
            title: String::from("Audio Plugin"),
        }
    }
}

impl EditorState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            editor_size: AtomicCell::new((width, height)),
            title: String::from("Audio Plugin"),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.editor_size.load()
    }

    pub fn physical_size(&self) -> PhysicalSize<u32> {
        let (w, h) = self.size();
        PhysicalSize::new(w, h)
    }
}

impl<'a> PersistentField<'a, EditorState> for Arc<EditorState> {
    fn set(&self, new_value: EditorState) {
        self.editor_size.store(new_value.editor_size.load())
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&EditorState) -> R,
    {
        f(self)
    }
}

pub struct DelaxSlintHost {
    params: Arc<DelaxParams>,
    data: Arc<InputData>,
    event_tx: Sender<UiEvent>,
    event_rx: Receiver<UiEvent>,
    param_index: HashMap<String, ParamPtr>
}

impl DelaxSlintHost {
    pub fn new(params: Arc<DelaxParams>, input_data: Arc<InputData>) -> Self {
        let (event_tx, event_rx) = unbounded();
        let param_index = params.param_map().into_iter().map(|(id, ptr, _)| (id, ptr)).collect();

        // Build param cache once on creation
        for (p_id, param_ptr, _) in params.param_map().iter() {
            let val = unsafe { param_ptr.unmodulated_normalized_value() };
            let display_val =
                unsafe {
                    SharedString::from(param_ptr.normalized_value_to_string(val, true))
                };

            param_store().write().unwrap().insert(p_id.clone(), (val, display_val));
        }
        Self {
            params,
            data: input_data,
            event_tx,
            event_rx,
            param_index
        }
    }

    fn sync_params_to_ui(&self, app: &slint_ui::AppWindow) {
        use slint_ui::param_component::ParamComponent;

        for (p_id, param_ptr) in self.param_index.iter() {
            let val = unsafe { param_ptr.unmodulated_normalized_value() };

            // Check the cache before doing string stuff, set_param_from_host updates the cache
            let cached = param_store().read().unwrap().get(p_id).cloned();
            let display_val = match cached {
                Some((cache_val, cache_display)) if cache_val.approx_eq(&val) => cache_display,
                _ => unsafe {
                    SharedString::from(param_ptr.normalized_value_to_string(val, true))
                }
            };
            <slint_ui::AppWindow as ParamComponent<DelaxParams>>::set_param_from_host(
                app,
                p_id,
                val,
                display_val,
            );
        }

        // TODO: Very dirty way of generating the labels. This should be done together somewhere with the params
        let count_l = self.params.delay_params.delay_note_l.value();
        let div_l = self.params.delay_params.delay_div_l.value();
        let factor_l = div_l.factor();
        let suffix_l = div_l.suffix();
        let display_l = {
            let c = (count_l * 10.0).round() / 10.0;
            if c.fract().abs() < 0.0005 {
                format!("{} {}", c as i32, suffix_l)
            } else {
                format!("{:.1} {}", c, suffix_l)
            }
        };
        app.set_timing_display_l(display_l.into());
        app.set_timing_factor_l(factor_l);
        let count_r = self.params.delay_params.delay_note_r.value();
        let div_r = self.params.delay_params.delay_div_r.value();
        let factor_r = div_r.factor();
        let suffix_r = div_r.suffix();
        let display_r = {
            let c = (count_r * 10.0).round() / 10.0;
            if c.fract().abs() < 0.0005 {
                format!("{} {}", c as i32, suffix_r)
            } else {
                format!("{:.1} {}", c, suffix_r)
            }
        };
        app.set_timing_display_r(display_r.into());
        app.set_timing_factor_r(factor_r);
    }

    fn render_vis(&self, app: &<DelaxSlintHost as SlintHost>::Component, wgpu: &RefCell<WgpuRegistry>) {
        let mut textures = app.get_textures();
        let mut registry = wgpu.borrow_mut();
        for &element in ElementId::ALL {
            let Some(spec) = element.spec() else { continue; };
            registry.register(element, spec);
            let Some(uniforms) = self.data.element_uniform(element) else { continue; };
            let (w, h) = element.default_size();
            let Some(image) = registry.render_to_image(element, w, h, &uniforms) else { continue; };
            match element {
                ElementId::Buffer => {textures.buffer = image.into()},
                ElementId::Spectrum => {textures.spectrum = image.into()}
                ElementId::Decay => {textures.decay = image.into()}
                ElementId::Peak => {}
            }
        }
        app.set_textures(textures);
    }
}

impl SlintHost for DelaxSlintHost {
    type Component = slint_ui::AppWindow;

    fn on_init(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>) {
        self.render_vis(app, wgpu);
    }

    fn build(&self) -> Result<Self::Component, PlatformError> {
        let app = slint_ui::AppWindow::new()?;
        app.set_version(env!("CARGO_PKG_VERSION").into());
        app.bind_param_changed(self.event_tx.clone(), self.params.clone());
        //app.window().set_rendering_notifier(|state, api| {
        //})
        Ok(app)
    }

    fn on_event(&self, _app: &Self::Component, gui_context: &GuiContext) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::ParamChanged { id, value } => {
                    let normalized = value.clamp(0.0, 1.0);
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, normalized);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                            break;
                        }
                    }
                }
                UiEvent::SetDiv {
                    div_id,
                    bpm_id,
                    factor,
                } => {
                    use crate::delay_engine::params::NoteDiv;
                    let norm = NoteDiv::from_factor(factor).to_norm();
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &div_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, norm);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                        }
                        if param_id == &bpm_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, 1.0);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                        }
                    }
                }
                UiEvent::SetTimeMode { bpm_id } => {
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &bpm_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, 0.0);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    fn on_frame(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>) {
        self.sync_params_to_ui(app);
        self.data.update_ui(app);
        self.render_vis(app, wgpu);
    }

    fn on_resized(&self, width: u32, height: u32) {
        self.params.editor_state.editor_size.store((width, height));
    }
}
