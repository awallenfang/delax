use vizia_plug::vizia::{
    prelude::*,
    vg::{self, Paint, Path, Rect},
};

#[derive(Lens)]
pub struct PeakMeter {}

impl PeakMeter {
    pub fn new<L>(cx: &mut Context, val: L) -> Handle<Self>
    where
        L: Lens<Target = f32> + Clone,
    {
        Self {}.build(cx, |cx| {
            PeakMeterBar::new(cx, val.clone());
            Label::new(cx, val.get(cx)).overflow(Overflow::Visible);
        })
    }
}

impl View for PeakMeter {
    fn element(&self) -> Option<&'static str> {
        Some("peak-meter")
    }
}

struct PeakMeterBar<L>
where
    L: Lens<Target = f32> + Clone,
{
    val: L,
}

impl<L> PeakMeterBar<L>
where
    L: Lens<Target = f32> + Clone,
{
    pub fn new(cx: &mut Context, val: L) -> Handle<Self> {
        Self { val }.build(cx, |cx| {}).bind(val, |mut handle, _| {
            handle.needs_redraw();
        })
    }
}

impl<L> View for PeakMeterBar<L>
where
    L: Lens<Target = f32> + Clone,
{
    fn element(&self) -> Option<&'static str> {
        Some("peak-meter-bar")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &vg::Canvas) {
        let val = self.val.get(cx);
        let bounds = cx.bounds();

        if bounds.w <= f32::EPSILON || bounds.h <= f32::EPSILON {
            return;
        }

        let width = bounds.w;
        let height = bounds.h;

        let mut paint = Paint::default();
        paint.set_color(Color::red());
        paint.set_style(vg::PaintStyle::Fill);

        let mut path = Path::new();
        let rect = Rect::new(
            bounds.x,
            bounds.y + (1. - val) * height,
            bounds.x + width,
            bounds.y + height,
        );
        path.add_rect(rect, None);

        canvas.draw_path(&path, &paint);
    }
}
