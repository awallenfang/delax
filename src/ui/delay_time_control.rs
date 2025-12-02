use std::f32::consts::PI;

use nih_plug::{nih_dbg, prelude::Param};
use vizia_plug::{
    vizia::{
        prelude::*,
        vg::{self, Paint, PaintCap, Path, PathDirection, Rect},
    },
    widgets::param_base::ParamWidgetBase,
};

use crate::{params::DelaxParams, ui::knob::ParamKnob};

#[allow(dead_code)]
pub struct DragState {
    start_val: f32,
    start_x: f32,
    start_y: f32,
}

/// A knob for nih_plug parameters.
#[derive(Lens)]
pub struct DelayTimeControl {
    param_base: ParamWidgetBase,
    drag_active: bool,
    default_val: f32,
    drag_status: Option<DragState>,
    active: bool,
    mult_16: u8 // Multiplier on the slider 1: 16th, 2: 8th, 4: 4th, 8: halves, 16: wholes
}

pub enum DelayTimeControlEvent {
    SetActive(bool),
    SetMult(u8)
}

impl DelayTimeControl {
    ///
    /// Requires the following:
    ///
    /// - context
    /// - Lens to all params
    /// - Function mapping all params to a param
    /// - Default value
    /// - Option for custom label
    /// - Lens to whether or not it is active. Can just be a lens on true
    pub fn new<L, La, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        default_val: f32,
        custom_label: Option<String>,
        active_lens: La,
    ) -> Handle<Self>
    where
        L: Lens<Target = Params> + Clone,
        La: Lens<Target = bool> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            drag_active: false,
            default_val,
            drag_status: None,
            active: true,
            mult_16: 1
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                // Grab a lens to the bound value
                let param_lens = param_data.make_lens(|param| param.unmodulated_normalized_value());
                
                // Make a binding to the active_lens
                let entity = cx.current();
                Binding::new(cx, active_lens, move |cx, val| {
                    let value = val.get(cx);
                    cx.emit_to(entity, DelayTimeControlEvent::SetActive(value));
                });

                // Stack the knob and a label vertically
                VStack::new(cx, move |cx| {
                    HStack::new(cx, |cx| {
                        Button::new(cx, |cx| Label::new(cx, "16th")).on_press(|cx| cx.emit(DelayTimeControlEvent::SetMult(1)));
                        Button::new(cx, |cx| Label::new(cx, "8th")).on_press(|cx| cx.emit(DelayTimeControlEvent::SetMult(2)));
                        Button::new(cx, |cx| Label::new(cx, "4th")).on_press(|cx| cx.emit(DelayTimeControlEvent::SetMult(4)));
                    });
                    DelayTimeControlVisual::new(cx, default_val)
                        .value(param_lens)
                        .class("delay-time-visual")
                        .tooltip(move |cx| {
                            Tooltip::new(cx, |cx| {
                                Binding::new(cx, param_lens, move |cx, val| {
                                    Label::new(
                                        cx,
                                        &format!(
                                            "{}",
                                            param_data
                                                .param()
                                                .normalized_value_to_string(val.get(cx), true),
                                                
                                        ),
                                    )
                                    .class("delay-time-tooltip");
                                })
                            })
                        })
                        .width(Stretch(1.))
                        .height(Pixels(25.))
                        .active(active_lens);

                    // if let Some(text) = custom_label {
                    //     Label::new(cx, &text).class("delay-time-label");
                    // } else {
                    //     Label::new(cx, *(&param_data.param().name())).class("delay-time-label");
                    // }
                })
                .alignment(Alignment::TopCenter);
            }),
        )
    }
}

impl View for DelayTimeControl {
    fn element(&self) -> Option<&'static str> {
        Some("delay-time-control")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // Internal events
        event.map(|param_knob_event, _| match param_knob_event {
            DelayTimeControlEvent::SetActive(active) => {
                self.active = *active;
                cx.needs_redraw();
            }
            DelayTimeControlEvent::SetMult(mult) => {
                let old_mult = self.mult_16;
                self.mult_16 = *mult;
                let param_change_multiplier = *mult as f32 / old_mult as f32;

                self.param_base.begin_set_parameter(cx);
                let new_value: f32 = self.param_base.unmodulated_normalized_value() * param_change_multiplier;
                self.param_base
                    .set_normalized_value(cx, new_value);
                self.param_base.end_set_parameter(cx);
                cx.needs_redraw();
            }
        });

        // External events
        event.map(|window_event, event_meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if self.active {
                    // Start dragging
                    self.drag_active = true;
                    event_meta.consume();
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);

                    self.param_base.begin_set_parameter(cx);
                }
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
                if self.active {
                    // Reset to default
                    self.param_base.begin_set_parameter(cx);
                    self.param_base
                        .set_normalized_value(cx, self.param_base.default_normalized_value());
                    self.param_base.end_set_parameter(cx);

                    event_meta.consume();
                }
            }
            WindowEvent::MouseMove(x, y) => {
                if self.drag_active {
                    let drag_status = self.drag_status.get_or_insert_with(|| DragState {
                        start_val: self.param_base.unmodulated_normalized_value(),
                        start_x: *x,
                        start_y: *y,
                    });

                    let delta_x = *x - drag_status.start_x;

                    self.param_base
                        .set_normalized_value(cx, drag_status.start_val + delta_x / 100.);
                    event_meta.consume();
                }
            }
            WindowEvent::MouseScroll(_x, y) => {
                if self.active {
                    let delta = -*y as f32 / 25.;
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(
                        cx,
                        self.param_base.unmodulated_normalized_value() + delta,
                    );
                    self.param_base.end_set_parameter(cx);
                    event_meta.consume();
                }
            }

            _ => (),
        })
    }
}

enum DelayTimeControlVisualEvent {
    SetValue(f32),
    SetActive(bool),
    SetMult(u8),
}

struct DelayTimeControlVisual {
    val: f32,
    active: bool,
    mult_16: u8
}

impl DelayTimeControlVisual {
    pub fn new(cx: &mut Context, default_val: f32) -> Handle<Self> {
        Self {
            val: default_val,
            active: true,
            mult_16: 1
        }
        .build(cx, |_| {})
    }
}

impl View for DelayTimeControlVisual {
    fn element(&self) -> Option<&'static str> {
        Some("delay-time-visual")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            DelayTimeControlVisualEvent::SetValue(val) => {
                self.val = *val;
                cx.needs_redraw();
            }
            DelayTimeControlVisualEvent::SetActive(active) => {
                self.active = *active;
                cx.needs_redraw();
            }
            DelayTimeControlVisualEvent::SetMult(mult) => {
                self.mult_16 = *mult;
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &vg::Canvas) {
        // Grab all the bounds
        let bounds = cx.bounds();

        let mut radius = bounds.h / 2.;

        let girthiness = 0.1 * radius;
        radius -= girthiness;

        // Grab all the colors
        let mut bar_color = Color::white();

        if !self.active {
            bar_color = Color::rgba(bar_color.r(), bar_color.g(), bar_color.b(), 100);
        }

        let mut active_color = Color::red();
        if !self.active {
            active_color = Color::rgba(active_color.r(), active_color.g(), active_color.b(), 100);
        }

        let mut line_color = Color::green();

        if !self.active {
            line_color = Color::rgba(line_color.r(), line_color.g(), line_color.b(), 100);
        }

        // Horizontal bar
        let mut path = Path::new();
        let rect = path.add_round_rect(
            vg::Rect::new(
                bounds.x,
                bounds.y,
                bounds.x + bounds.w,
                bounds.y + bounds.h / 2.,
            ),
            (2., 2.),
            PathDirection::CW,
        );

        let mut bar_paint = Paint::default();
        bar_paint.set_color(bar_color);
        bar_paint.set_stroke_width(girthiness);
        bar_paint.set_stroke_cap(PaintCap::Round);
        bar_paint.set_style(vg::PaintStyle::Fill);
        bar_paint.set_anti_alias(true);

        canvas.draw_path(&path, &bar_paint);
        
        let mut caret_path = Path::new();
        let val = self.val.get(cx);
        // Set the relative position to be half the designated length at the left side and double at the right side
        // Uses the multiplier to get the situation 
        // mult=1 -> left 0.5, right 2
        // mult=2 -> left 1, right 4
        // mult=4 -> left 2, right 8
        // mult=8 -> left 4, right 16
        let left: f32 = 0.5 * self.mult_16 as f32;
        let right = 2. * self.mult_16 as f32;
        let mut relative_position = (val - left) / (right - left);
        relative_position = relative_position.clamp(0., 1.);
        let rect = caret_path.add_round_rect(
            vg::Rect::new(
                bounds.x + (relative_position) * bounds.w + 1.,
                bounds.y,
                bounds.x + (relative_position) * bounds.w - 1.,
                bounds.y + bounds.h / 2.
            ),
            (5., 5.),
            PathDirection::CW
        );

        let mut caret_paint = Paint::default();
        caret_paint.set_color(active_color);
        caret_paint.set_stroke_width(girthiness);
        caret_paint.set_stroke_cap(PaintCap::Round);
        caret_paint.set_style(vg::PaintStyle::Fill);
        caret_paint.set_anti_alias(true);
        
        canvas.draw_path(&caret_path, &caret_paint);

        let line_width = 5.;
        let mut line_path = Path::new();
        let rect = line_path.add_round_rect(
            vg::Rect::new(
                bounds.x + bounds.w / 2. - line_width / 2.,
                bounds.y + bounds.h / 2.,
                bounds.x + bounds.w / 2. + line_width / 2.,
                bounds.y + bounds.h,
            ),
            (5., 5.),
            PathDirection::CW,
        );
        let rect = line_path.add_round_rect(
            vg::Rect::new(
                bounds.x + bounds.w / 3. - line_width / 2.,
                bounds.y + bounds.h / 2.,
                bounds.x + bounds.w / 3. + line_width / 2.,
                bounds.y + bounds.h,
            ),
            (5., 5.),
            PathDirection::CW,
        );
        let rect = line_path.add_round_rect(
            vg::Rect::new(
                bounds.x + bounds.w - line_width,
                bounds.y + bounds.h / 2.,
                bounds.x + bounds.w,
                bounds.y + bounds.h,
            ),
            (5., 5.),
            PathDirection::CW,
        );

        let mut line_paint = Paint::default();
        line_paint.set_color(line_color);
        line_paint.set_stroke_cap(PaintCap::Round);
        line_paint.set_style(vg::PaintStyle::Fill);
        line_paint.set_anti_alias(true);

        canvas.draw_path(&line_path, &line_paint);

    }
}

pub trait DelayTimeControlVisualExt {
    fn value<L: Lens<Target = f32>>(self, lens: L) -> Self;
    fn active<L: Lens<Target = bool>>(self, lens: L) -> Self;
}

impl DelayTimeControlVisualExt for Handle<'_, DelayTimeControlVisual> {
    fn value<L: Lens<Target = f32>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let mut value = val.get(cx);
            value *= 32.;
            cx.emit_to(entity, DelayTimeControlVisualEvent::SetValue(value));
        });

        self
    }

    fn active<L: Lens<Target = bool>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, DelayTimeControlVisualEvent::SetActive(value));
        });

        self
    }
}
