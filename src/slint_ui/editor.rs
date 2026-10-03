use crate::params::DelaxParams;
use crate::slint_ui;
use crate::slint_ui::bind;
use crate::slint_ui::derived::{JumpCursor, SpectrumState};
use crate::slint_ui::frames::{BufferChannel, FrameReads};
use crate::slint_ui::render;
use crate::slint_ui::transport::active_len_for;
use crate::slint_ui::param_component::ParamComponent;
use crate::slint_ui::param_store;
use crate::slint_ui::plug_con::host::SlintHost;
use crate::slint_ui::renderer::WgpuRegistry;
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
use slint::{PlatformError, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::delay_engine::jump_builder::{Jump, JumpBuilder, SegmentEditor};

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
    MoveSegmentBoundary {
        channel: i32,
        boundary: i32,
        pos: i32,
    },
    UngluePortal {
        channel: i32,
        boundary: i32,
    },
    MovePortalExit {
        channel: i32,
        boundary: i32,
        pos: i32,
    },
    MovePortalEntry {
        channel: i32,
        boundary: i32,
        pos: i32,
    },
    ReweldPortal {
        channel: i32,
        boundary: i32,
    },
    PresetSplit {
        channel: i32,
        splits: i32,
    },
    SplitSegment {
        channel: i32,
        segment: i32,
    },
    MergeSegments {
        channel: i32,
        boundary: i32,
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

#[derive(Clone, Debug, Default)]
pub struct DerivedJumps {
    pub version: u64,
    pub table: Vec<Jump>,
    pub cycle: Vec<usize>,
}

pub struct BufferEditorState {
    pub version_l: AtomicU64,
    pub version_r: AtomicU64,
    pub editor_l: Mutex<SegmentEditor>,
    pub editor_r: Mutex<SegmentEditor>,
    pub derived_l: Mutex<Option<DerivedJumps>>,
    pub derived_r: Mutex<Option<DerivedJumps>>,
}

impl Default for BufferEditorState {
    fn default() -> Self {
        let init = SegmentEditor::split_evenly(8, 8);
        Self {
            version_l: Default::default(),
            version_r: Default::default(),
            editor_l: Mutex::new(init.clone()),
            editor_r: Mutex::new(init),
            derived_l: Default::default(),
            derived_r: Default::default(),
        }
    }
}

impl BufferEditorState {
    fn fallback_editor() -> SegmentEditor {
        SegmentEditor::split_evenly(8, 8)
    }

    fn editor_snapshot(&self, channel: BufferChannel) -> (SegmentEditor, u64) {
        match channel {
            BufferChannel::Left => (
                self.editor_l
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_else(|_| Self::fallback_editor()),
                self.version_l.load(Ordering::Relaxed),
            ),
            BufferChannel::Right => (
                self.editor_r
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_else(|_| Self::fallback_editor()),
                self.version_r.load(Ordering::Relaxed),
            ),
        }
    }

    pub fn snapshot_jumps(&self) -> ((Vec<Jump>, usize), (Vec<Jump>, usize)) {
        let (el, _) = self.editor_snapshot(BufferChannel::Left);
        let (er, _) = self.editor_snapshot(BufferChannel::Right);
        ((el.build_jumps(), el.size()), (er.build_jumps(), er.size()))
    }

    pub fn snapshot_for(&self, channel: BufferChannel) -> (Vec<Jump>, usize) {
        let (editor, _) = self.editor_snapshot(channel);
        (editor.build_jumps(), editor.size())
    }

    pub fn editor_for(&self, channel: BufferChannel, active_len: usize) -> SegmentEditor {
        let (editor, _) = self.editor_snapshot(channel);
        if editor.size() == active_len {
            editor
        } else {
            editor.scaled(active_len)
        }
    }

    pub fn store_editor(&self, channel: BufferChannel, editor: SegmentEditor) -> bool {
        if !editor.validate_cycle() {
            return false;
        }
        let (slot, derived, ver) = match channel {
            BufferChannel::Left => (&self.editor_l, &self.derived_l, &self.version_l),
            BufferChannel::Right => (&self.editor_r, &self.derived_r, &self.version_r),
        };
        let version = ver.load(Ordering::Relaxed) + 1;
        if let Ok(mut g) = derived.lock() {
            *g = Some(DerivedJumps {
                version,
                table: editor.build_jumps(),
                cycle: editor.visit_cycle(),
            });
        }
        if let Ok(mut g) = slot.lock() {
            *g = editor;
        }
        ver.store(version, Ordering::Relaxed);
        true
    }

    pub fn derived_for(&self, channel: BufferChannel) -> Option<DerivedJumps> {
        let (derived, ver) = match channel {
            BufferChannel::Left => (&self.derived_l, &self.version_l),
            BufferChannel::Right => (&self.derived_r, &self.version_r),
        };
        let current = ver.load(Ordering::Relaxed);
        let g = derived.lock().ok()?;
        if g.as_ref()?.version != current {
            return None;
        }
        g.clone()
    }

    pub fn builder_for(&self, channel: BufferChannel, active_len: usize) -> JumpBuilder {
        assert!(active_len > 0);
        let editor = self.editor_for(channel, active_len);
        JumpBuilder::from_jumps(active_len, &editor.build_jumps())
    }
}

impl Serialize for BufferEditorState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let (el, _) = self.editor_snapshot(BufferChannel::Left);
        let (er, _) = self.editor_snapshot(BufferChannel::Right);
        let mut state = serializer.serialize_struct("BufferEditorState", 2)?;
        state.serialize_field("editor_l", &el)?;
        state.serialize_field("editor_r", &er)?;
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
            editor_l: Option<SegmentEditor>,
            #[serde(default)]
            editor_r: Option<SegmentEditor>,
            #[serde(default)]
            jumps_l: Vec<Jump>,
            #[serde(default)]
            size_l: usize,
            #[serde(default)]
            jumps_r: Vec<Jump>,
            #[serde(default)]
            size_r: usize,
        }

        fn legacy(size: usize, jumps: &[Jump]) -> SegmentEditor {
            if size == 0 || jumps.is_empty() {
                return SegmentEditor::split_evenly(8, 8);
            }
            SegmentEditor::from_jumps(size, jumps)
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(Self {
            version_l: AtomicU64::new(0),
            version_r: AtomicU64::new(0),
            editor_l: Mutex::new(
                raw.editor_l
                    .unwrap_or_else(|| legacy(raw.size_l, &raw.jumps_l)),
            ),
            editor_r: Mutex::new(
                raw.editor_r
                    .unwrap_or_else(|| legacy(raw.size_r, &raw.jumps_r)),
            ),
            derived_l: Default::default(),
            derived_r: Default::default(),
        })
    }
}

impl<'a> PersistentField<'a, BufferEditorState> for Arc<BufferEditorState> {
    fn set(&self, new_value: BufferEditorState) {
        let (el, _) = new_value.editor_snapshot(BufferChannel::Left);
        let (er, _) = new_value.editor_snapshot(BufferChannel::Right);
        self.store_editor(BufferChannel::Left, el);
        self.store_editor(BufferChannel::Right, er);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&BufferEditorState) -> R,
    {
        f(self)
    }
}

pub struct UiConnection {
    spectrum: SpectrumState,
    jumps: JumpCursor,
}

pub struct DelaxSlintHost {
    params: Arc<DelaxParams>,
    ui: std::sync::Mutex<UiConnection>,
    event_tx: Sender<UiEvent>,
    event_rx: Receiver<UiEvent>,
    param_index: HashMap<String, ParamPtr>,
    effect_order: Arc<std::sync::RwLock<Vec<String>>>,
}

impl DelaxSlintHost {
    pub fn new(
        params: Arc<DelaxParams>,
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
            ui: std::sync::Mutex::new(UiConnection {
                spectrum: SpectrumState::default(),
                jumps: JumpCursor::default(),
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
        let Ok(mut ui) = self.ui.lock() else { return };
        let frames = FrameReads::read();
        let UiConnection { spectrum, jumps } = &mut *ui;
        bind::sync_slint(app, &frames, &mut *spectrum, &mut *jumps);
        let mut registry = wgpu.borrow_mut();
        render::render_textures(app, &mut registry, &frames, spectrum);
    }

    /// Update segment editor scales if it was changed
    fn poll_scaled_editors(&self) {
        for ch in [BufferChannel::Left, BufferChannel::Right] {
            let len = active_len_for(ch);
            if len == 0 {
                continue;
            }
            let state = &self.params.buffer_editor_state;
            let (_, stored_size) = state.snapshot_for(ch);
            if stored_size != len {
                let scaled = state.editor_for(ch, len);
                state.store_editor(ch, scaled);
            }
        }
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

    fn on_event(&self, _app: &Self::Component, gui_context: &GuiContext) {
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
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.swap_segments(first_id as usize, second_id as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::MoveSegmentBoundary {
                    channel,
                    boundary,
                    pos,
                } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.move_boundary(boundary.max(0) as usize, pos.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::UngluePortal { channel, boundary } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.unglue(boundary.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::MovePortalExit {
                    channel,
                    boundary,
                    pos,
                } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.move_exit(boundary.max(0) as usize, pos.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::MovePortalEntry {
                    channel,
                    boundary,
                    pos,
                } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.move_entry(boundary.max(0) as usize, pos.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::ReweldPortal { channel, boundary } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.reweld(boundary.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::PresetSplit { channel, splits } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.preset_split(splits.max(1) as u32);
                    state.store_editor(ch, editor);
                }
                UiEvent::SplitSegment { channel, segment } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.split_segment(segment.max(0) as usize);
                    state.store_editor(ch, editor);
                }
                UiEvent::MergeSegments { channel, boundary } => {
                    let ch = BufferChannel::from_i32(channel);
                    let len = active_len_for(ch);
                    if len == 0 {
                        continue;
                    }
                    let state = &self.params.buffer_editor_state;
                    let mut editor = state.editor_for(ch, len);
                    editor.merge_segments(boundary.max(0) as usize);
                    state.store_editor(ch, editor);
                }
            }
        }
        self.poll_scaled_editors();
    }

    fn on_frame(&self, app: &Self::Component, wgpu: &RefCell<WgpuRegistry>) {
        self.sync_params_to_ui(app);
        let Ok(mut ui) = self.ui.lock() else { return };
        let UiConnection { spectrum, jumps } = &mut *ui;
        let frames = FrameReads::read();
        bind::sync_slint(app, &frames, &mut *spectrum, &mut *jumps);
        render::render_textures(app, &mut wgpu.borrow_mut(), &frames, spectrum);
    }

    fn on_resized(&self, width: u32, height: u32) {
        self.params.editor_state.editor_size.store((width, height));
    }
}

#[cfg(test)]
mod tests {
    use super::BufferEditorState;
    use nice_plug::params::persist::PersistentField;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::delay_engine::jump_builder::{Jump, Portal, SegmentEditor};
    use crate::slint_ui::frames::BufferChannel;

    fn version(state: &BufferEditorState, ch: BufferChannel) -> u64 {
        match ch {
            BufferChannel::Left => state.version_l.load(Ordering::Relaxed),
            BufferChannel::Right => state.version_r.load(Ordering::Relaxed),
        }
    }

    fn editor(state: &BufferEditorState, ch: BufferChannel) -> SegmentEditor {
        match ch {
            BufferChannel::Left => state.editor_l.lock().unwrap().clone(),
            BufferChannel::Right => state.editor_r.lock().unwrap().clone(),
        }
    }

    #[test]
    fn store_bumps_version_per_channel() {
        let state = BufferEditorState::default();
        let left = SegmentEditor::split_evenly(6, 3);
        let right = SegmentEditor::single(2);
        assert!(state.store_editor(BufferChannel::Left, left.clone()));
        assert!(state.store_editor(BufferChannel::Right, right.clone()));

        let ((jl, sl), (jr, sr)) = state.snapshot_jumps();
        assert_eq!(jl, left.build_jumps());
        assert_eq!(sl, 6);
        assert_eq!(jr, right.build_jumps());
        assert_eq!(sr, 2);
        assert_eq!(
            (
                version(&state, BufferChannel::Left),
                version(&state, BufferChannel::Right)
            ),
            (1, 1)
        );
    }

    #[test]
    fn store_keeps_portals_that_a_jump_round_trip_would_drop() {
        let state = BufferEditorState::default();
        let mut e = SegmentEditor::split_evenly(12, 3);
        e.unglue(1);
        e.move_exit(1, 1);
        assert!(state.store_editor(BufferChannel::Left, e.clone()));

        let stored = editor(&state, BufferChannel::Left);
        assert_eq!(stored.portals()[1], e.portals()[1]);
        assert!(stored.is_unglued(1), "the portal survives the store");
    }

    #[test]
    fn a_buffer_length_change_rescales_the_layout_instead_of_replacing_it() {
        let state = BufferEditorState::default();
        let mut e = SegmentEditor::split_evenly(12, 3);
        e.swap_segments(0, 2);
        state.store_editor(BufferChannel::Left, e.clone());

        let grown = state.editor_for(BufferChannel::Left, 24);
        assert_eq!(grown.size(), 24);
        assert_eq!(
            grown.starts(),
            &[0, 8, 16],
            "the three segments must survive the resize, not be replaced"
        );
        assert_eq!(grown.order(), &[0, 2, 1], "and so must their order");

        let shrunk = state.editor_for(BufferChannel::Left, 2);
        assert_eq!(shrunk.size(), 2);
        assert!(shrunk.validate_cycle());
    }

    #[test]
    fn store_refuses_a_table_that_fails_validation() {
        let state = BufferEditorState::default();
        let before = editor(&state, BufferChannel::Left);
        let mut e = SegmentEditor::split_evenly(12, 4);
        e.swap_segments(0, 2);
        e.portals[1] = Some(Portal {
            exit: 11,
            entry: 11,
        });
        assert!(!e.validate_cycle(), "fixture must be invalid");
        assert!(!state.store_editor(BufferChannel::Left, e));
        assert_eq!(version(&state, BufferChannel::Left), 0);
        assert_eq!(editor(&state, BufferChannel::Left), before);
    }

    #[test]
    fn derived_cache_tracks_the_stored_version() {
        let state = BufferEditorState::default();
        assert!(
            state.derived_for(BufferChannel::Left).is_none(),
            "nothing derived yet, so the cache cannot be current"
        );
        let mut e = SegmentEditor::split_evenly(12, 3);
        e.unglue(1);
        e.move_exit(1, 1);
        state.store_editor(BufferChannel::Left, e.clone());

        let d = state.derived_for(BufferChannel::Left).expect("current");
        assert_eq!(d.version, version(&state, BufferChannel::Left));
        assert_eq!(d.table, e.build_jumps());
        assert_eq!(d.cycle, e.visit_cycle());
    }

    #[test]
    fn round_trips_welded_and_unglued_through_serde() {
        for portals in [false, true] {
            let mut e = SegmentEditor::split_evenly(12, 3);
            if portals {
                e.unglue(1);
                e.move_exit(1, 1);
            }
            let state = BufferEditorState::default();
            state.store_editor(BufferChannel::Left, e.clone());
            state.store_editor(BufferChannel::Right, e.clone());

            let json = serde_json::to_string(&state).unwrap();
            let back: BufferEditorState = serde_json::from_str(&json).unwrap();
            assert_eq!(
                editor(&back, BufferChannel::Left),
                e,
                "left, portals={portals}"
            );
            assert_eq!(
                editor(&back, BufferChannel::Right),
                e,
                "right, portals={portals}"
            );
        }
    }

    #[test]
    fn legacy_jump_shape_still_loads() {
        let json = r#"{"jumps_l":[{"from":2,"to":3,"rank":0},{"from":5,"to":0,"rank":1}],"size_l":6,"jumps_r":[],"size_r":0}"#;
        let back: BufferEditorState = serde_json::from_str(json).unwrap();
        let ((jl, sl), (jr, sr)) = back.snapshot_jumps();
        assert_eq!(jl, vec![Jump::new(2, 3, 0), Jump::new(5, 0, 1)]);
        assert_eq!(sl, 6);
        // size_r == 0 historically meant "unset" and resolved to an 8-way split.
        assert_eq!(jr, SegmentEditor::split_evenly(8, 8).build_jumps());
        assert_eq!(sr, 8);
    }

    #[test]
    fn persistent_field_copies_without_swapping_arc() {
        let state = Arc::new(BufferEditorState::default());
        state.store_editor(BufferChannel::Left, SegmentEditor::single(2));
        let fresh = BufferEditorState {
            version_l: Default::default(),
            version_r: Default::default(),
            editor_l: std::sync::Mutex::new(SegmentEditor::split_evenly(2, 2)),
            editor_r: std::sync::Mutex::new(SegmentEditor::split_evenly(2, 2)),
            derived_l: Default::default(),
            derived_r: Default::default(),
        };
        PersistentField::set(&state, fresh);
        assert_eq!(
            editor(&state, BufferChannel::Left),
            SegmentEditor::split_evenly(2, 2)
        );
        assert!(Arc::strong_count(&state) == 1);
    }

    #[test]
    fn persistent_field_bumps_versions_so_a_load_reaches_the_audio_thread() {
        let state = Arc::new(BufferEditorState::default());
        let before = version(&state, BufferChannel::Left);
        let fresh = BufferEditorState {
            version_l: Default::default(),
            version_r: Default::default(),
            editor_l: std::sync::Mutex::new(SegmentEditor::split_evenly(9, 3)),
            editor_r: std::sync::Mutex::new(SegmentEditor::split_evenly(9, 3)),
            derived_l: Default::default(),
            derived_r: Default::default(),
        };
        PersistentField::set(&state, fresh);
        assert!(
            version(&state, BufferChannel::Left) > before,
            "a load must change the version or the audio thread ignores it"
        );
        assert_eq!(editor(&state, BufferChannel::Left).size(), 9);
    }
}
