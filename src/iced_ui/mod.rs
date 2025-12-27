use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
};

use crossbeam::atomic::AtomicCell;
use iced_baseview::{
    Application, Font, IcedBaseviewSettings, Settings, Task, Theme,
    alignment::Horizontal,
    baseview::WindowOpenOptions,
    futures::backend::default::Executor,
    widget::{Column, Text},
};
use nih_plug::{
    context,
    prelude::{AtomicF32, Editor, GuiContext, ParamPtr, ParentWindowHandle},
};
use serde::{Deserialize, Serialize};

use crate::params::DelaxParams;

#[derive(Debug)]
pub enum DelaxMessage {
    BeginEditParameter(ParamPtr),
    SetParameter(ParamPtr, f32),
    EndEditParameter(ParamPtr),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IcedState {
    #[serde(with = "nih_plug::params::persist::serialize_atomic_cell")]
    size: AtomicCell<(u32, u32)>,
    #[serde(skip)]
    open: AtomicBool,
}

impl Default for IcedState {
    fn default() -> Self {
        Self {
            size: AtomicCell::new((510, 350)),
            open: AtomicBool::new(false),
        }
    }
}

pub struct DelaxUI {
    iced_state: Arc<IcedState>,
    scale: AtomicF32,
    params: Arc<DelaxParams>, // Input states
}

impl DelaxUI {
    pub fn create(params: Arc<DelaxParams>) -> Self {
        DelaxUI {
            iced_state: Arc::new(IcedState::default()),
            scale: AtomicF32::new(1.),
            params,
        }
    }
}

impl Editor for DelaxUI {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        let (w, h) = self.iced_state.size.load();
        let window = iced_baseview::open_parented::<DelaxApplication, ParentWindowHandle>(
            &parent,
            (context, self.params.clone()),
            Settings {
                window: WindowOpenOptions {
                    title: "Delax".into(),
                    size: iced_baseview::baseview::Size {
                        width: w as f64,
                        height: h as f64,
                    }
                    .into(),
                    scale: iced_baseview::baseview::WindowScalePolicy::SystemScaleFactor,
                },
                iced_baseview: IcedBaseviewSettings {
                    ignore_non_modifier_keys: false,
                    always_redraw: true,
                },
                graphics_settings: iced_baseview::GraphicsSettings {
                    antialiasing: Some(iced_baseview::graphics::Antialiasing::MSAAx16),
                    ..Default::default()
                },
                fonts: vec![],
            },
        );

        self.iced_state.open.store(true, Relaxed);
        Box::new(WindowHandle {
            iced_state: self.iced_state.clone(),
            window,
        })
    }

    fn size(&self) -> (u32, u32) {
        self.iced_state.size.load()
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        if self.iced_state.open.load(Relaxed) {
            return false;
        }
        self.scale.store(factor, Relaxed);
        true
    }

    fn param_value_changed(&self, id: &str, normalized_value: f32) {
        ()
    }

    fn param_modulation_changed(&self, id: &str, modulation_offset: f32) {
        ()
    }

    fn param_values_changed(&self) {
        ()
    }
}

struct WindowHandle<Message: 'static + Send> {
    iced_state: Arc<IcedState>,
    window: iced_baseview::window::WindowHandle<Message>,
}

unsafe impl<Message: Send> Send for WindowHandle<Message> {}

impl<Message: Send> Drop for WindowHandle<Message> {
    fn drop(&mut self) {
        self.iced_state.open.store(false, Relaxed);
        self.window.close_window();
    }
}

struct DelaxApplication {
    iced_state: Arc<IcedState>,
    gui_context: Arc<dyn GuiContext>,
    scale: AtomicF32,

    params: Arc<DelaxParams>,
}

impl Application for DelaxApplication {
    type Message = DelaxMessage;

    type Theme = Theme;

    type Executor = Executor;

    type Flags = (Arc<dyn GuiContext>, Arc<DelaxParams>);

    fn new((gui_context, params): Self::Flags) -> (Self, iced_baseview::Task<Self::Message>) {
        (
            Self {
                gui_context,
                params,
                scale: AtomicF32::new(1.),
                iced_state: Arc::new(IcedState {
                    size: AtomicCell::new((550, 310)),
                    open: AtomicBool::new(false),
                }),
            },
            // Task to run on startup
            Task::none(),
        )
    }

    fn update(&mut self, message: Self::Message) -> iced_baseview::Task<Self::Message> {
        match message {
            DelaxMessage::BeginEditParameter(param_ptr) => unsafe {
                self.gui_context.raw_begin_set_parameter(param_ptr)
            },
            DelaxMessage::SetParameter(param_ptr, val) => unsafe {
                self.gui_context
                    .raw_set_parameter_normalized(param_ptr, val)
            },
            DelaxMessage::EndEditParameter(param_ptr) => unsafe {
                self.gui_context.raw_end_set_parameter(param_ptr)
            },
        }
        Task::none()
    }

    fn view(
        &self,
    ) -> iced_baseview::core::Element<'_, Self::Message, Self::Theme, iced_baseview::Renderer> {
        // Iced view
        Column::new()
            .align_x(Horizontal::Center)
            .push(Text::new("Test"))
            .into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }
}
