use crate::slint_ui::editor::{EditorState, UiEvent};
use baseview::{Event, EventStatus, HandlerError, MouseEvent, ScrollDelta, WindowEvent, WindowHandler, WindowSize};
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

fn global_platform() -> Arc<SlintPlatform> {
    GLOBAL_PLATFORM.with(|cell| {
        if let Some(p) = cell.borrow().clone() {
            p
        } else {
            let p = Arc::new(SlintPlatform::new());
            let wrapper = ArcPlatformWrapper(p.clone());
            // `set_platform` can only succeed once per process; ignore error on subsequent windows
            let _ = platform::set_platform(Box::new(wrapper));
            *cell.borrow_mut() = Some(p.clone());
            p
        }
    })
}

#[derive(Clone)]
struct OpenGLInterface {
    ctx: baseview::gl::GlContext,
}

impl Debug for OpenGLInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenGLInterface").finish()
    }
}
impl OpenGLInterface {
    fn new(ctx: baseview::gl::GlContext) -> Self {
        Self { ctx }
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
        self.ctx.get_proc_address(name)
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
    pub fn init_gl_context(&self, ctx: &baseview::gl::GlContext) -> Result<(), PlatformError> {
        self.gl
            .set(OpenGLInterface::new(ctx.clone()))
            .map_err(|_| PlatformError::Other("GL context already initialized".into()))?;
        Ok(())
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

/// `WindowState` hosts Slint inside a `baseview` window.
/// It is `!Send` / `!Sync` by design (contains `Rc`/`RefCell`) and must stay on the window thread.
pub struct WindowState<T: slint::ComponentHandle, P: Params> {
    gui_context: GuiContext,
    pub editor_state: Arc<EditorState>,
    adapter: Rc<SlintAdapter>,
    root: RefCell<T>,
    window_context: baseview::WindowContext,
    event_rx: Receiver<UiEvent>,
    params: Arc<P>,
    last_pos: RefCell<LogicalPosition>,
    frame_callback: Arc<dyn Fn(&T) + Send + Sync>,
}

fn map_button(button: baseview::MouseButton) -> Option<platform::PointerEventButton> {
    match button {
        baseview::MouseButton::Left => Some(platform::PointerEventButton::Left),
        baseview::MouseButton::Right => Some(platform::PointerEventButton::Right),
        baseview::MouseButton::Middle => Some(platform::PointerEventButton::Middle),
        _ => None,
    }
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
    ) -> Result<Self, PlatformError>
    where
        F: FnOnce() -> Result<T, PlatformError>,
    {
        if let Some(gl_ctx) = window_context.gl_context() {
            // SAFETY: baseview GL context must be made current on this thread.
            unsafe {
                gl_ctx
                    .make_current()
                    .map_err(|e| PlatformError::Other(format!("make_current failed: {e:?}")))?
            };
        }

        let platform = global_platform();

        let adapter = SlintAdapter::new(init_width, init_height);
        if let Some(gl_ctx) = window_context.gl_context() {
            adapter.init_gl_context(&gl_ctx)?;
        }
        platform.set_current(adapter.clone());

        let root = builder().map_err(|e| {
            PlatformError::Other(format!("Failed to build Slint component: {e}"))
        })?;
        root.show().map_err(|e| {
            PlatformError::Other(format!("Failed to show Slint component: {e}"))
        })?;

        Ok(Self {
            gui_context,
            editor_state,
            adapter,
            root: RefCell::new(root),
            window_context,
            event_rx,
            params,
            last_pos: RefCell::new(LogicalPosition::new(0.0, 0.0)),
            frame_callback,
        })
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
            if let Err(e) = renderer.render() {
                eprintln!("Slint render error: {e:?}");
            }
        }

        if let Some(ctx) = self.window_context.gl_context() {
            if let Err(e) = ctx.swap_buffers() {
                eprintln!("GL swap_buffers error: {e:?}");
            }
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
                let slint_event = match mouse_event {
                    MouseEvent::CursorMoved { position, .. } => {
                        let log_pos = LogicalPosition::new(position.x as f32, position.y as f32);
                        *self.last_pos.borrow_mut() = log_pos;
                        Some(platform::WindowEvent::PointerMoved { position: log_pos })
                    }
                    MouseEvent::ButtonPressed { button, .. } => {
                        let Some(slint_button) = map_button(button) else {
                            return EventStatus::Ignored;
                        };
                        Some(platform::WindowEvent::PointerPressed {
                            position: *self.last_pos.borrow(),
                            button: slint_button,
                        })
                    }
                    MouseEvent::ButtonReleased { button, .. } => {
                        let Some(slint_button) = map_button(button) else {
                            return EventStatus::Ignored;
                        };
                        Some(platform::WindowEvent::PointerReleased {
                            position: *self.last_pos.borrow(),
                            button: slint_button,
                        })
                    }
                    MouseEvent::WheelScrolled { delta, .. } => {
                        match delta {
                            ScrollDelta::Lines { x, y } => {
                                const LINES_TO_PX: f32 = 20.0;
                                Some(platform::WindowEvent::PointerScrolled {
                                    position: *self.last_pos.borrow(),
                                    delta_x: x * LINES_TO_PX,
                                    delta_y: y * LINES_TO_PX,
                                })
                            }
                            ScrollDelta::Pixels { x, y } => {
                                Some(platform::WindowEvent::PointerScrolled {
                                    position: *self.last_pos.borrow(),
                                    delta_x: x,
                                    delta_y: y,
                                })
                            }
                        }
                    },
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
