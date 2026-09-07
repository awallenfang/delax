use crate::slint_ui::baseview_con::BaseviewWindow;
use crate::slint_ui::plug_con::host::{SlintHost, to_baseview_host};
use crate::slint_ui::renderer::WgpuRegistry;
use baseview::Window;
use baseview::dpi::PhysicalSize;
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::{
    Editor, EditorHandle, HostMethods, Modifiers, ParentWindowHandle, ResizeHint, SpawnedEditor,
    VirtualKeyCode,
};
use std::cell::RefCell;
use std::error::Error;
use std::sync::Arc;
use baseview::gl::GlConfig;

pub struct SlintEditor<H: SlintHost> {
    host: Arc<H>,
    size: PhysicalSize<u32>,
    title: String,
}

impl<H: SlintHost> SlintEditor<H> {
    pub fn new(host: Arc<H>, size: PhysicalSize<u32>, title: String) -> Self {
        Self {
            host,
            size,
            title,
        }
    }
}

impl<H: SlintHost + 'static> Editor
    for SlintEditor<H>
{
    type Handle = SlintEditorHandle;

    fn spawn(
        &self,
        parent: Option<ParentWindowHandle>,
        wait_for_parent: bool,
        fallback_scale_factor: Option<f64>,
        gui_context: GuiContext,
        host: Option<HostMethods>,
    ) -> Result<SpawnedEditor<Self::Handle>, Box<dyn Error>> {
        let (width, height) = (self.size.width, self.size.height);
        let builder = {
            let h = self.host.clone();
            move || h.build()
        };
        let on_event: Arc<dyn Fn(&<H as SlintHost>::Component, &RefCell<WgpuRegistry>) + Send + Sync> = Arc::new(
            {
                let host = self.host.clone();
                let gui_context = gui_context.clone();
                move |app, _wgpu| host.on_event(app, &gui_context)
            }
        );
        let on_frame: Arc<dyn Fn(&<H as SlintHost>::Component, &RefCell<WgpuRegistry>) + Send + Sync> = Arc::new(
            {
                let host = self.host.clone();
                move |app, wgpu| host.on_frame(app, wgpu)
            }
        );
        let on_resize: Arc<dyn Fn(&<H as SlintHost>::Component, &RefCell<WgpuRegistry>, u32, u32) + Send + Sync> = Arc::new(
            {
                let host = self.host.clone();
                move |_app, _wgpu, w, h| host.on_resized(w,h)
            }
        );

        let window = Window::create_with_host(
            baseview::WindowSettings::new()
                .with_title(self.title.clone())
                .with_size(self.size)
                .with_parent(parent.as_ref())
                .with_fallback_scale_factor(fallback_scale_factor)
                .with_wait_for_parent(wait_for_parent)
                .with_gl_config(Some(GlConfig {
                    version: (3,2),
                        ..Default::default()
                })),
            move |window_context| {
                Ok(BaseviewWindow::new(
                    window_context,
                    width,
                    height,
                    builder,
                    on_event,
                    on_frame,
                    on_resize,
                )?)
            },
            to_baseview_host(host),
        )?;

        Ok(SpawnedEditor {
            handle: SlintEditorHandle,
            window,
        })
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    fn resize_hint(&self) -> ResizeHint {
        ResizeHint::NON_RESIZABLE
    }
}

pub struct SlintEditorHandle;

impl EditorHandle for SlintEditorHandle {
    type Window = Window;
    type Error = baseview::Error;

    fn run_until_closed(window: Self::Window) -> Result<(), Self::Error> {
        window.run_until_closed()
    }

    fn set_parent(
        &self,
        parent: ParentWindowHandle,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        window.set_parent(&parent)
    }

    fn show(&self, window: &Self::Window) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hide(&self, window: &Self::Window) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_size(
        &self,
        new_size: PhysicalSize<u32>,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        window.resize(new_size)
    }

    fn host_main_thread_callback(&self, window: &Self::Window) {
        window.host_main_thread_callback()
    }

    fn adjust_size(
        &self,
        new_size: PhysicalSize<u32>,
        window: &Self::Window,
    ) -> Option<PhysicalSize<u32>> {
        todo!()
    }

    fn on_virtual_key_from_host(
        &self,
        key_code: VirtualKeyCode,
        is_down: bool,
        modifiers: Modifiers,
    ) -> bool {
        false
    }

    fn state_changed(&self) {
        ()
    }

    fn param_value_changed(&self, id: &str, normalized_value: f32) {
        ()
    }

    fn param_modulation_changed(&self, id: &str, modulation_offset: f32) {
        ()
    }
}
