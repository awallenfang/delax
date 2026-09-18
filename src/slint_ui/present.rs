use std::sync::atomic::Ordering::Relaxed;

use slint::{Model, ModelRc, VecModel};

use crate::delay_engine::jump_builder::Jump;
use crate::slint_ui::data_transport::{DataTransportRx, InputData};
use crate::slint_ui::snapshot::{EditorChannel, UiVisualState};
use crate::slint_ui::elements::ElementId;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::{self, EditorData, HeaderData, UIJump};

fn normalize_jumps(jumps: &[Jump], active_len: usize) -> Vec<UIJump> {
    let len = active_len.max(1)  as f32;
    jumps.iter().map(|j| UIJump {
        from: j.0 as f32 / len,
        to: j.1 as f32 / len,
        order: j.2 as i32
    }).collect()
}

pub fn poll_and_present(
    data: &InputData,
    visual: &mut UiVisualState,
    rx: &mut DataTransportRx,
    app: &slint_ui::AppWindow,
) {
    visual.poll_spectrum(rx);
    visual.poll_wave(rx);
    push_waveforms(visual, app);
    visual.poll_editor(rx, data);

    app.set_header_data(HeaderData {
        in_level_l: data.in_l.load(Relaxed),
        in_level_r: data.in_r.load(Relaxed),
        out_level_l: data.out_l.load(Relaxed),
        out_level_r: data.out_r.load(Relaxed),
    });
    app.set_bpm(data.bpm.load(Relaxed));

    let mut editor = EditorData {
        write_head_l: data.write_head_l.load(Relaxed),
        write_head_r: data.write_head_r.load(Relaxed),
        read_head_l: data.read_head_l.load(Relaxed),
        read_head_r: data.read_head_r.load(Relaxed),
        read_jumps_l: app.get_editor_data().read_jumps_l,
        read_jumps_r: app.get_editor_data().read_jumps_r,
        write_jumps_l: app.get_editor_data().write_jumps_l,
        write_jumps_r: app.get_editor_data().write_jumps_r,
    };
    let version = data.jump_version.load(Relaxed);
    if version != visual.seen_jump_version {
        visual.seen_jump_version = version;
        let len_l = data.active_len_l.load(Relaxed);
        let len_r = data.active_len_r.load(Relaxed);

        if let Ok(j) = data.read_jumps_l.lock() {
            editor.read_jumps_l = push_model(editor.read_jumps_l, normalize_jumps(&j, len_l));
        }
        if let Ok(j) = data.read_jumps_r.lock() {
            editor.read_jumps_r = push_model(editor.read_jumps_r, normalize_jumps(&j, len_r));
        }
        if let Ok(j) = data.write_jumps_l.lock() {
            editor.write_jumps_l = push_model(editor.write_jumps_l, normalize_jumps(&j, len_l));
        }
        if let Ok(j) = data.write_jumps_r.lock() {
            editor.write_jumps_r = push_model(editor.write_jumps_r, normalize_jumps(&j, len_r));
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

pub fn render_all(
    data: &InputData,
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
                let wetness = data.wetness.load(Relaxed);
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
            ElementId::Decay => data
                .decay_uniform()
                .as_ref()
                .map(bytemuck::bytes_of)
                .and_then(|b| {
                    let (w, h) = element.default_size();
                    registry.render_to_image(element, w, h, b)
                }),
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
