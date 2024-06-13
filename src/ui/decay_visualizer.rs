use nih_plug::params::Param;
use nih_plug_vizia::{
    vizia::{
        binding::Lens,
        context::{Context, DrawContext},
        vg::{Color, LineCap, Paint, Path},
        view::{Canvas, Handle, View},
    },
    widgets::param_base::ParamWidgetBase,
};
pub struct DecayVisualizer<L: Lens<Target = f32>> {
    delay_l_lens: Option<L>,
    delay_r_lens: Option<L>,
    feedback_l_lens: Option<L>,
    feedback_r_lens: Option<L>,
    // param_base: ParamWidgetBase
}

impl<L: Lens<Target = f32>> DecayVisualizer<L> {
    pub fn new(cx: &mut Context) -> Handle<Self> {
        Self {
            delay_l_lens: None,
            delay_r_lens: None,
            feedback_l_lens: None,
            feedback_r_lens: None,
        }
        .build(cx, |_| {})
    }
}
impl<L> View for DecayVisualizer<L>
where
    L: Lens<Target = f32> + Clone,
{
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(0.0, 1.0);
        path.line_to(1.0, 1.0);
        path.line_to(1.0, 0.0);
        path.close();
        let mut paint = Paint::color(Color::white());
        paint.set_line_cap(LineCap::Round);
        paint.set_line_width(0.1);
        canvas.stroke_path(&path, &paint);
    }
}
