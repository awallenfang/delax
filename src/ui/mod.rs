use std::sync::Arc;

use crate::params::DelaxParams;
use nih_plug::{editor::Editor, formatters::v2s_f32_rounded, params::Param};
use nih_plug_vizia::{assets, create_vizia_editor, vizia::prelude::*, widgets::*, ViziaState};

use self::knob::ParamKnob;

mod knob;

#[derive(Lens)]
struct Data {
    params: Arc<DelaxParams>,
}

impl Model for Data {}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (500, 275))
}

pub(crate) fn create(
    params: Arc<DelaxParams>,
    editor_state: Arc<ViziaState>,
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
            }
            .build(cx);
            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        // TODO: Peakmeter
                        Element::new(cx)
                            .width(Pixels(50.))
                            .height(Stretch(1.))
                            .background_color(Color::black());
                    })
                    .class("meter-box");
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Delax").class("centered");
                        HStack::new(cx, |cx| {
                            // TODO: Toggle button
                            Label::new(cx, "Mono").left(Stretch(1.));
                            Label::new(cx, "Stereo").right(Stretch(1.));
                        })
                        .col_between(Pixels(20.));
                        // TODO: Delay visualizer
                        Element::new(cx)
                            .width(Stretch(1.))
                            .height(Pixels(50.))
                            .background_color(Color::black());
                        HStack::new(cx, |cx| {
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.delay_len_l,
                                params.delay_params.delay_len_l.default_normalized_value(),
                                None
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.feedback_l,
                                params.delay_params.feedback_l.default_normalized_value(),
                                None
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.delay_len_r,
                                params.delay_params.delay_len_r.default_normalized_value(),
                                Some("Delay".to_string())
                            );
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.feedback_r,
                                params.delay_params.feedback_r.default_normalized_value(),
                                Some("Feedback".to_string())
                            );
                        }).col_between(Stretch(1.));
                        Label::new(cx, "Filter").class("centered");
                        HStack::new(cx, |cx| {
                            // TODO: Toggle Button
                            Label::new(cx, "Mono").left(Stretch(1.));
                            Label::new(cx, "Stereo").right(Stretch(1.));
                        })
                        .col_between(Pixels(20.));
                    }).width(Pixels(350.));
                    VStack::new(cx, |cx| {
                        Element::new(cx)
                            .width(Pixels(50.))
                            .height(Stretch(1.))
                            .background_color(Color::black());
                        ParamKnob::new(
                            cx,
                            Data::params,
                            |params| &params.wetness,
                            params.wetness.default_normalized_value(),
                            None
                        );
                    })
                    .class("meter-box");
                })
                .id("main-hstack");
                HStack::new(cx, |cx| {
                    ResizeHandle::new(cx);
                })
                .id("resize-handle-box");
            }).id("main");
        },
    )
}
