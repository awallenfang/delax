use crate::params::DelaxParams;
use crate::slint_ui;
use crate::slint_ui::data_transport::{DataTransportRx, InputData};
use crate::slint_ui::param_component::ParamComponent;
use crate::slint_ui::param_store;
use crate::slint_ui::plug_con::host::SlintHost;
use crate::slint_ui::present;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::snapshot::UiVisualState;
use baseview::dpi::PhysicalSize;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{Receiver, Sender, unbounded};
use nice_plug::context::gui::GuiContext;
use nice_plug::params::Params;
use nice_plug::params::persist::PersistentField;
use nice_plug::prelude::ParamPtr;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ApproxEq;
use slint::{Model, PlatformError, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::delay_engine::jump_builder::{Jump, JumpBuilder, JumpSegment};
use crate::slint_ui::data_transport::BufferChannel;

pub enum UiEvent {
    ParamChanged {
        id: String,
        value: f32,
    },
    SetDiv {
        div_id: String,
        bpm_id: String,
        factor: f32,
    },
    SetTimeMode {
        bpm_id: String,
    },
    SetEffectOrder {
        order: Vec<String>,
    },
    SetSegmentSwap {
        channel: i32,
        first_id: i32,
        second_id: i32,
    },
}
#[derive(Deserialize, Serialize)]
pub struct EditorState {
    #[serde(with = "nice_plug::params::persist::serialize_atomic_cell")]
    pub editor_size: AtomicCell<(u32, u32)>,
    pub title: String,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            editor_size: AtomicCell::new((550, 350)),
            title: String::from("Audio Plugin"),
        }
    }
}

impl EditorState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            editor_size: AtomicCell::new((width, height)),
            title: String::from("Audio Plugin"),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.editor_size.load()
    }

    pub fn physical_size(&self) -> PhysicalSize<u32> {
        let (w, h) = self.size();
        PhysicalSize::new(w, h)
    }
}

impl<'a> PersistentField<'a, EditorState> for Arc<EditorState> {
    fn set(&self, new_value: EditorState) {
        self.editor_size.store(new_value.editor_size.load())
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&EditorState) -> R,
    {
        f(self)
    }
}

pub struct BufferEditorState {
    pub jumps_l: Mutex<Vec<Jump>>,
    pub size_l: AtomicUsize,
    pub jumps_r: Mutex<Vec<Jump>>,
    pub size_r: AtomicUsize,
    pub version_l: AtomicU64,
    pub version_r: AtomicU64,
}

impl Default for BufferEditorState {
    fn default() -> Self {
        Self {
            jumps_l: Mutex::new(vec![]),
            size_l: AtomicUsize::new(0),
            jumps_r: Mutex::new(vec![]),
            size_r: AtomicUsize::new(0),
            version_l: Default::default(),
            version_r: Default::default(),
        }
    }
}

impl BufferEditorState {
    pub fn snapshot_jumps(&self) -> ((Vec<Jump>, usize), (Vec<Jump>, usize)) {
        let jl = self.jumps_l.lock().map(|g| g.clone()).unwrap_or_default();
        let sl = self.size_l.load(Ordering::Relaxed);
        let jr = self.jumps_r.lock().map(|g| g.clone()).unwrap_or_default();
        let sr = self.size_r.load(Ordering::Relaxed);
        ((jl, sl), (jr, sr))
    }

    pub fn snapshot_for(&self, channel: BufferChannel) -> (Vec<Jump>, usize) {
        match channel {
            BufferChannel::Left => (
                self.jumps_l.lock().map(|g| g.clone()).unwrap_or_default(),
                self.size_l.load(Ordering::Relaxed),
            ),
            BufferChannel::Right => (
                self.jumps_r.lock().map(|g| g.clone()).unwrap_or_default(),
                self.size_r.load(Ordering::Relaxed),
            ),
        }
    }

    pub fn store_jumps(&self, channel: BufferChannel, jumps: Vec<Jump>, size: usize) {
        let (slot, len, ver) = match channel {
            BufferChannel::Left => (&self.jumps_l, &self.size_l, &self.version_l),
            BufferChannel::Right => (&self.jumps_r, &self.size_r, &self.version_r),
        };
        if let Ok(mut g) = slot.lock() {
            *g = jumps;
        }
        len.store(size, Ordering::Relaxed);
        ver.fetch_add(1, Ordering::Relaxed);
    }

    pub fn builder_for(&self, channel: BufferChannel, active_len: usize) -> JumpBuilder {
        assert!(active_len > 0);
        let (jumps, size) = self.snapshot_for(channel);
        if jumps.is_empty() || size == 0 {
            return JumpBuilder::empty(active_len);
        }
        if size == active_len {
            JumpBuilder::from_jumps(active_len, jumps)
        } else {
            JumpBuilder::from_jumps(size, jumps).scaled(active_len)
        }
    }

    pub fn store_builder(&self, channel: BufferChannel, builder: JumpBuilder) {
        let jumps = builder.build();
        let size = builder.size();
        self.store_jumps(channel, jumps, size);
    }
}

impl Serialize for BufferEditorState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let ((jl, sl), (jr, sr)) = self.snapshot_jumps();
        let mut state = serializer.serialize_struct("BufferEditorState", 4)?;
        state.serialize_field("jumps_l", &jl)?;
        state.serialize_field("size_l", &sl)?;
        state.serialize_field("jumps_r", &jr)?;
        state.serialize_field("size_r", &sr)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for BufferEditorState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            jumps_l: Vec<Jump>,
            #[serde(default)]
            size_l: usize,
            #[serde(default)]
            jumps_r: Vec<Jump>,
            #[serde(default)]
            size_r: usize,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(Self {
            jumps_l: Mutex::new(raw.jumps_l),
            size_l: AtomicUsize::new(raw.size_l),
            jumps_r: Mutex::new(raw.jumps_r),
            size_r: AtomicUsize::new(raw.size_r),
            version_l: AtomicU64::new(0),
            version_r: AtomicU64::new(0),
        })
    }
}

impl<'a> PersistentField<'a, BufferEditorState> for Arc<BufferEditorState> {
    fn set(&self, new_value: BufferEditorState) {
        let ((jl, sl), (jr, sr)) = new_value.snapshot_jumps();
        if let Ok(mut g) = self.jumps_l.lock() {
            *g = jl;
        }
        self.size_l.store(sl, Ordering::Relaxed);
        if let Ok(mut g) = self.jumps_r.lock() {
            *g = jr;
        }
        self.size_r.store(sr, Ordering::Relaxed);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&BufferEditorState) -> R,
    {
        f(self)
    }
}

pub struct UiConnection {
    rx: DataTransportRx,
    visual: UiVisualState,
}

pub struct DelaxSlintHost {
    params: Arc<DelaxParams>,
    data: Arc<InputData>,
    ui: std::sync::Mutex<UiConnection>,
    event_tx: Sender<UiEvent>,
    event_rx: Receiver<UiEvent>,
    param_index: HashMap<String, ParamPtr>,
    effect_order: Arc<std::sync::RwLock<Vec<String>>>,
}

impl DelaxSlintHost {
    pub fn new(
        params: Arc<DelaxParams>,
        input_data: Arc<InputData>,
        transport_rx: DataTransportRx,
        effect_order: Arc<std::sync::RwLock<Vec<String>>>,
    ) -> Self {
        let (event_tx, event_rx) = unbounded();
        let param_index = params
            .param_map()
            .into_iter()
            .map(|(id, ptr, _)| (id, ptr))
            .collect();

        // Build param cache once on creation
        for (p_id, param_ptr, _) in params.param_map().iter() {
            let val = unsafe { param_ptr.unmodulated_normalized_value() };
            let display_val =
                unsafe { SharedString::from(param_ptr.normalized_value_to_string(val, true)) };

            param_store()
                .write()
                .unwrap()
                .insert(p_id.clone(), (val, display_val));
        }
        Self {
            params,
            data: input_data,
            ui: std::sync::Mutex::new(UiConnection {
                rx: transport_rx,
                visual: UiVisualState::default(),
            }),
            event_tx,
            event_rx,
            param_index,
            effect_order,
        }
    }

    fn sync_params_to_ui(&self, app: &slint_ui::AppWindow) {
        use slint_ui::param_component::ParamComponent;

        for (p_id, param_ptr) in self.param_index.iter() {
            let val = unsafe { param_ptr.unmodulated_normalized_value() };

            // Check the cache before doing string stuff, set_param_from_host updates the cache
            let cached = param_store().read().unwrap().get(p_id).cloned();
            let display_val = match cached {
                Some((cache_val, cache_display)) if cache_val.approx_eq(&val) => cache_display,
                _ => unsafe { SharedString::from(param_ptr.normalized_value_to_string(val, true)) },
            };
            <slint_ui::AppWindow as ParamComponent<DelaxParams>>::set_param_from_host(
                app,
                p_id,
                val,
                display_val,
            );
        }

        // TODO: Very dirty way of generating the labels. This should be done together somewhere with the params
        let count_l = self.params.delay_params.delay_note_l.value();
        let div_l = self.params.delay_params.delay_div_l.value();
        let factor_l = div_l.factor();
        let suffix_l = div_l.suffix();
        let display_l = {
            let c = (count_l * 10.0).round() / 10.0;
            if c.fract().abs() < 0.0005 {
                format!("{} {}", c as i32, suffix_l)
            } else {
                format!("{:.1} {}", c, suffix_l)
            }
        };
        app.set_timing_display_l(display_l.into());
        app.set_timing_factor_l(factor_l);
        let count_r = self.params.delay_params.delay_note_r.value();
        let div_r = self.params.delay_params.delay_div_r.value();
        let factor_r = div_r.factor();
        let suffix_r = div_r.suffix();
        let display_r = {
            let c = (count_r * 10.0).round() / 10.0;
            if c.fract().abs() < 0.0005 {
                format!("{} {}", c as i32, suffix_r)
            } else {
                format!("{:.1} {}", c, suffix_r)
            }
        };
        app.set_timing_display_r(display_r.into());
        app.set_timing_factor_r(factor_r);
    }

    fn render_vis(
        &self,
        app: &<DelaxSlintHost as SlintHost>::Component,
        wgpu: &RefCell<WgpuRegistry>,
    ) {
        let Ok(ui) = self.ui.lock() else { return };
        let mut registry = wgpu.borrow_mut();
        present::render_all(&self.data, &ui.visual, app, &mut registry);
    }
}

impl SlintHost for DelaxSlintHost {
    type Component = slint_ui::AppWindow;

    fn on_init(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>) {
        self.render_vis(app, wgpu);
    }

    fn build(&self) -> Result<Self::Component, PlatformError> {
        let app = slint_ui::AppWindow::new()?;
        app.set_version(env!("CARGO_PKG_VERSION").into());
        app.bind_param_changed(self.event_tx.clone(), self.params.clone());
        //app.window().set_rendering_notifier(|state, api| {
        //})
        Ok(app)
    }

    fn on_event(&self, app: &Self::Component, gui_context: &GuiContext) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::ParamChanged { id, value } => {
                    let normalized = value.clamp(0.0, 1.0);
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, normalized);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                            break;
                        }
                    }
                }
                UiEvent::SetDiv {
                    div_id,
                    bpm_id,
                    factor,
                } => {
                    use crate::delay_engine::params::NoteDiv;
                    let norm = NoteDiv::from_factor(factor).to_norm();
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &div_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, norm);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                        }
                        if param_id == &bpm_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, 1.0);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                        }
                    }
                }
                UiEvent::SetTimeMode { bpm_id } => {
                    for (param_id, ptr) in self.param_index.iter() {
                        if param_id == &bpm_id {
                            unsafe {
                                gui_context.raw_begin_set_parameter(*ptr);
                                gui_context.raw_set_parameter_normalized(*ptr, 0.0);
                                gui_context.raw_end_set_parameter(*ptr);
                            }
                            break;
                        }
                    }
                }
                UiEvent::SetEffectOrder { order } => {
                    if let Ok(mut shared) = self.effect_order.write() {
                        *shared = order;
                    }
                }
                UiEvent::SetSegmentSwap {
                    channel,
                    first_id,
                    second_id,
                } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = self.data.active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let builder = self.params.buffer_editor_state.builder_for(ch, len);
                    let builder = builder.swap_segments(first_id as usize, second_id as usize);
                    self.params
                        .buffer_editor_state
                        .store_builder(ch, builder);
                }
            }
        }
    }

    fn on_frame(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>) {
        self.sync_params_to_ui(app);
        let Ok(mut ui) = self.ui.lock() else { return };
        let UiConnection { rx, visual } = &mut *ui;
        present::poll_and_present(&self.data, visual, rx, app);
        present::render_all(&self.data, visual, app, &mut wgpu.borrow_mut());
    }

    fn on_resized(&self, width: u32, height: u32) {
        self.params.editor_state.editor_size.store((width, height));
    }
}

#[cfg(test)]
mod tests {
    use super::BufferEditorState;
    use crate::delay_engine::jump_builder::Jump;
    use nice_plug::params::persist::PersistentField;
    use std::sync::Arc;

    #[test]
    fn store_bumps_version_per_channel() {
        let state = BufferEditorState::default();
        state.store_jumps(
            crate::slint_ui::data_transport::BufferChannel::Left,
            vec![Jump::new(2, 3, 0), Jump::new(5, 0, 1)],
            6,
        );
        state.store_jumps(
            crate::slint_ui::data_transport::BufferChannel::Right,
            vec![Jump::new(1, 0, 0)],
            2,
        );
        let ((jl, sl), (jr, sr)) = state.snapshot_jumps();
        assert_eq!(jl, vec![Jump::new(2, 3, 0), Jump::new(5, 0, 1)]);
        assert_eq!(sl, 6);
        assert_eq!(jr, vec![Jump::new(1, 0, 0)]);
        assert_eq!(sr, 2);
        assert_eq!(
            (
                state.version_l.load(std::sync::atomic::Ordering::Relaxed),
                state.version_r.load(std::sync::atomic::Ordering::Relaxed)
            ),
            (1, 1)
        );
    }

    #[test]
    fn persistent_field_copies_without_swapping_arc() {
        let state = Arc::new(BufferEditorState::default());
        state.store_jumps(
            crate::slint_ui::data_transport::BufferChannel::Left,
            vec![Jump::new(1, 0, 0)],
            2,
        );
        let fresh = BufferEditorState {
            jumps_l: std::sync::Mutex::new(vec![Jump::new(0, 1, 0), Jump::new(1, 0, 1)]),
            size_l: std::sync::atomic::AtomicUsize::new(2),
            jumps_r: std::sync::Mutex::new(vec![]),
            size_r: std::sync::atomic::AtomicUsize::new(0),
            version_l: std::sync::atomic::AtomicU64::new(7),
            version_r: std::sync::atomic::AtomicU64::new(9),
        };
        PersistentField::set(&state, fresh);
        let ((jl, sl), _) = state.snapshot_jumps();
        assert_eq!(jl, vec![Jump::new(0, 1, 0), Jump::new(1, 0, 1)]);
        assert_eq!(sl, 2);
        assert!(Arc::strong_count(&state) == 1);
    }
}
