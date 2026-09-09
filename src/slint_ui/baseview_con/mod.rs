use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use baseview::{Event, EventStatus, HandlerError, MouseEvent, ScrollDelta, WindowContext, WindowEvent, WindowHandler, WindowSize};
use slint::{platform, PlatformError};
use slint::platform::WindowAdapter;
use crate::slint_ui::gpu_context::GpuContext;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::slint_con::adapter::SlintAdapter;
use crate::slint_ui::slint_con::platform::global_platform;
use slint::private_unstable_api::re_exports::ApproxEq;

pub(crate) fn map_button(button: baseview::MouseButton) -> Option<platform::PointerEventButton> {
    match button {
        baseview::MouseButton::Left => Some(platform::PointerEventButton::Left),
        baseview::MouseButton::Right => Some(platform::PointerEventButton::Right),
        baseview::MouseButton::Middle => Some(platform::PointerEventButton::Middle),
        _ => None,
    }
}
pub struct BaseviewWindow<T: slint::ComponentHandle> {
    adapter: Rc<SlintAdapter>,
    root: RefCell<T>,
    last_pos: RefCell<slint::LogicalPosition>,
    wgpu_registry: RefCell<WgpuRegistry>,
    on_event_closure: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>) + Send + Sync>,
    on_frame_closure: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>) + Send + Sync>,
    on_resize_closure: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>, u32, u32) + Send + Sync>,
}

impl<T: slint::ComponentHandle + 'static> BaseviewWindow<T> {
    pub fn new<F>(
        window_context: WindowContext,
        init_width: u32,
        init_height: u32,
        builder: F,
        on_event: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>) + Send + Sync>,
        on_frame: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>) + Send + Sync>,
        on_resize: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>, u32, u32) + Send + Sync>,
        on_init: Arc<dyn Fn(&T, &RefCell<WgpuRegistry>) + Send + Sync>,
    ) -> Result<Self, PlatformError>
    where
        F: FnOnce() -> Result<T, PlatformError>,
    {
        let adapter = SlintAdapter::new(init_width, init_height, &window_context);
        let platform = global_platform();

        platform.set_current(adapter.clone());

        let root = builder().map_err(|e| PlatformError::Other(format!("Failed to build Slint component: {e}")))?;
        root.show().map_err(|e| PlatformError::Other(format!("Failed to show Slint component: {e}")))?;

        let gpu_context = GpuContext::ensure_initialized()?;
        let wgpu_registry = RefCell::new(WgpuRegistry::new(gpu_context.clone()));
        on_init(&root, &wgpu_registry);

        Ok(Self {
            adapter,
            root: RefCell::new(root),
            last_pos: RefCell::new(slint::LogicalPosition::new(0.,0.)),
            wgpu_registry,
            on_event_closure: on_event.clone(),
            on_frame_closure: on_frame.clone(),
            on_resize_closure: on_resize.clone(),
        })
    }
}
impl<T: slint::ComponentHandle + 'static> WindowHandler for BaseviewWindow<T> {
    fn on_frame(&self) -> Result<(), HandlerError> {
        {
            let root = self.root.borrow();
            (self.on_event_closure)(&root, &self.wgpu_registry);
        }
        {
            let root = self.root.borrow();
            (self.on_frame_closure)(&root, &self.wgpu_registry);
        }
        platform::update_timers_and_animations();
        self.adapter.window().request_redraw();

        if let Some(renderer) = self.adapter.renderer.get() {
            if let Err(e) = renderer.render() {
                eprintln!("Slint render error: {e:?}");
            }
        }

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let w = new_size.physical.width;
        let h = new_size.physical.height;
        let root = self.root.borrow();
        (self.on_resize_closure)(&root, &self.wgpu_registry, w, h);
        self.adapter.resize(w,h);
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Window(window_event) => match window_event {
                WindowEvent::WillClose => {
                    self.adapter.window()
                        .dispatch_event(platform::WindowEvent::CloseRequested);
                    EventStatus::Captured
                }
                _ => EventStatus::Ignored,
            },
            Event::Mouse(mouse_event) => {
                let slint_event = match mouse_event {
                    MouseEvent::CursorMoved { position, .. } => {
                        let log_pos = slint::LogicalPosition::new(position.x as f32, position.y as f32);
                        if self.last_pos.borrow().x.approx_eq(&log_pos.x) &&  self.last_pos.borrow().y.approx_eq(&log_pos.y) {
                            None
                        } else {
                            *self.last_pos.borrow_mut() = log_pos;
                            Some(platform::WindowEvent::PointerMoved { position: log_pos })
                        }
                    }
                    MouseEvent::ButtonPressed { button, modifiers } => {
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
                    MouseEvent::WheelScrolled { delta, .. } => match delta {
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
                    },
                    _ => None,
                };
                if let Some(se) = slint_event {
                    self.adapter.window().dispatch_event(se);
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