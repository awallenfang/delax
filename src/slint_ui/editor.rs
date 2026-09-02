use crate::slint_ui::param_component::ParamComponent;
use crate::slint_ui::window_state::WindowState;
use baseview::Window;
use baseview::dpi::PhysicalSize;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::unbounded;
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::{Editor, EditorHandle, HostMethods, ParentWindowHandle, SpawnedEditor};
use nice_plug::params::Params;
use nice_plug::params::persist::PersistentField;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;

pub const DEFAULT_WIDTH: u32 = 550;
pub const DEFAULT_HEIGHT: u32 = 350;

pub enum UiEvent {
    ParamChanged { id: String, value: f32 },
    SetDiv { div_id: String, bpm_id: String, factor: f32 },
    SetTimeMode { bpm_id: String },
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
            editor_size: AtomicCell::new((DEFAULT_WIDTH, DEFAULT_HEIGHT)),
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

pub struct UIEditor<T: slint::ComponentHandle + ParamComponent<P>, P: Params> {
    state: Arc<EditorState>,
    builder: Arc<
        dyn Fn(crossbeam::channel::Sender<UiEvent>) -> Result<T, slint::PlatformError>
            + Send
            + Sync,
    >,
    params: Arc<P>,
    frame_callback: Arc<dyn Fn(&T) + Send + Sync>,
}

impl<T: slint::ComponentHandle + ParamComponent<P>, P: Params> UIEditor<T, P> {
    pub fn new(
        editor_state: Arc<EditorState>,
        builder: Arc<
            dyn Fn(crossbeam::channel::Sender<UiEvent>) -> Result<T, slint::PlatformError>
                + Send
                + Sync,
        >,
        params: Arc<P>,
    ) -> Self {
        Self {
            state: editor_state,
            builder,
            params: params.clone(),
            frame_callback: Arc::new(|_| {}),
        }
    }

    pub fn on_frame<F>(mut self, callback: F) -> Self
    where
        F: Fn(&T) + 'static + Send + Sync,
    {
        self.frame_callback = Arc::new(callback);
        self
    }
}

pub struct UIEditorHandle {
    state: Arc<EditorState>,
}

impl EditorHandle for UIEditorHandle {
    type Window = Window;
    type Error = baseview::Error;

    fn run_until_closed(window: Self::Window) -> Result<(), Self::Error> {
        window.run_until_closed()
    }

    fn set_parent(
        &self,
        parent: ParentWindowHandle,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        window.set_parent(&parent)
    }

    fn show(&self, _window: &Self::Window) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hide(&self, _window: &Self::Window) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_size(
        &self,
        new_size: PhysicalSize<u32>,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        window.resize(new_size)
    }

    fn host_main_thread_callback(&self, window: &Self::Window) {
        window.host_main_thread_callback();
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
}

impl<T: slint::ComponentHandle + ParamComponent<P> + 'static, P: Params> Editor for UIEditor<T, P> {
    type Handle = UIEditorHandle;

    fn spawn(
        &self,
        parent: Option<ParentWindowHandle>,
        wait_for_parent: bool,
        fallback_scale_factor: Option<f64>,
        gui_context: GuiContext,
        host: Option<HostMethods>,
    ) -> Result<SpawnedEditor<Self::Handle>, Box<dyn Error>> {
        let (w, h) = self.state.size();

        let host = {
            struct HostCallbackAdapter {
                host: Box<dyn nice_plug::editor::HostCallbacks>,
            }
            impl baseview::host::HostCallbacks for HostCallbackAdapter {
                fn request_resize(
                    &mut self,
                    new_size: baseview::WindowSize,
                ) -> Result<(), baseview::HandlerError> {
                    self.host
                        .request_resize(new_size.physical.into(), new_size.scale_factor)
                        .map_err(baseview::HandlerError::from_boxed)
                }
                fn destroyed(&mut self) {
                    self.host.destroyed();
                }
            }
            struct HostMainThreadCallerAdapter {
                host: Box<dyn nice_plug::editor::HostMainThreadCaller>,
            }
            impl baseview::host::HostMainThreadCaller for HostMainThreadCallerAdapter {
                fn call_main_thread(&mut self) {
                    self.host.call_main_thread();
                }
            }
            host.map(|host| {
                baseview::host::Host::new()
                    .with_callbacks(HostCallbackAdapter {
                        host: host.callbacks,
                    })
                    .with_main_thread(HostMainThreadCallerAdapter {
                        host: host.main_thread_caller,
                    })
            })
        };

        let (event_tx, event_rx) = unbounded::<UiEvent>();
        let builder = self.builder.clone();
        let state_ref = self.state.clone();
        let params_clone = self.params.clone();
        let callback_clone = self.frame_callback.clone();

        let window = Window::create_with_host(
            baseview::WindowSettings::new()
                .with_title(self.state.title.clone())
                .with_size(PhysicalSize::new(w, h))
                .with_parent(parent.as_ref())
                .with_wait_for_parent(wait_for_parent)
                .with_fallback_scale_factor(fallback_scale_factor)
                .with_gl_config(Some(baseview::gl::GlConfig {
                    version: (3, 2),
                    ..Default::default()
                })),
            move |window_context: baseview::WindowContext| {
                Ok(WindowState::new(
                    gui_context,
                    state_ref,
                    window_context,
                    w,
                    h,
                    {
                        let builder = builder.clone();
                        move || builder(event_tx.clone())
                    },
                    event_rx,
                    params_clone.clone(),
                    callback_clone.clone(),
                )?)
            },
            host,
        )?;

        Ok(SpawnedEditor {
            handle: UIEditorHandle {
                state: self.state.clone(),
            },
            window,
        })
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.state.physical_size()
    }
}
