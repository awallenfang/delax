use std::cell::RefCell;
use slint::platform::femtovg_renderer::OpenGLInterface;
use std::error::Error;
use std::ffi::{c_void, CStr};
use std::num::NonZeroU32;
use baseview::dpi::{LogicalSize, PhysicalSize};
use baseview::WindowContext;
pub(crate) struct SlintOpenGLInterface {
    ctx: RefCell<WindowContext>
}

impl SlintOpenGLInterface {
    pub(crate) fn new(window_context: WindowContext, width: u32, height: u32) -> Result<Self, String> {
        Ok(Self {
            ctx: RefCell::new(window_context)
        })
    }
}

unsafe impl OpenGLInterface for SlintOpenGLInterface {
    fn ensure_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        unsafe {self.ctx.borrow().gl_context().unwrap().make_current().map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string())) }
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.ctx.borrow().gl_context().unwrap().swap_buffers().map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string()))
    }

    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.ctx.borrow().resize(PhysicalSize::new(width.get(), height.get())).expect("Resize failed in gl_interface");
        Ok(())
    }

    fn get_proc_address(&self, name: &CStr) -> *const c_void {
        self.ctx.borrow().gl_context().unwrap().get_proc_address(name)
    }
}

