use slint::{Model, ModelRc, VecModel};

use crate::delay_engine::jump_builder::{Jump, JumpSegment};
use crate::slint_ui::data_transport::{BufferChannel, DataTransportRx, UiState};
use crate::slint_ui::elements::ElementId;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::snapshot::{EditorChannel, UiVisualState};
use crate::slint_ui::{self, EditorData, HeaderData, UIJump, UIJumpSegment};

fn normalize_ratio(value: usize, active_len: usize) -> f32 {
    value as f32 / active_len.max(1) as f32
}

fn normalize_jumps(jumps: &[Jump], active_len: usize) -> Vec<UIJump> {
    jumps
        .iter()
        .map(|j| UIJump {
            from: normalize_ratio(j.from, active_len),
            to: normalize_ratio(j.to, active_len),
            order: j.rank as i32,
        })
        .collect()
}

fn normalize_segments(segments: &[JumpSegment], active_len: usize) -> Vec<UIJumpSegment> {
    segments
        .iter()
        .map(|s| UIJumpSegment {
            start: normalize_ratio(s.start, active_len),
            end: normalize_ratio(s.end, active_len),
            order: s.order as i32,
        })
        .collect()
}

pub fn poll_and_present(
    data: &UiState,
    visual: &mut UiVisualState,
    rx: &mut DataTransportRx,
    app: &slint_ui::AppWindow,
) {
    visual.poll_spectrum(rx);
    visual.poll_wave(rx);
    push_waveforms(visual, app);
    visual.poll_editor(rx, data);

    let block = data.read_block();
    app.set_header_data(HeaderData {
        in_level_l: block.meters_in.left,
        in_level_r: block.meters_in.right,
        out_level_l: block.meters_out.left,
        out_level_r: block.meters_out.right,
    });
    app.set_bpm(block.bpm);

    let mut editor = EditorData {
        write_head_l: block.heads.left.write,
        write_head_r: block.heads.right.write,
        read_head_l: block.heads.left.read,
        read_head_r: block.heads.right.read,
        read_jumps_l: app.get_editor_data().read_jumps_l,
        read_jumps_r: app.get_editor_data().read_jumps_r,
        write_jumps_l: app.get_editor_data().write_jumps_l,
        write_jumps_r: app.get_editor_data().write_jumps_r,
        read_segments_l: app.get_editor_data().read_segments_l,
        read_segments_r: app.get_editor_data().read_segments_r,
        write_segments_l: app.get_editor_data().write_segments_l,
        write_segments_r: app.get_editor_data().write_segments_r,
    };

    let version = data.jumps.version();
    if version != visual.seen_jump_version {
        visual.seen_jump_version = version;
        let lens = block.active_len;
        for ch in [BufferChannel::Left, BufferChannel::Right] {
            let len = *lens.get(ch);
            let read_jumps = data.jumps.read_jumps(ch);
            let write_jumps = data.jumps.write_jumps(ch);
            let read_segs = data.jumps.read_segments(ch);
            let write_segs = data.jumps.write_segments(ch);
            let (target_read, target_write, target_read_seg, target_write_seg) = match ch {
                BufferChannel::Left => (
                    &mut editor.read_jumps_l,
                    &mut editor.write_jumps_l,
                    &mut editor.read_segments_l,
                    &mut editor.write_segments_l,
                ),
                BufferChannel::Right => (
                    &mut editor.read_jumps_r,
                    &mut editor.write_jumps_r,
                    &mut editor.read_segments_r,
                    &mut editor.write_segments_r,
                ),
            };
            *target_read = push_model(
                std::mem::replace(target_read, ModelRc::new(VecModel::from(vec![]))),
                normalize_jumps(&read_jumps, len),
            );
            *target_write = push_model(
                std::mem::replace(target_write, ModelRc::new(VecModel::from(vec![]))),
                normalize_jumps(&write_jumps, len),
            );
            *target_read_seg = push_model(
                std::mem::replace(target_read_seg, ModelRc::new(VecModel::from(vec![]))),
                normalize_segments(&read_segs, len),
            );
            *target_write_seg = push_model(
                std::mem::replace(target_write_seg, ModelRc::new(VecModel::from(vec![]))),
                normalize_segments(&write_segs, len),
            );
        }
    }

    app.set_editor_data(editor);
}

fn push_model<T: Clone + 'static>(current: ModelRc<T>, values: Vec<T>) -> ModelRc<T> {
    match current.as_any().downcast_ref::<VecModel<T>>() {
        Some(model) => {
            model.set_vec(values);
            current
        }
        None => ModelRc::new(VecModel::from(values)),
    }
}

pub fn push_waveforms(visual: &UiVisualState, app: &slint_ui::AppWindow) {
    let (dry, wet) = visual.wave_snapshot();
    app.set_dry_buffer(push_model(app.get_dry_buffer(), dry.to_vec()));
    app.set_wet_buffer(push_model(app.get_wet_buffer(), wet.to_vec()));
}

fn wetness_from_params() -> f32 {
    crate::slint_ui::param_store()
        .read()
        .unwrap()
        .get("wetness")
        .map(|(v, _)| *v)
        .unwrap_or(0.5)
}

pub fn render_all(
    data: &UiState,
    visual: &UiVisualState,
    app: &slint_ui::AppWindow,
    registry: &mut WgpuRegistry,
) {
    let mut textures = app.get_textures();
    for &element in ElementId::ALL {
        let Some(spec) = element.spec() else { continue };
        registry.register(element, spec);
        let rendered = match element {
            ElementId::Spectrum => visual
                .spectrum_uniform()
                .as_ref()
                .map(bytemuck::bytes_of)
                .and_then(|b| registry.render_to_image(element, 100, 40, b)),
            ElementId::Buffer => {
                let wetness = wetness_from_params();
                visual
                    .buffer_uniform(wetness)
                    .as_ref()
                    .map(bytemuck::bytes_of)
                    .and_then(|b| {
                        let (w, h) = element.default_size();
                        registry.render_to_image(element, w, h, b)
                    })
            }
            ElementId::EditorBufferL => visual
                .editor_buffer_uniform(EditorChannel::Left)
                .as_ref()
                .map(bytemuck::bytes_of)
                .and_then(|b| {
                    let (w, h) = element.default_size();
                    registry.render_to_image(element, w, h, b)
                }),
            ElementId::EditorBufferR => visual
                .editor_buffer_uniform(EditorChannel::Right)
                .as_ref()
                .map(bytemuck::bytes_of)
                .and_then(|b| {
                    let (w, h) = element.default_size();
                    registry.render_to_image(element, w, h, b)
                }),
            ElementId::Decay => {
                let store = crate::slint_ui::param_store().read().unwrap();
                let is_stereo = store.get("stereo").map(|(v, _)| *v > 0.3).unwrap_or(false);
                let is_ping_pong = store.get("stereo").map(|(v, _)| *v > 0.8).unwrap_or(false);
                let bpm_bound_l = store.get("bpm_bound_l").map(|(v, _)| *v > 0.5).unwrap_or(false);
                let bpm_bound_r = store.get("bpm_bound_r").map(|(v, _)| *v > 0.5).unwrap_or(false);
                drop(store);
                data.decay_uniform_with_flags(is_stereo, is_ping_pong, bpm_bound_l, bpm_bound_r)
                    .as_ref()
                    .map(bytemuck::bytes_of)
                    .and_then(|b| {
                        let (w, h) = element.default_size();
                        registry.render_to_image(element, w, h, b)
                    })
            }
            ElementId::Peak => continue,
        };
        let Some(image) = rendered else { continue };
        match element {
            ElementId::Buffer => textures.buffer = image.into(),
            ElementId::EditorBufferL => textures.editor_buffer_l = image.into(),
            ElementId::EditorBufferR => textures.editor_buffer_r = image.into(),
            ElementId::Spectrum => textures.spectrum = image.into(),
            ElementId::Decay => textures.decay = image.into(),
            ElementId::Peak => {}
        }
    }
    app.set_textures(textures);
}
