use std::time;

use crate::ui::{shaders::{self, make_effect}, DelaxEvent};
use nih_plug::{nih_dbg, params::Param, prelude::*};
use vizia_plug::{
    vizia::{
        prelude::*,
        vg::{
            Canvas, Matrix, Paint, PaintCap, PaintStyle, Path, Point, RCHandle, Rect,
            RuntimeEffect, Shader, runtime_effect::RuntimeShaderBuilder, wrapper::PointerWrapper,
        },
    },
    widgets::param_base::ParamWidgetBase,
};
use shaders::TIME;

const SHADER: &'static str = include_str!("shaders/background.sksl");

/// A switch to control a boolean nih-plug parameter
pub struct Background {
    time: f32,
    // shader: Option<Shader>,
    effect: Option<RuntimeEffect>
}

impl Background {
    pub fn new(cx: &mut Context) -> Handle<Self> {
        // let mut shader = None;
        let result = make_effect(SHADER);
        let mut effect = None;

        if let Ok(runtime) = result {
            effect = Some(runtime);
        } else if let Err(error) = result {
            nih_dbg!(error);
        }
        Self { time: 0., /*shader,*/ effect }.build(cx, |_| {})
    }
}

impl View for Background {
    fn element(&self) -> Option<&'static str> {
        Some("background")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let bounds = cx.bounds();
        let mut path = Path::new();
        let rect = Rect::new(bounds.x, bounds.y, bounds.x + bounds.w, bounds.y + bounds.h);
        path.add_rect(rect, None);
        let mut paint = Paint::default();
        paint.set_style(PaintStyle::Fill);

        if let Some(effect) = &self.effect {
            let mut shader_to_device = Matrix::translate((bounds.x, bounds.y));
            shader_to_device = *shader_to_device.pre_scale((bounds.w, bounds.h), None);

            let local_matrix = shader_to_device;
            let mut builder = RuntimeShaderBuilder::new(effect.clone());
            let _ = builder.set_uniform_float("time", &[TIME.load(std::sync::atomic::Ordering::Relaxed)]);
            let shader = builder.make_shader(&local_matrix);
            if let Some(s) = shader {
                paint.set_shader(s);
            }
        }
        canvas.draw_path(&path, &paint);
        cx.needs_redraw();
    }

}
