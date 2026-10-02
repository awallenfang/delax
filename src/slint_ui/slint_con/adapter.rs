use crate::slint_ui::slint_con::gl_interface::SlintOpenGLInterface;
use baseview::WindowContext;
use slint::platform::femtovg_renderer::FemtoVGRenderer;
use slint::platform::{Renderer, WindowAdapter};
use slint::{PhysicalSize, Window};
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

pub struct SlintAdapter {
    window: Window,
    pub(crate) renderer: OnceCell<FemtoVGRenderer>,
    size: RefCell<PhysicalSize>,
}

impl SlintAdapter {
    pub(crate) fn new(width: u32, height: u32, window_context: &WindowContext) -> Rc<Self> {
        let scale = window_context.scale_factor();
        let phys = window_context.size().physical;
        let (init_w, init_h) = if phys.width > 0 && phys.height > 0 {
            (phys.width, phys.height)
        } else {
            (
                (width as f64 * scale).round() as u32,
                (height as f64 * scale).round() as u32,
            )
        };
        Rc::new_cyclic(|weak_adapter| {
            let gl = SlintOpenGLInterface::new(window_context.clone(), width, height).unwrap();
            let window = Window::new(weak_adapter.clone() as _);
            let renderer = FemtoVGRenderer::new(gl).expect("Femtovg failed to initialize");
            let cell = OnceCell::new();
            let _ = cell.set(renderer);
            Self {
                window,
                renderer: cell,
                size: RefCell::new(PhysicalSize::new(init_w, init_h)),
            }
        })
    }
}

impl SlintAdapter {
    pub(crate) fn resize(&self, width: u32, height: u32) {
        *self.size.borrow_mut() = PhysicalSize::new(width, height);
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
        self.renderer.get().expect("renderer not initialized")
    }
}
