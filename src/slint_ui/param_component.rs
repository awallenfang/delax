use nice_plug::params::Params;
use std::sync::Arc;

pub trait ParamComponent<P: Params + 'static> {
    fn bind_param_changed(
        &self,
        tx: crossbeam::channel::Sender<crate::slint_ui::editor::UiEvent>,
        params: Arc<P>,
    );

    fn set_param_from_host(&self, param_id: &str, value: f32);
}
