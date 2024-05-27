use std::{f32::consts::PI, sync::Arc};

use nih_plug::{nih_dbg, prelude::Param};
use nih_plug_vizia::{
    vizia::{
        prelude::*,
        vg::{LineCap, Paint, Path, Solidity},
    },
    widgets::param_base::ParamWidgetBase,
};

#[allow(dead_code)]
pub struct DragState {
    start_val: f32,
    start_x: f32,
    start_y: f32,
}

#[derive(Lens)]
pub struct ParamKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    default_val: f32,
    drag_status: Option<DragState>,
}

#[allow(dead_code)]
enum ParamKnobEvent {}

impl ParamKnob {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        default_val: f32,
        custom_label: Option<String>
    ) -> Handle<Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            drag_active: false,
            default_val,
            drag_status: None,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let wetness_lens =
                    param_data.make_lens(|param| param.unmodulated_normalized_value());
                VStack::new(cx, |cx| {
                    KnobVisual::new(cx, default_val).value(wetness_lens).class("knob-visual").tooltip(|cx| {
                        Label::new(cx, "TODO: Number");
                    });
                    if let Some(text) = custom_label {
                        Label::new(cx,  &text).class("knob-label");

                    } else {

                        Label::new(cx,  *(&param_data.param().name())).class("knob-label");
                    }

                // Label::new(cx, &param_data.param().to_string()).class("knob-label");
                });
                
            }),
        )
    }
}

impl View for ParamKnob {
    fn element(&self) -> Option<&'static str> {
        Some("param-knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, event_meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                // Start dragging
                self.drag_active = true;
                event_meta.consume();
                cx.capture();
                cx.focus();
                cx.set_active(true);

                self.param_base.begin_set_parameter(cx);
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                // Stop dragging
                if self.drag_active {
                    self.drag_active = false;
                    event_meta.consume();
                    cx.release();
                    cx.set_active(false);

                    self.param_base.end_set_parameter(cx);

                    self.drag_status = None;

                    event_meta.consume();
                }
            }
            WindowEvent::MouseDoubleClick(_) => {
                // Reset to default
                self.param_base.begin_set_parameter(cx);
                self.param_base
                    .set_normalized_value(cx, self.param_base.default_normalized_value());
                self.param_base.end_set_parameter(cx);

                event_meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if self.drag_active {
                    let drag_status = self.drag_status.get_or_insert_with(|| DragState {
                        start_val: self.param_base.unmodulated_normalized_value(),
                        start_x: *x,
                        start_y: *y,
                    });

                    // let delta_x = *x - drag_status.start_x * cx.scale_factor();
                    let delta_y = *y - drag_status.start_y * cx.scale_factor();

                    self.param_base
                        .set_normalized_value(cx, drag_status.start_val - delta_y / 1000.);
                    event_meta.consume();
                }
            }
            WindowEvent::MouseScroll(_x, y) => {
                let delta = *y as f32 / 25.;
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(
                    cx,
                    self.param_base.unmodulated_normalized_value() + delta,
                );
                self.param_base.end_set_parameter(cx);
                event_meta.consume();
            }
            _ => (),
        })
    }
}

enum KnobVisualEvent {
    SetValue(f32),
}

struct KnobVisual {
    val: f32,
}

impl KnobVisual {
    pub fn new(cx: &mut Context, default_val: f32) -> Handle<Self> {
        Self { val: default_val }.build(cx, |_| {})
    }
}

impl View for KnobVisual {
    fn element(&self) -> Option<&'static str> {
        Some("knob-visual")
    }
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            KnobVisualEvent::SetValue(val) => {
                self.val = *val;
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();

        let center_x = bounds.x + bounds.w / 2.;
        let center_y = bounds.y + bounds.h / 2.;

        let mut radius = bounds.w.min(bounds.h) / 2.;

        let girthiness = 0.1 * radius;
        radius -= girthiness;

        let mut path = Path::new();
        let start = 0.75 * PI;
        let range = 1.5 * PI;

        path.arc(
            center_x,
            center_y,
            radius,
            start + self.val * range,
            start,
            Solidity::Solid,
        );
        let arc_color = cx.border_color();
        let mut arc_paint = Paint::color(arc_color.into());
        arc_paint.set_line_width(girthiness);
        arc_paint.set_line_cap(LineCap::Round);

        canvas.stroke_path(&path, &arc_paint);

        let body_color = cx.background_color();
        let mut body_paint = Paint::color(body_color.into());
        path = Path::new();
        path.circle(center_x, center_y, radius - girthiness * 2.);
        canvas.fill_path(&path, &body_paint);

        let arc_pos_x = center_x + (radius - girthiness*2.) * (0.75 * PI + self.val * range).cos();
        let arc_pos_y = center_y + (radius - girthiness*2.) * (0.75 * PI + self.val * range).sin();

        let line_paint = cx.caret_color();
        let mut line_paint = Paint::color(line_paint.into());
        path = Path::new();

        path.move_to(center_x, center_y);
        path.line_to(arc_pos_x, arc_pos_y);

        line_paint.set_line_width(girthiness);
        line_paint.set_line_cap(LineCap::Round);

        canvas.stroke_path(&path, &line_paint);
    }
}

pub trait KnobVisualExt {
    fn value<L: Lens<Target = f32>>(self, lens: L) -> Self;
}

impl KnobVisualExt for Handle<'_, KnobVisual> {
    fn value<L: Lens<Target = f32>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            nih_dbg!(value);
            cx.emit_to(entity, KnobVisualEvent::SetValue(value));
        });

        self
    }
}
