use nih_plug::params::Param;
use vizia_plug::{
    vizia::{
        prelude::*,
        vg::{Canvas, Paint, PaintCap, PaintStyle, Path},
    },
    widgets::param_base::ParamWidgetBase,
};

/// A switch to control a boolean nih-plug parameter
pub struct ParamSwitch {
    param_base: ParamWidgetBase,
    active: bool,
}

pub enum ParamSwitchEvent {
    SetActive(bool),
}
impl ParamSwitch {
    pub fn new<L, Params, P, FMap, La>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        default_val: bool,
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
            active: true,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let param_lens = param_data.make_lens(|param| param.unmodulated_normalized_value());
                // Make a binding to the active_lens
                let entity = cx.current();
                Binding::new(cx, active_lens, move |cx, val| {
                    let value = val.get(cx);
                    cx.emit_to(entity, ParamSwitchEvent::SetActive(value));
                });
                // Simply create a visual instance on the lens
                ParamSwitchVisual::new(cx, default_val)
                    .value(param_lens)
                    .class("switch-visual")
                    .height(Stretch(1.))
                    .width(Stretch(1.))
                    .active(active_lens);
            }),
        )
    }

    /// Toggles the value of the parameter
    fn toggle(&mut self, cx: &mut EventContext) {
        let current = self.param_base.unmodulated_normalized_value();

        let new_val = if current > 0.5 { 0. } else { 1. };

        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, new_val);
        self.param_base.end_set_parameter(cx);
    }
}

impl View for ParamSwitch {
    fn element(&self) -> Option<&'static str> {
        Some("param-switch")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|param_knob_event, _| match param_knob_event {
            ParamSwitchEvent::SetActive(active) => {
                self.active = *active;
                cx.needs_redraw();
            }
        });
        event.map(|input_event, _| if let WindowEvent::MouseDown(MouseButton::Left) = input_event {
            if self.active {
                self.toggle(cx);
            }
        })
    }
}

pub enum ParamSwitchVisualEvent {
    SetValue(f32),
    SetActive(bool),
}
struct ParamSwitchVisual {
    val: bool,
    active: bool,
}

impl ParamSwitchVisual {
    pub fn new(cx: &mut Context, default_val: bool) -> Handle<Self> {
        Self {
            val: default_val,
            active: true,
        }
        .build(cx, |_| {})
    }
}

impl View for ParamSwitchVisual {
    fn element(&self) -> Option<&'static str> {
        Some("switch-visual")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        // Grab all the bounds
        // This assumes a horizontal alignment, so it restricts the height if it is larger.
        // NOTE: At a later point this can be change to also allow vertical switches, but that might be kinda weird
        let bounds = cx.bounds();

        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.width();
        let mut h = bounds.h;

        if h > w {
            h = w;
        }
        let opacity = cx.opacity();
        let mut bg_col = cx.background_color();
        let mut border_col = cx.border_color();
        let mut inside_col = cx.caret_color();

        if !self.active {
            bg_col = Color::rgba(bg_col.r(), bg_col.g(), bg_col.b(), 100);
            border_col = Color::rgba(border_col.r(), border_col.g(), border_col.b(), 100);
            inside_col = Color::rgba(inside_col.r(), inside_col.g(), inside_col.b(), 80);
        }

        let mut path = Path::new();
        // bg_col.into()
        let mut paint = Paint::default();
        paint.set_color(bg_col);
        paint.set_stroke_width(h);
        paint.set_stroke_cap(PaintCap::Round);
        paint.set_style(PaintStyle::Stroke);
        paint.set_anti_alias(true);

        path.move_to((x + h / 2., y + h / 2.));
        path.line_to((x + w - h / 2., y + h / 2.));

        canvas.draw_path(&path, &paint);

        // Place the circle based on the value
        let mut paint = Paint::default();
        paint.set_color(border_col);
        paint.set_style(PaintStyle::Fill);
        paint.set_anti_alias(true);

        path = Path::new();

        let center_x = if !self.val {
            x + h / 2.
        } else {
            x + w - h / 2.
        };
        path.add_circle((center_x, y + h / 2.), h / 2., None);

        canvas.draw_path(&path, &paint);

        let mut paint = Paint::default();
        paint.set_color(inside_col);
        paint.set_anti_alias(true);

        path = Path::new();
        path.add_circle((center_x, y + h / 2.), h / 2. - cx.border_width(), None);

        canvas.draw_path(&path, &paint);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            ParamSwitchVisualEvent::SetValue(val) => {
                if *val < 0.5 {
                    self.val = false;
                    cx.needs_redraw();
                } else {
                    self.val = true;
                    cx.needs_redraw();
                }
            }
            ParamSwitchVisualEvent::SetActive(active) => {
                self.active = *active;
                cx.needs_redraw();
            }
        })
    }
}

pub trait SwitchVisualExt {
    fn value<L: Lens<Target = f32>>(self, lens: L) -> Self;
    fn active<L: Lens<Target = bool>>(self, lens: L) -> Self;
}

impl SwitchVisualExt for Handle<'_, ParamSwitchVisual> {
    fn value<L: Lens<Target = f32>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, ParamSwitchVisualEvent::SetValue(value));
        });

        self
    }

    fn active<L: Lens<Target = bool>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, ParamSwitchVisualEvent::SetActive(value));
        });

        self
    }
}
