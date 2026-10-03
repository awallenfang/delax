use crate::slint_ui::derived::SpectrumState;
use crate::slint_ui::elements::{EditorChannel, ElementId};
use crate::slint_ui::frames::FrameReads;
use crate::slint_ui::renderer::WgpuRegistry;
use crate::slint_ui::uniforms::{buffer_uniform, decay_uniform, editor_buffer_uniform, spectrum_uniform};
use crate::slint_ui::{self};

pub fn render_textures(
    app: &slint_ui::AppWindow,
    registry: &mut WgpuRegistry,
    frames: &FrameReads,
    spectrum: &SpectrumState,
) {
    let mut textures = app.get_textures();
    for &element in ElementId::ALL {
        let Some(spec) = element.spec() else { continue };
        registry.register(element, spec);
        let rendered = match element {
            ElementId::Spectrum => {
                let u = spectrum_uniform(spectrum.levels());
                registry.render_to_image(element, 100, 40, bytemuck::bytes_of(&u))
            }
            ElementId::Buffer => {
                let wetness = wetness_from_params();
                let u = buffer_uniform(&frames.wave.dry, &frames.wave.wet, wetness);
                let (w, h) = element.default_size();
                registry.render_to_image(element, w, h, bytemuck::bytes_of(&u))
            }
            ElementId::EditorBufferL => {
                let u = editor_buffer_uniform(&frames.editor.levels_l, EditorChannel::Left);
                let (w, h) = element.default_size();
                registry.render_to_image(element, w, h, bytemuck::bytes_of(&u))
            }
            ElementId::EditorBufferR => {
                let u = editor_buffer_uniform(&frames.editor.levels_r, EditorChannel::Right);
                let (w, h) = element.default_size();
                registry.render_to_image(element, w, h, bytemuck::bytes_of(&u))
            }
            ElementId::Decay => {
                let u = decay_uniform_for(&frames.frame);
                let (w, h) = element.default_size();
                registry.render_to_image(element, w, h, bytemuck::bytes_of(&u))
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

fn decay_uniform_for(frame: &crate::slint_ui::frames::UiFrame) -> crate::slint_ui::uniforms::DecayUniforms {
    let store = crate::slint_ui::param_store().read().unwrap();
    let is_stereo = store.get("stereo").map(|(v, _)| *v > 0.3).unwrap_or(false);
    let is_ping_pong = store.get("stereo").map(|(v, _)| *v > 0.8).unwrap_or(false);
    let bpm_bound_l = store
        .get("bpm_bound_l")
        .map(|(v, _)| *v > 0.5)
        .unwrap_or(false);
    let bpm_bound_r = store
        .get("bpm_bound_r")
        .map(|(v, _)| *v > 0.5)
        .unwrap_or(false);
    drop(store);
    decay_uniform(
        [frame.feedback.left, frame.feedback.right],
        [frame.decay.left, frame.decay.right],
        frame.bpm,
        [
            is_stereo as u8 as f32,
            is_ping_pong as u8 as f32,
            bpm_bound_l as u8 as f32,
            bpm_bound_r as u8 as f32,
        ],
    )
}

fn wetness_from_params() -> f32 {
    crate::slint_ui::param_store()
        .read()
        .unwrap()
        .get("wetness")
        .map(|(v, _)| *v)
        .unwrap_or(0.5)
}
