use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use slint::{platform, PlatformError};
use slint::platform::WindowAdapter;

pub struct SlintPlatform {
    current: RefCell<Option<Rc<dyn WindowAdapter>>>,
}

impl SlintPlatform {
    fn new() -> Self {
        Self {
            current: RefCell::new(None),
        }
    }

    pub fn set_current(&self, adapter: Rc<dyn WindowAdapter>) {
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

pub fn global_platform() -> Arc<SlintPlatform> {
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