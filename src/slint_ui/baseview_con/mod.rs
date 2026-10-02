use crate::slint_ui::gpu_context::GpuContext;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::slint_con::adapter::SlintAdapter;
use crate::slint_ui::slint_con::platform::global_platform;
use baseview::{
    Event, EventStatus, HandlerError, MouseEvent, ScrollDelta, WindowContext, WindowEvent,
    WindowHandler, WindowSize,
};
use slint::platform::WindowAdapter;
use slint::private_unstable_api::re_exports::ApproxEq;
use slint::{PlatformError, platform};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

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
    scale_factor: Cell<f64>,
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

        let root = builder()
            .map_err(|e| PlatformError::Other(format!("Failed to build Slint component: {e}")))?;
        root.show()
            .map_err(|e| PlatformError::Other(format!("Failed to show Slint component: {e}")))?;

        {
            let scale = window_context.scale_factor();
            let phys = window_context.size().physical;
            adapter.window().dispatch_event(
                platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale as f32,
                },
            );
            adapter.window().dispatch_event(platform::WindowEvent::Resized {
                size: slint::LogicalSize::new(
                    phys.width as f32 / scale as f32,
                    phys.height as f32 / scale as f32,
                ),
            });
        }

        let gpu_context = match GpuContext::ensure_initialized() {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!(
                    "\n\
                     delax: cannot open its window — no Vulkan GPU driver is available.\n\
                     Detail: {e}\n"
                );
                std::process::exit(1);
            }
        };
        let wgpu_registry = RefCell::new(WgpuRegistry::new(gpu_context.clone()));
        on_init(&root, &wgpu_registry);

        Ok(Self {
            adapter,
            root: RefCell::new(root),
            last_pos: RefCell::new(slint::LogicalPosition::new(0., 0.)),
            wgpu_registry,
            on_event_closure: on_event.clone(),
            on_frame_closure: on_frame.clone(),
            on_resize_closure: on_resize.clone(),
            scale_factor: Cell::new(window_context.scale_factor())
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
        let phys_w = new_size.physical.width;
        let phys_h = new_size.physical.height;
        let scale = new_size.scale_factor;
        let old_scale = self.scale_factor.get();
        self.scale_factor.set(scale);
        // Host/editor state tracks logical pixels; SlintPhysical size goes to the adapter.
        let logical_w = (phys_w as f64 / scale).round() as u32;
        let logical_h = (phys_h as f64 / scale).round() as u32;
        let root = self.root.borrow();
        (self.on_resize_closure)(&root, &self.wgpu_registry, logical_w, logical_h);
        self.adapter.resize(phys_w, phys_h);
        if (scale - old_scale).abs() > f64::EPSILON {
            self.adapter.window().dispatch_event(
                platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale as f32,
                },
            );
        }
        self.adapter.window().dispatch_event(platform::WindowEvent::Resized {
            size: slint::LogicalSize::new(logical_w as f32, logical_h as f32),
        });
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Window(window_event) => match window_event {
                WindowEvent::WillClose => {
                    self.adapter
                        .window()
                        .dispatch_event(platform::WindowEvent::CloseRequested);
                    EventStatus::Captured
                }
                _ => EventStatus::Ignored,
            },
            Event::Mouse(mouse_event) => {
                let slint_event = match mouse_event {
                    MouseEvent::CursorMoved { position, .. } => {
                        let scale = self.scale_factor.get();
                        let log_pos = slint::LogicalPosition::new(
                            (position.x / scale) as f32,
                            (position.y / scale) as f32,
                        );
                        if self.last_pos.borrow().x.approx_eq(&log_pos.x)
                            && self.last_pos.borrow().y.approx_eq(&log_pos.y)
                        {
                            None
                        } else {
                            *self.last_pos.borrow_mut() = log_pos;
                            Some(platform::WindowEvent::PointerMoved { position: log_pos })
                        }
                    }
                    MouseEvent::ButtonPressed {
                        button,
                        modifiers: _,
                    } => {
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
                            let scale = self.scale_factor.get() as f32;
                            Some(platform::WindowEvent::PointerScrolled {
                                position: *self.last_pos.borrow(),
                                delta_x: x / scale,
                                delta_y: y / scale,
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
