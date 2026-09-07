use crate::params::DelaxParams;
use crate::{slint_ui, sync_params_to_ui};
use crate::slint_ui::connection::InputData;
use crate::slint_ui::elements::{ElementId, GpuElementData, GpuImageSink};
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
use slint::{ComponentHandle, PlatformError};
use std::cell::RefCell;
use std::sync::Arc;
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
}

impl DelaxSlintHost {
    pub fn new(params: Arc<DelaxParams>, input_data: Arc<InputData>) -> Self {
        let (event_tx, event_rx) = unbounded();
        Self {
            params,
            data: input_data,
            event_tx,
            event_rx,
        }
    }
}

impl SlintHost for DelaxSlintHost {
    type Component = slint_ui::AppWindow;

    fn build(&self) -> Result<Self::Component, PlatformError> {
        let app = slint_ui::AppWindow::new()?;
        app.set_version(env!("CARGO_PKG_VERSION").into());
        app.bind_param_changed(self.event_tx.clone(), self.params.clone());
        //app.window().set_rendering_notifier(|state, api| {
        //})
        Ok(app)
    }

    fn on_event(&self, app: &Self::Component, gui_context: &GuiContext) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::ParamChanged { id, value } => {
                    let normalized = value.clamp(0.0, 1.0);
                    for (param_id, ptr, _) in self.params.param_map().iter() {
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
                    for (param_id, ptr, _) in self.params.param_map().iter() {
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
                    for (param_id, ptr, _) in self.params.param_map().iter() {
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
        sync_params_to_ui(&self.params, app);
        self.data.update_ui(app);

        let mut registry = wgpu.borrow_mut();
        for &element in ElementId::ALL {
            let Some(spec) = element.spec() else { continue; };
            registry.register(element, spec);
            let Some(uniforms) = self.data.element_uniform(element) else { continue; };
            let (w, h) = element.default_size();
            let Some(image) = registry.render_to_image(element, w, h, &uniforms) else { continue; };
            app.set_element_image(element, image);
        }
    }

    fn on_resized(&self, width: u32, height: u32) {
        self.params.editor_state.editor_size.store((width, height));
    }
}
