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

            Data {
                params: params.clone(),
            }
            .build(cx);
            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Element::new(cx)
                            .width(Pixels(50.))
                            .height(Stretch(1.))
                            .background_color(Color::black());
                    })
                    .width(Pixels(50.));
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Delax").left(Stretch(1.)).right(Stretch(1.));
                        HStack::new(cx, |cx| {
                            Label::new(cx, "Mono").left(Stretch(1.));
                            Label::new(cx, "Stereo").right(Stretch(1.));
                        })
                        .col_between(Pixels(20.));
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
                            )
                            .width(Pixels(50.))
                            .height(Pixels(50.));
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.feedback_l,
                                params.delay_params.feedback_l.default_normalized_value(),
                            )
                            .width(Pixels(50.))
                            .height(Pixels(50.));
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.delay_len_r,
                                params.delay_params.delay_len_r.default_normalized_value(),
                            )
                            .width(Pixels(50.))
                            .height(Pixels(50.));
                            ParamKnob::new(
                                cx,
                                Data::params,
                                |params| &params.delay_params.feedback_r,
                                params.delay_params.feedback_r.default_normalized_value(),
                            )
                            .width(Pixels(50.))
                            .height(Pixels(50.));
                        }).col_between(Stretch(1.));
                        Label::new(cx, "Filter").left(Stretch(1.)).right(Stretch(1.));
                        HStack::new(cx, |cx| {
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
                        )
                        .width(Pixels(50.))
                        .height(Pixels(50.));
                    })
                    .width(Pixels(50.));
                })
                .col_between(Stretch(1.));
                HStack::new(cx, |cx| {
                    ResizeHandle::new(cx).left(Stretch(1.));
                })
                .height(Pixels(25.));
            });

            // Label::new(cx, "Test");
            // ParamSlider::new(cx, Data::params, |params| &params.wetness);
            // ParamKnob::new(cx, Data::params, |params| &params.wetness, params.wetness.default_normalized_value())
            // ;
        },
    )
}
