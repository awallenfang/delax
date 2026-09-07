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
        let this = Self {
            ctx: RefCell::new(window_context)
        };
        unsafe {
            this.ctx.borrow().gl_context().ok_or_else(|| "Failed to initialize window context")?.make_current().map_err(|e| e.to_string())?;
        }
        Ok(this)
    }
}

unsafe impl OpenGLInterface for SlintOpenGLInterface {
    fn ensure_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        unsafe {self.ctx.borrow().gl_context().expect("GL context missing despite with_gl_config").make_current().map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string())) }
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.ctx.borrow().gl_context().expect("GL context missing despite with_gl_config").swap_buffers().map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string()))
    }

    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Result<(), Box<dyn Error + Send + Sync>> {
        // self.ctx.borrow().resize(PhysicalSize::new(width.get(), height.get())).expect("Resize failed in gl_interface");
        Ok(())
    }

    fn get_proc_address(&self, name: &CStr) -> *const c_void {
        self.ctx.borrow().gl_context().expect("GL context missing despite with_gl_config").get_proc_address(name)
    }
}

