use crate::ui::{shader_utils::make_shader, DelaxEvent};
use nih_plug::{nih_dbg, params::Param};
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
const SHADER: &'static str = include_str!("shaders/background.sksl");

/// A switch to control a boolean nih-plug parameter
pub struct Background {
    time: f32,
    shader: Option<Shader>,
}

impl Background {
    pub fn new(cx: &mut Context) -> Handle<Self>
where {
        let mut shader = None;
        let effect = make_shader(SHADER);
        if let Ok(runtime) = effect {
            let builder = RuntimeShaderBuilder::new(runtime);
            shader = builder.make_shader(&Matrix::new_identity());
        } else if let Err(error) = effect {
            nih_dbg!(error);
        }
        Self {
            time: 0.,
            shader,
        }
        .build(cx, |_| {})
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

        if let Some(shader) = &self.shader {
            let mut shader_to_device = Matrix::translate((bounds.x, bounds.y));
            shader_to_device = *shader_to_device.pre_scale((bounds.w, bounds.h), None);

            let local_matrix = shader_to_device;
            let s = shader.with_local_matrix(&local_matrix);
            paint.set_shader(s);
        }
        canvas.draw_path(&path, &paint);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|visual_event, _| match visual_event {
            DelaxEvent::ShaderTick(delta) => {
                self.time += delta.as_secs_f32();
                nih_dbg!("Tick");
                cx.needs_redraw();
            },
            _ => ()
        });
    }
}
