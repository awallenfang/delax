use nih_plug::{nih_dbg, params::Param};
use vizia_plug::{
    vizia::{
        prelude::*,
        vg::{runtime_effect::RuntimeShaderBuilder, Canvas, Matrix, Paint, PaintCap, PaintStyle, Path, Point, RCHandle, Rect, RuntimeEffect, Shader},
    },
    widgets::param_base::ParamWidgetBase,
};
use crate::ui::shader_utils::make_shader;
const SHADER: &'static str = include_str!("shaders/background.sksl");

/// A switch to control a boolean nih-plug parameter
pub struct Background {
    time: f32,
}

impl Background {
    pub fn new(
        cx: &mut Context,
    ) -> Handle<Self>
    where
    {
        Self {time: 0. }.build(cx, |_| {})
        
    }

}

impl View for Background {
     fn element(&self) -> Option<&'static str> {
        Some("background")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let bounds = cx.bounds();

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
        paint.set_style(PaintStyle::Fill);

        let effect = make_shader(SHADER);
        if let Ok(runtime) = effect {
            let builder = RuntimeShaderBuilder::new(runtime);
            let mut shader_to_device = Matrix::translate((bounds.x, bounds.y));
            shader_to_device = *shader_to_device.pre_scale((bounds.w, bounds.h), None);

            let local_matrix = shader_to_device;
            let shader = builder.make_shader(&local_matrix);
            paint.set_shader(shader);
        } else if let Err(error) = effect {
            nih_dbg!(error);
        }

        canvas.draw_path(&path, &paint);
        // Force redraw for time update
        cx.needs_redraw();
    }
}

