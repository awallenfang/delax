use std::f32::consts::PI;

use nih_plug::prelude::Param;
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

/// A knob for nih_plug parameters.
#[derive(Lens)]
pub struct ParamKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    default_val: f32,
    drag_status: Option<DragState>,
    active: bool,
}

pub enum ParamKnobEvent {
    SetActive(bool),
}

impl ParamKnob {
    /// Create a new knob for a nih_plug parameter
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
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                // Grab a lens to the bound value
                let param_lens =
                    param_data.make_lens(|param| param.unmodulated_normalized_value());

                // Make a binding to the active_lens
                let entity = cx.current();
                Binding::new(cx, active_lens, move |cx, val| {
                    let value = val.get(cx);
                    cx.emit_to(entity, ParamKnobEvent::SetActive(value));
                });

                // Stack the knob and a label vertically
                VStack::new(cx, |cx| {
                    KnobVisual::new(cx, default_val)
                        .value(param_lens)
                        .class("knob-visual")
                        .tooltip(|cx| {
                            Binding::new(cx, param_lens, move |cx, val| {
                                Label::new(
                                    cx,
                                    &format!(
                                        "{}",
                                        param_data
                                            .param()
                                            .normalized_value_to_string(val.get(cx), true)
                                    ),
                                )
                                .class("knob-tooltip");
                            });
                        })
                        .active(active_lens);

                    if let Some(text) = custom_label {
                        Label::new(cx, &text).class("knob-label");
                    } else {
                        Label::new(cx, *(&param_data.param().name())).class("knob-label");
                    }
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
        // Internal events
        event.map(|param_knob_event, _| match param_knob_event {
            ParamKnobEvent::SetActive(active) => {
                self.active = *active;
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

                    let delta_y = *y - drag_status.start_y;

                    self.param_base
                        .set_normalized_value(cx, drag_status.start_val - delta_y / 1000.);
                    event_meta.consume();
                }
            }
            WindowEvent::MouseScroll(_x, y) => {
                if self.active {
                    let delta = *y as f32 / 25.;
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

enum KnobVisualEvent {
    SetValue(f32),
    SetActive(bool),
}

struct KnobVisual {
    val: f32,
    active: bool,
}

impl KnobVisual {
    pub fn new(cx: &mut Context, default_val: f32) -> Handle<Self> {
        Self {
            val: default_val,
            active: true,
        }
        .build(cx, |_| {})
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
            KnobVisualEvent::SetActive(active) => {
                self.active = *active;
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // Grab all the bounds
        let bounds = cx.bounds();

        let center_x = bounds.x + bounds.w / 2.;
        let center_y = bounds.y + bounds.h / 2.;

        let mut radius = bounds.w.min(bounds.h) / 2.;

        let girthiness = 0.1 * radius;
        radius -= girthiness;

        // Grab all the colors
        let mut arc_color = cx.border_color();

        if !self.active {
            arc_color = Color::rgba(arc_color.r(), arc_color.g(), arc_color.b(), 100);
        }

        let mut body_color = cx.background_color();
        if !self.active {
            body_color = Color::rgba(body_color.r(), body_color.g(), body_color.b(), 100);
        }

        let mut line_paint = cx.caret_color();

        if !self.active {
            line_paint = Color::rgba(line_paint.r(), line_paint.g(), line_paint.b(), 100);
        }

        // Arc path
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
        

        let mut arc_paint = Paint::color(arc_color.into());
        arc_paint.set_line_width(girthiness);
        arc_paint.set_line_cap(LineCap::Round);

        canvas.stroke_path(&path, &arc_paint);

        
        // Body path
        let body_paint = Paint::color(body_color.into());
        path = Path::new();
        path.circle(center_x, center_y, radius - girthiness * 2.);
        canvas.fill_path(&path, &body_paint);

        let arc_pos_x =
            center_x + (radius - girthiness * 2.) * (0.75 * PI + self.val * range).cos();
        let arc_pos_y =
            center_y + (radius - girthiness * 2.) * (0.75 * PI + self.val * range).sin();

        
        // Line path
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
    fn active<L: Lens<Target = bool>>(self, lens: L) -> Self;
}

impl KnobVisualExt for Handle<'_, KnobVisual> {
    fn value<L: Lens<Target = f32>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, KnobVisualEvent::SetValue(value));
        });

        self
    }

    fn active<L: Lens<Target = bool>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, KnobVisualEvent::SetActive(value));
        });

        self
    }
}
