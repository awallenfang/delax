use std::sync::{Arc, atomic::Ordering};

use crate::{
    delay_engine::params::DelayMode,
    filters::params::SVFStereoMode,
    params::DelaxParams,
    ui::background::{Background},
};
use meter::PeakMeter;
// use decay_visualizer::DecayVisualizer;
use nih_plug::{editor::Editor, params::Param, prelude::*};
use switch::ParamSwitch;
use vizia_plug::{
    ViziaState, create_vizia_editor,
    vizia::{prelude::*, vg::font_style::Width},
    widgets::ParamButton,
};

use self::knob::ParamKnob;

mod background;
mod decay_visualizer;
mod knob;
mod meter;
mod shader_utils;
mod switch;

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
    ui_page: u8,
}

impl Model for Data {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|delax_event, _| match delax_event {
            DelaxEvent::OpenTab(n) => {
                self.ui_page = *n;
            },
            _ => ()
        });
    }
}

enum DelaxEvent {
    OpenTab(u8),
    ShaderTick(Duration)
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (550, 310))
}

pub(crate) fn create(
    params: Arc<DelaxParams>,
    editor_state: Arc<ViziaState>,
    input_data: Arc<InputData>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        editor_state,
        vizia_plug::ViziaTheming::Custom,
        move |cx, _ui_cx| {
            let timer = cx.add_timer(Duration::from_secs(1), None, |cx, reason| {
                match reason {
                    TimerAction::Tick(delta) => {
                        nih_dbg!("Tick sent");
                        cx.emit_custom(Event::new(DelaxEvent::ShaderTick(delta)).propagate(Propagation::Subtree));
                    },
                    TimerAction::Start => {nih_dbg!("Timer started");},
                    TimerAction::Stop => ()
                }
                nih_dbg!("Callback");
            });
            cx.start_timer(timer);

            // assets::register_noto_sans_light(cx);
            // assets::register_noto_sans_thin(cx);
            let _ = cx.add_stylesheet(include_style!("src/ui/style.css"));
            
            Data {
                params: params.clone(),
                input_data: input_data.clone(),
                ui_page: 0,
            }
            .build(cx);
        let internal_params = params.clone();
        ZStack::new(cx, |cx| {

                Background::new(cx)
                    .width(Stretch(1.))
                    .height(Stretch(1.));
                VStack::new(cx, |cx| {
                    // Top bar
                    nav_bar(cx, internal_params.clone());

                    HStack::new(cx, |cx| {
                        Binding::new(cx, Data::ui_page, move |cx, lens| {
                            let page = lens.get(cx);
                            match page {
                                0 => main_page(cx, internal_params.clone()),
                                1 => filter_page(cx, internal_params.clone()),
                                2 => banks_page(cx, internal_params.clone()),
                                _ => unimplemented!(),
                            }
                        });
                    });
                })
                .id("main");
            });
        },
    )
}

fn nav_bar(cx: &mut Context, params: Arc<DelaxParams>) {
    HStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            PeakMeter::new(
                cx,
                Data::input_data.map(|d| d.in_l.load(Ordering::Relaxed)),
                meter::MeterDirection::Right,
            )
            .class("nav-bar-meter");
            PeakMeter::new(
                cx,
                Data::input_data.map(|d| d.in_r.load(Ordering::Relaxed)),
                meter::MeterDirection::Right,
            )
            .class("nav-bar-meter");
        })
        .class("nav-bar-meter-stack");
        HStack::new(cx, |cx| {
            Button::new(cx, |cx| Label::new(cx, "Delay"))
                .on_press(|ex| ex.emit(DelaxEvent::OpenTab(0)));
            Element::new(cx).class("vr");
            Button::new(cx, |cx| Label::new(cx, "Filters"))
                .on_press(|ex| ex.emit(DelaxEvent::OpenTab(1)));
            Element::new(cx).class("vr");
            Button::new(cx, |cx| Label::new(cx, "Banks"))
                .on_press(|ex| ex.emit(DelaxEvent::OpenTab(2)));
        })
        .class("nav-button-hstack");

        VStack::new(cx, |cx| {
            PeakMeter::new(
                cx,
                Data::input_data.map(|d| d.out_l.load(Ordering::Relaxed)),
                meter::MeterDirection::Right,
            )
            .class("nav-bar-meter");
            PeakMeter::new(
                cx,
                Data::input_data.map(|d| d.out_r.load(Ordering::Relaxed)),
                meter::MeterDirection::Right,
            )
            .class("nav-bar-meter");
        })
        .class("nav-bar-meter-stack");

        ParamKnob::new(
            cx,
            Data::params,
            |inter_params| &inter_params.wetness,
            params.wetness.default_normalized_value(),
            None,
            Data::params.map(|p| true),
        );
    })
    .class("nav-bar");
}

fn main_page(cx: &mut Context, params: Arc<DelaxParams>) {
    HStack::new(cx, |cx| {
        // Box for the input meters

        // Box for most of the parameter controls
        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "Mono").left(Stretch(1.));
                ParamSwitch::new(
                    cx,
                    Data::params,
                    |params| &params.delay_params.stereo_delay,
                    false,
                );
                Label::new(cx, "Stereo").right(Stretch(1.));
            })
            .class("switch-block");

            // All the delay knobs
            HStack::new(cx, |cx| {
                // The mono knobs
                ParamKnob::new(
                    cx,
                    Data::params,
                    |params| &params.delay_params.delay_len_l,
                    params.delay_params.delay_len_l.default_normalized_value(),
                    None,
                    Data::params.map(|p| true),
                );
                ParamKnob::new(
                    cx,
                    Data::params,
                    |params| &params.delay_params.feedback_l,
                    params.delay_params.feedback_l.default_normalized_value(),
                    None,
                    Data::params.map(|p| true),
                );

                // Only show the stereo delay knobs if the whole delay is stereo

                ParamKnob::new(
                    cx,
                    Data::params,
                    |params| &params.delay_params.delay_len_r,
                    params.delay_params.delay_len_r.default_normalized_value(),
                    Some("Delay".to_string()),
                    Data::params.map(|p| p.delay_params.stereo_delay.value() == DelayMode::Stereo),
                );
                ParamKnob::new(
                    cx,
                    Data::params,
                    |params| &params.delay_params.feedback_r,
                    params.delay_params.feedback_r.default_normalized_value(),
                    Some("Feedback".to_string()),
                    Data::params.map(|p| p.delay_params.stereo_delay.value() == DelayMode::Stereo),
                );
            })
            .horizontal_gap(Stretch(1.));
        })
        .class("main-box")
        .alignment(Alignment::Center);
    })
    .class("main-page")
    .width(Stretch(1.));
}

fn filter_page(cx: &mut Context, params: Arc<DelaxParams>) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            // TODO: Toggle Button
            Label::new(cx, "Mono").left(Stretch(1.));
            ParamSwitch::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_stereo_mode,
                false,
            );
            Label::new(cx, "Stereo").right(Stretch(1.));
        })
        .class("switch-block");

        // All the filter knobs
        HStack::new(cx, |cx| {
            // The mono knobs
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_cutoff_l,
                params.filter_params.svf_cutoff_l.default_normalized_value(),
                Some("Cutoff".to_string()),
                Data::params.map(|p| true),
            );
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_res_l,
                params.filter_params.svf_res_l.default_normalized_value(),
                Some("Res".to_string()),
                Data::params.map(|p| true),
            );
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_mix_l,
                params.filter_params.svf_mix_l.default_normalized_value(),
                Some("Mix".to_string()),
                Data::params.map(|p| true),
            );

            // Only show the stereo filter knobs if the whole filter is stereo
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_cutoff_r,
                params.filter_params.svf_cutoff_r.default_normalized_value(),
                Some("Cutoff".to_string()),
                Data::params
                    .map(|p| p.filter_params.svf_stereo_mode.value() == SVFStereoMode::Stereo),
            );
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_res_r,
                params.filter_params.svf_res_r.default_normalized_value(),
                Some("Res".to_string()),
                Data::params
                    .map(|p| p.filter_params.svf_stereo_mode.value() == SVFStereoMode::Stereo),
            );
            ParamKnob::new(
                cx,
                Data::params,
                |params| &params.filter_params.svf_mix_r,
                params.filter_params.svf_mix_r.default_normalized_value(),
                Some("Mix".to_string()),
                Data::params
                    .map(|p| p.filter_params.svf_stereo_mode.value() == SVFStereoMode::Stereo),
            );
        })
        .class("parameter-list");
    })
    .class("filter-page");
}

fn banks_page(cx: &mut Context, params: Arc<DelaxParams>) {
    VStack::new(cx, |cx| {
        Element::new(cx).class("banks-block");
        Element::new(cx).class("banks-block");
        Element::new(cx).class("banks-block");
    })
    .class("banks-page");
}
