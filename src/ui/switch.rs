use nih_plug::params::Param;
use nih_plug_vizia::{
    vizia::{
        prelude::*,
        vg::{LineCap, Paint, Path},
    },
    widgets::param_base::ParamWidgetBase,
};

pub struct ParamSwitch {
    param_base: ParamWidgetBase,
    val: bool,
}

impl ParamSwitch {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        default_val: bool,
    ) -> Handle<Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            val: default_val,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let param_lens = param_data.make_lens(|param| param.unmodulated_normalized_value());

                ParamSwitchVisual::new(cx, default_val)
                    .value(param_lens)
                    .class("switch-visual")
                    .height(Stretch(1.))
                    .width(Stretch(1.));
            }),
        )
    }

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
        event.map(|input_event, _| match input_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                self.toggle(cx);
            }
            _ => (),
        })
    }
}

pub enum ParamSwitchVisualEvent {
    SetValue(f32),
}
struct ParamSwitchVisual {
    val: bool,
}

impl ParamSwitchVisual {
    pub fn new(cx: &mut Context, default_val: bool) -> Handle<Self> {
        Self { val: default_val }.build(cx, |_| {})
    }
}

impl View for ParamSwitchVisual {
    fn element(&self) -> Option<&'static str> {
        Some("switch-visual")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // Grab all the bounds
        // This assumes a horizontal alignment
        let bounds = cx.bounds();

        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.w;
        let mut h = bounds.h;

        if h > w {
            h = w;
        }

        let bg_col = cx.background_color();
        let border_col = cx.border_color();
        let inside_col = cx.caret_color();

        let mut path = Path::new();
        let paint = Paint::color(bg_col.into())
            .with_line_cap(LineCap::Round)
            .with_line_width(h);

        path.move_to(x + h / 2., y + h / 2.);
        path.line_to(x + w - h / 2., y + h / 2.);

        canvas.stroke_path(&path, &paint);

        path = Path::new();
        let paint = Paint::color(border_col.into());
        let center_x = if !self.val {
            x + h / 2.
        } else {
            x + w - h / 2.
        };
        path.circle(center_x, y + h / 2., h / 2.);

        canvas.fill_path(&path, &paint);

        path = Path::new();
        let paint = Paint::color(inside_col.into());

        path.circle(center_x, y + h / 2., h / 2. - cx.border_width());

        canvas.fill_path(&path, &paint);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            ParamSwitchVisualEvent::SetValue(val) => {
                if *val < 0.5 {
                    self.val = false;
                } else {
                    self.val = true;
                }
            }
        })
    }
}

pub trait SwitchVisualExt {
    fn value<L: Lens<Target = f32>>(self, lens: L) -> Self;
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
}
