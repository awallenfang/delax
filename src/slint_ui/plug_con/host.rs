use std::cell::RefCell;
use baseview::host::{Host, HostCallbacks};
use baseview::{HandlerError, WindowSize};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::{HostMainThreadCaller, HostMethods};
use crate::slint_ui::renderer::WgpuRegistry;

pub trait SlintHost: Send + Sync {
    type Component: slint::ComponentHandle + 'static;

    fn on_init(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>);
    fn build(&self) -> Result<Self::Component, slint::PlatformError>;
    fn on_event(&self, app: &Self::Component, gui_context: &GuiContext);
    fn on_frame(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>);
    fn on_resized(&self, width: u32, height: u32);
}

struct HostCallbackAdapter {
    host: Box<dyn nice_plug::editor::HostCallbacks>
}

impl HostCallbacks for HostCallbackAdapter {
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError> {
        self.host.request_resize(new_size.physical.into(), new_size.scale_factor).map_err(baseview::HandlerError::from_boxed)
    }

    fn destroyed(&mut self) {
        self.host.destroyed();
    }
}

struct HostMainThreadCallerAdapter { host: Box<dyn nice_plug::editor::HostMainThreadCaller>}
impl baseview::host::HostMainThreadCaller for HostMainThreadCallerAdapter {
    fn call_main_thread(&mut self) {
        self.host.call_main_thread();
    }
}

pub(crate) fn to_baseview_host(host: Option<HostMethods>) -> Option<Host> {
    host.map(|host| Host::new().with_callbacks(HostCallbackAdapter {host: host.callbacks}).with_main_thread(HostMainThreadCallerAdapter{host: host.main_thread_caller}))
}