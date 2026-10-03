use slint::{Model, ModelRc, VecModel};

use crate::slint_ui::derived::{
    JumpCursor, SpectrumState, normalize_jumps, normalize_portals, normalize_segments,
};
use crate::slint_ui::frames::{BufferChannel, FrameReads};
use crate::slint_ui::{self};

pub fn sync_slint(
    app: &slint_ui::AppWindow,
    frames: &FrameReads,
    spectrum: &mut SpectrumState,
    jumps: &mut JumpCursor,
) {
    if let Some(raw) = &frames.spectrum_raw {
        spectrum.push_raw(&raw.samples);
    }
    app.set_dry_buffer(push_model(app.get_dry_buffer(), frames.wave.dry.to_vec()));
    app.set_wet_buffer(push_model(app.get_wet_buffer(), frames.wave.wet.to_vec()));

    let block = &frames.frame;
    app.set_bpm(block.bpm);
    app.set_header_data(crate::slint_ui::HeaderData {
        in_level_l: block.meters_in.left,
        in_level_r: block.meters_in.right,
        out_level_l: block.meters_out.left,
        out_level_r: block.meters_out.right,
    });

    let mut editor = crate::slint_ui::EditorData {
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
        read_portals_l: app.get_editor_data().read_portals_l,
        read_portals_r: app.get_editor_data().read_portals_r,
    };

    if let Some(snap) = frames.jumps.as_ref().and_then(|j| jumps.fresh(j)) {
        let lens = block.active_len;
        for ch in [BufferChannel::Left, BufferChannel::Right] {
            let len = *lens.get(ch);
            let s = snap.channels.get(ch);
            let (target_read, target_write, target_read_seg, target_write_seg, target_portals) =
                match ch {
                    BufferChannel::Left => (
                        &mut editor.read_jumps_l,
                        &mut editor.write_jumps_l,
                        &mut editor.read_segments_l,
                        &mut editor.write_segments_l,
                        &mut editor.read_portals_l,
                    ),
                    BufferChannel::Right => (
                        &mut editor.read_jumps_r,
                        &mut editor.write_jumps_r,
                        &mut editor.read_segments_r,
                        &mut editor.write_segments_r,
                        &mut editor.read_portals_r,
                    ),
                };
            *target_read = push_model(
                std::mem::replace(target_read, ModelRc::new(VecModel::from(vec![]))),
                normalize_jumps(&s.read, len),
            );
            *target_write = push_model(
                std::mem::replace(target_write, ModelRc::new(VecModel::from(vec![]))),
                normalize_jumps(&s.write, len),
            );
            *target_read_seg = push_model(
                std::mem::replace(target_read_seg, ModelRc::new(VecModel::from(vec![]))),
                normalize_segments(&s.read_segments, len),
            );
            *target_write_seg = push_model(
                std::mem::replace(target_write_seg, ModelRc::new(VecModel::from(vec![]))),
                normalize_segments(&s.write_segments, len),
            );
            *target_portals = push_model(
                std::mem::replace(target_portals, ModelRc::new(VecModel::from(vec![]))),
                normalize_portals(&s.read_portals, len),
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
