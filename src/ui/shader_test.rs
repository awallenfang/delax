use nih_plug::params::Param;
use vizia_plug::{
    vizia::{
        prelude::*,
        vg::{image_filters::runtime_shader, runtime_effect::{Options, RuntimeShaderBuilder, Uniform}, Canvas, Matrix, Paint, PaintCap, PaintStyle, Path, Point, Rect, RuntimeEffect},
    },
    widgets::param_base::ParamWidgetBase,
};

const SHADER: &'static str = include_str!("shaders/test.sksl");

/// A switch to control a boolean nih-plug parameter
pub struct ShaderSwitch {
    param_base: ParamWidgetBase,
}

impl ShaderSwitch {
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
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let param_lens = param_data.make_lens(|param| param.unmodulated_normalized_value());

                // Simply create a visual instance on the lens
                ShaderSwitchVisual::new(cx, default_val)
                    .value(param_lens)
                    .class("shader-switch-visual")
                    .height(Stretch(1.))
                    .width(Stretch(1.));
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

impl View for ShaderSwitch {
    fn element(&self) -> Option<&'static str> {
        Some("shader-switch")
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

pub enum ShaderSwitchVisualEvent {
    SetValue(f32),
}
struct ShaderSwitchVisual {
    val: bool,
}

impl ShaderSwitchVisual {
    pub fn new(cx: &mut Context, default_val: bool) -> Handle<Self> {
        Self { val: default_val }.build(cx, |_| {})
    }
}

impl View for ShaderSwitchVisual {
    fn element(&self) -> Option<&'static str> {
        Some("shader-switch-visual")
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
        let bg_col = cx.background_color();
        let border_col = cx.border_color();
        let inside_col = cx.caret_color();

        let mut path = Path::new();
        let rect = Rect::new(
                bounds.x,
                bounds.y,
                bounds.x + bounds.w,
                bounds.y +bounds.h,
            );
        path.add_rect(rect, None);
        let mut paint = Paint::default();
        let effect = RuntimeEffect::make_for_shader(SHADER, None);
        if let Ok(runtime) = effect {
            let builder = RuntimeShaderBuilder::new(runtime);
            let shader = builder.make_shader(&Matrix::new_identity());
            paint.set_shader(shader);
        }
        // // bg_col.into()
        // paint.set_color(bg_col);
        // paint.set_stroke_width(h);
        // paint.set_stroke_cap(PaintCap::Round);
        // paint.set_style(PaintStyle::Stroke);
        // paint.set_anti_alias(true);

        // path.move_to((x + h / 2., y + h / 2.));
        // path.line_to((x + w - h / 2., y + h / 2.));

        // canvas.draw_path(&path, &paint);

        // // Place the circle based on the value
        // let mut paint = Paint::default();
        // paint.set_color(border_col);
        // paint.set_style(PaintStyle::Fill);
        // paint.set_anti_alias(true);

        // path = Path::new();

        // let center_x = if !self.val {
        //     x + h / 2.
        // } else {
        //     x + w - h / 2.
        // };
        // path.add_circle((center_x, y + h / 2.), h / 2., None);

        // canvas.draw_path(&path, &paint);

        // let mut paint = Paint::default();
        // paint.set_color(inside_col);
        // paint.set_anti_alias(true);

        // path = Path::new();
        // path.add_circle((center_x, y + h / 2.), h / 2. - cx.border_width(), None);

        // canvas.draw_path(&path, &paint);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            ShaderSwitchVisualEvent::SetValue(val) => {
                if *val < 0.5 {
                    self.val = false;
                    cx.needs_redraw();
                } else {
                    self.val = true;
                    cx.needs_redraw();
                }
            }
        })
    }
}

pub trait SwitchVisualExt {
    fn value<L: Lens<Target = f32>>(self, lens: L) -> Self;
}

impl SwitchVisualExt for Handle<'_, ShaderSwitchVisual> {
    fn value<L: Lens<Target = f32>>(mut self, lens: L) -> Self {
        let entity = self.entity();
        Binding::new(self.context(), lens, move |cx, val| {
            let value = val.get(cx);
            cx.emit_to(entity, ShaderSwitchVisualEvent::SetValue(value));
        });

        self
    }
}
