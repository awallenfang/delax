use crate::slint_ui::editor::{EditorState, UiEvent};
use baseview::{Event, EventStatus, HandlerError, MouseEvent, WindowEvent, WindowHandler, WindowSize};
use crossbeam::channel::Receiver;
use nice_plug::context::gui::GuiContext;
use nice_plug::params::Params;
use slint::platform::femtovg_renderer::FemtoVGRenderer;
use slint::platform::{Renderer, WindowAdapter};
use slint::{LogicalPosition, PhysicalSize, PlatformError, Window, platform};
use std::cell::{OnceCell, RefCell};
use std::error::Error;
use std::ffi::{CStr, c_void};
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

struct SlintPlatform {
    current: RefCell<Option<Rc<dyn WindowAdapter>>>,
}

impl SlintPlatform {
    fn new() -> Self {
        Self {
            current: RefCell::new(None),
        }
    }

    fn set_current(&self, adapter: Rc<dyn WindowAdapter>) {
        *self.current.borrow_mut() = Some(adapter);
    }
}

impl platform::Platform for SlintPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.current
            .borrow()
            .clone()
            .ok_or_else(|| PlatformError::Other("no current adapter set".into()))
    }
}

struct ArcPlatformWrapper(Arc<SlintPlatform>);

impl platform::Platform for ArcPlatformWrapper {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.0.create_window_adapter()
    }
}

thread_local! {
    static GLOBAL_PLATFORM: RefCell<Option<Arc<SlintPlatform>>> = RefCell::new(None);
}

#[derive(Clone)]
struct OpenGLInterface {
    get_adr: Arc<dyn Fn(&str) -> *const c_void + Send + Sync>,
}

impl Debug for OpenGLInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenGLInterface").finish()
    }
}
impl OpenGLInterface {
    fn new(ctx: &baseview::gl::GlContext) -> Self {
        let context_adr = ctx as *const baseview::gl::GlContext as usize;
        Self {
            get_adr: Arc::new(move |name: &str| {
                let context = context_adr as *const baseview::gl::GlContext;
                unsafe { &*context }.get_proc_address_from_str(name)
            }),
        }
    }
}

unsafe impl platform::femtovg_renderer::OpenGLInterface for OpenGLInterface {
    fn ensure_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    fn resize(
        &self,
        _width: NonZeroU32,
        _height: NonZeroU32,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    fn get_proc_address(&self, name: &CStr) -> *const c_void {
        (self.get_adr)(name.to_str().unwrap_or(""))
    }
}

struct SlintAdapter {
    window: Window,
    renderer: OnceCell<FemtoVGRenderer>,
    size: RefCell<PhysicalSize>,
    gl: OnceCell<OpenGLInterface>,
}

impl SlintAdapter {
    fn new(width: u32, height: u32) -> Rc<Self> {
        Rc::new_cyclic(|weak_adapter| {
            let window = Window::new(weak_adapter.clone() as _);
            Self {
                window,
                renderer: OnceCell::new(),
                size: RefCell::new(PhysicalSize::new(width, height)),
                gl: OnceCell::new(),
            }
        })
    }
    pub fn init_gl_context(&self, ctx: &baseview::gl::GlContext) {
        self.gl
            .set(OpenGLInterface::new(ctx))
            .expect("Failed initiating the GL context");
    }

    pub fn update_size(&self, width: u32, height: u32) {
        *self.size.borrow_mut() = PhysicalSize::new(width, height);
        self.window.request_redraw();
    }

    fn resize(&self, w: u32, h: u32) {
        *self.size.borrow_mut() = PhysicalSize::new(w, h);
        self.window.request_redraw();
    }
}
impl WindowAdapter for SlintAdapter {
    fn window(&self) -> &Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        *self.size.borrow()
    }

    fn renderer(&self) -> &dyn Renderer {
        self.renderer.get_or_init(|| {
            let gl_ref = self.gl.get().expect("GL context is not initialized");
            FemtoVGRenderer::new(gl_ref.clone()).expect("Femtovg failed to initialize")
        })
    }
}

pub struct WindowState<T: slint::ComponentHandle, P: Params> {
    gui_context: GuiContext,
    pub editor_state: Arc<EditorState>,
    adapter: Rc<SlintAdapter>,
    root: RefCell<T>,
    window_context: baseview::WindowContext,
    event_rx: Receiver<UiEvent>,
    params: Arc<P>,
    last_pos: RefCell<Option<LogicalPosition>>,
    frame_callback: Arc<dyn Fn(&T) + Send + Sync>,
}

impl<T: slint::ComponentHandle, P: Params> WindowState<T, P> {
    pub fn new<F>(
        gui_context: GuiContext,
        editor_state: Arc<EditorState>,
        window_context: baseview::WindowContext,
        init_width: u32,
        init_height: u32,
        builder: F,
        event_rx: Receiver<UiEvent>,
        params: Arc<P>,
        frame_callback: Arc<dyn Fn(&T) + Send + Sync>,
    ) -> Self
    where
        F: FnOnce() -> Result<T, PlatformError>,
    {
        if let Some(gl_ctx) = window_context.gl_context() {
            unsafe { gl_ctx.make_current().unwrap() };
        }

        let platform = GLOBAL_PLATFORM.with(|cell| {
            if cell.borrow().is_none() {
                let p = Arc::new(SlintPlatform::new());
                let wrapper = ArcPlatformWrapper(p.clone());
                let _ = platform::set_platform(Box::new(wrapper));
                *cell.borrow_mut() = Some(p.clone());
                p
            } else {
                cell.borrow().as_ref().unwrap().clone()
            }
        });

        let adapter = SlintAdapter::new(init_width, init_height);
        if let Some(gl_ctx) = window_context.gl_context() {
            adapter.init_gl_context(&gl_ctx);
        }
        platform.set_current(adapter.clone());

        let root = builder().unwrap_or_else(|e| panic!("Failed to build: {}", e));
        root.show().expect("Failed to show the root component");

        Self {
            gui_context,
            editor_state,
            adapter,
            root: RefCell::new(root),
            window_context,
            event_rx,
            params,
            last_pos: RefCell::new(None),
            frame_callback,
        }
    }

    pub fn window(&self) -> &Window {
        &self.adapter.window
    }
}

impl<T: slint::ComponentHandle + 'static, P: Params + 'static> WindowHandler for WindowState<T, P> {
    fn on_frame(&self) -> Result<(), HandlerError> {
        if let Some(gl_ctx) = self.window_context.gl_context() {
            unsafe { gl_ctx.make_current()? };
        }

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::ParamChanged { id, value } => {
                    let normalized = value.clamp(0.0, 1.0);
                    for (param_id, ptr, _) in self.params.param_map().iter() {
                        if param_id == &id {
                            unsafe {
                                self.gui_context.raw_begin_set_parameter(*ptr);
                                self.gui_context
                                    .raw_set_parameter_normalized(*ptr, normalized);
                                self.gui_context.raw_end_set_parameter(*ptr);
                            }
                            break;
                        }
                    }
                }
            }
        }

        {
            let root = self.root.borrow();
            (self.frame_callback)(&root);
        }

        platform::update_timers_and_animations();
        self.window().request_redraw();

        if let Some(renderer) = self.adapter.renderer.get() {
            let _ = renderer.render();
        }

        if let Some(ctx) = self.window_context.gl_context() {
            let _ = ctx.swap_buffers();
        }

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let w = new_size.physical.width;
        let h = new_size.physical.height;
        self.adapter.resize(w, h);
        self.editor_state.editor_size.store((w, h));
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Window(window_event) => match window_event {
                WindowEvent::WillClose => {
                    self.window()
                        .dispatch_event(platform::WindowEvent::CloseRequested);
                    EventStatus::Captured
                }
                _ => EventStatus::Ignored,
            },
            Event::Mouse(mouse_event) => {
                if self.last_pos.borrow().is_none() {
                    *self.last_pos.borrow_mut() = Some(LogicalPosition::new(0_f32, 0_f32))
                }
                let slint_event = match mouse_event {
                    MouseEvent::CursorMoved { position, .. } => {
                        let log_pos = LogicalPosition::new(position.x as f32, position.y as f32);
                        *self.last_pos.borrow_mut() = Some(log_pos);
                        Some(platform::WindowEvent::PointerMoved { position: log_pos })
                    }
                    MouseEvent::ButtonPressed { button, .. } => {
                        let slint_button = match button {
                            baseview::MouseButton::Left => platform::PointerEventButton::Left,
                            baseview::MouseButton::Right => platform::PointerEventButton::Right,
                            baseview::MouseButton::Middle => platform::PointerEventButton::Middle,
                            _ => return EventStatus::Ignored,
                        };
                        Some(platform::WindowEvent::PointerPressed {
                            position: self.last_pos.borrow().unwrap(),
                            button: slint_button,
                        })
                    }
                    MouseEvent::ButtonReleased { button, .. } => {
                        let slint_button = match button {
                            baseview::MouseButton::Left => platform::PointerEventButton::Left,
                            baseview::MouseButton::Right => platform::PointerEventButton::Right,
                            baseview::MouseButton::Middle => platform::PointerEventButton::Middle,
                            _ => return EventStatus::Ignored,
                        };
                        Some(platform::WindowEvent::PointerReleased {
                            position: self.last_pos.borrow().unwrap(),
                            button: slint_button,
                        })
                    }
                    MouseEvent::WheelScrolled {  .. } => return EventStatus::Ignored,
                    _ => None,
                };
                if let Some(se) = slint_event {
                    self.window().dispatch_event(se);
                    EventStatus::Captured
                } else {
                    EventStatus::Ignored
                }
            }
            Event::Keyboard(keyboard_event) => {
                let _ = keyboard_event;
                EventStatus::Ignored
            }
            _ => EventStatus::Ignored,
        }
    }
}
