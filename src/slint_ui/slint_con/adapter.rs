use slint::platform::femtovg_renderer::{FemtoVGRenderer, OpenGLInterface};
use slint::platform::{Renderer, WindowAdapter};
use slint::{PhysicalSize, Window, WindowSize};
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;
use baseview::WindowContext;
use crate::slint_ui::slint_con::gl_interface::SlintOpenGLInterface;

pub struct SlintAdapter {
    window: Window,
    pub(crate) renderer: OnceCell<FemtoVGRenderer>,
    size: RefCell<PhysicalSize>,
}

impl SlintAdapter {
    pub(crate) fn new(width: u32, height: u32, window_context: &WindowContext) -> Rc<Self> {
        Rc::new_cyclic(|weak_adapter| {
            let gl = SlintOpenGLInterface::new(window_context.clone(), width, height).unwrap();
            let window = Window::new(weak_adapter.clone() as _);
            let renderer = FemtoVGRenderer::new(gl).expect("Femtovg failed to initialize");
            let cell = OnceCell::new();
            let _ = cell.set(renderer);
            Self {
                window,
                renderer: cell,
                size: RefCell::new(PhysicalSize::new(width, height)),
            }
        })
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
