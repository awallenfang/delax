use std::sync::atomic::Ordering::Relaxed;

use slint::Model;

use crate::slint_ui::data_transport::{DataTransportRx, InputData};
use crate::slint_ui::snapshot::{EditorChannel, UiVisualState};
use crate::slint_ui::elements::ElementId;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::{self, EditorData, HeaderData};

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
    app.set_editor_data(EditorData {
        write_head_l: data.write_head_l.load(Relaxed),
        write_head_r: data.write_head_r.load(Relaxed),
        read_head_l: data.read_head_l.load(Relaxed),
        read_head_r: data.read_head_r.load(Relaxed),
    });
}

fn push_model(current: &slint::ModelRc<f32>, values: Vec<f32>) -> bool {
    match current.as_any().downcast_ref::<slint::VecModel<f32>>() {
        Some(model) => {
            model.set_vec(values);
            true
        }
        None => false,
    }
}

pub fn push_waveforms(visual: &UiVisualState, app: &slint_ui::AppWindow) {
    let (dry, wet) = visual.wave_snapshot();
    if !push_model(&app.get_dry_buffer(), dry.to_vec()) {
        app.set_dry_buffer(slint::ModelRc::new(slint::VecModel::from(dry.to_vec())));
    }
    if !push_model(&app.get_wet_buffer(), wet.to_vec()) {
        app.set_wet_buffer(slint::ModelRc::new(slint::VecModel::from(wet.to_vec())));
    }
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
