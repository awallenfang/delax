use std::sync::{atomic::Ordering, Arc};

use crate::{delay_engine::params::DelayMode, filters::params::SVFStereoMode, params::DelaxParams};
use decay_visualizer::DecayVisualizer;
use nih_plug::{editor::Editor, params::Param, prelude::*};
use nih_plug_vizia::{
    assets, create_vizia_editor,
    vizia::prelude::*,
    widgets::{ParamButton, ResizeHandle},
    ViziaState,
};

use self::{knob::ParamKnob, meter::PeakMeter};

mod decay_visualizer;
mod knob;
mod meter;

pub struct InputData {
    pub in_l: AtomicF32,
    pub in_r: AtomicF32,
    pub out_l: AtomicF32,
    pub out_r: AtomicF32,
}

impl Default for InputData {
    fn default() -> Self {
        Self {
            in_l: AtomicF32::new(0.),
            in_r: AtomicF32::new(0.),
            out_l: AtomicF32::new(0.),
            out_r: AtomicF32::new(0.),
        }
    }
}

#[derive(Lens)]
struct Data {
    params: Arc<DelaxParams>,
    input_data: Arc<InputData>,
}

impl Model for Data {}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (500, 275))
}

pub(crate) fn create(
    params: Arc<DelaxParams>,
    editor_state: Arc<ViziaState>,
    input_data: Arc<InputData>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        editor_state,
        nih_plug_vizia::ViziaTheming::Custom,
        move |cx, _| {
            assets::register_noto_sans_light(cx);
            assets::register_noto_sans_thin(cx);
            let _ = cx.add_stylesheet(include_style!("src/ui/style.css"));

            Data {
                params: params.clone(),
                input_data: input_data.clone(),
            }
            .build(cx);
            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    // Box for the input meters
                    VStack::new(cx, |cx| {
                        // PeakMeter::new(
                        //     cx,
                        //     Data::input_data
                        //         .map(|d| d.in_l.load(Ordering::Relaxed)),
                        // )
                        // .width(Pixels(50.))
                        // .height(Pixels(200.));
                        // Label::new(cx, Data::input_data.map(|d| d.in_l.load(Ordering::Relaxed)));
                    })
                    .class("meter-box");

                    // Box for most of the parameter controls
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Delax").class("centered");
                        HStack::new(cx, |cx| {
                            // TODO: Toggle button
                            Label::new(cx, "Mono").left(Stretch(1.));
                            ParamButton::new(cx, Data::params, |params| {
                                &params.delay_params.stereo_delay
                            });
                            Label::new(cx, "Stereo").right(Stretch(1.));
                        })
                        .col_between(Pixels(20.));
                        // TODO: Delay visualizer
                        // DecayVisualizer::new(cx);

                        // All the delay knobs
                        HStack::new(cx, |cx| {
                            // The mono knobs
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.delay_len_l,
                                params.delay_params.delay_len_l.default_normalized_value(),
                                None,
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.feedback_l,
                                params.delay_params.feedback_l.default_normalized_value(),
                                None,
                            );

                            // Only show the stereo delay knobs if the whole delay is stereo
                            let params_clone = params.clone();
                            Binding::new(
                                cx,
                                Data::params.map(|p| {
                                    p.delay_params.stereo_delay.value() == DelayMode::Stereo
                                }),
                                move |cx, val| {
                                    if val.get(cx) {
                                        ParamKnob::new(
                                            cx,
                                            Data::params,
                                            |params| &params.delay_params.delay_len_r,
                                            params_clone
                                                .delay_params
                                                .delay_len_r
                                                .default_normalized_value(),
                                            Some("Delay".to_string()),
                                        );
                                        ParamKnob::new(
                                            cx,
                                            Data::params,
                                            |params| &params.delay_params.feedback_r,
                                            params_clone
                                                .delay_params
                                                .feedback_r
                                                .default_normalized_value(),
                                            Some("Feedback".to_string()),
                                        );
                                    }
                                },
                            );
                        })
                        .col_between(Stretch(1.));
                        Label::new(cx, "Filter").class("centered");
                        HStack::new(cx, |cx| {
                            // TODO: Toggle Button
                            Label::new(cx, "Mono").left(Stretch(1.));
                            ParamButton::new(cx, Data::params, |params| {
                                &params.filter_params.svf_stereo_mode
                            });
                            Label::new(cx, "Stereo").right(Stretch(1.));
                        })
                        .col_between(Pixels(20.));

                        // All the filter knobs
                        HStack::new(cx, |cx| {
                            // The mono knobs
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.filter_params.svf_cutoff_l,
                                params.filter_params.svf_cutoff_l.default_normalized_value(),
                                Some("Cutoff".to_string()),
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.filter_params.svf_res_l,
                                params.filter_params.svf_res_l.default_normalized_value(),
                                Some("Res".to_string()),
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.filter_params.svf_mix_l,
                                params.filter_params.svf_mix_l.default_normalized_value(),
                                Some("Mix".to_string()),
                            );

                            // Only show the stereo filter knobs if the whole filter is stereo
                            let params_clone = params.clone();
                            Binding::new(
                                cx,
                                Data::params.map(|p| {
                                    p.filter_params.svf_stereo_mode.value() == SVFStereoMode::Stereo
                                }),
                                move |cx, val| {
                                    if val.get(cx) {
                                        ParamKnob::new(
                                            cx,
                                            Data::params,
                                            |params| &params.filter_params.svf_cutoff_r,
                                            params_clone
                                                .filter_params
                                                .svf_cutoff_r
                                                .default_normalized_value(),
                                            Some("Cutoff".to_string()),
                                        );
                                        ParamKnob::new(
                                            cx,
                                            Data::params,
                                            |params| &params.filter_params.svf_res_r,
                                            params_clone
                                                .filter_params
                                                .svf_res_r
                                                .default_normalized_value(),
                                            Some("Res".to_string()),
                                        );
                                        ParamKnob::new(
                                            cx,
                                            Data::params,
                                            |params| &params.filter_params.svf_mix_r,
                                            params_clone
                                                .filter_params
                                                .svf_mix_r
                                                .default_normalized_value(),
                                            Some("Mix".to_string()),
                                        );
                                    }
                                },
                            );
                        })
                        .col_between(Stretch(1.));
                    })
                    .class("main-box");
                    VStack::new(cx, |cx| {
                        // Element::new(cx)
                        //     .width(Pixels(50.))
                        //     .height(Stretch(1.))
                        //     .background_color(Color::black());
                        ParamKnob::new(
                            cx,
                            Data::params,
                            |params| &params.wetness,
                            params.wetness.default_normalized_value(),
                            None,
                        )
                        .top(Stretch(1.));
                    })
                    .class("meter-box");
                })
                .id("main-hstack");
                HStack::new(cx, |cx| {
                    ResizeHandle::new(cx);
                })
                .id("resize-handle-box");
            })
            .id("main");
        },
    )
}
