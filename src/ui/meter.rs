use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

use vizia_plug::vizia::{
    prelude::*,
    vg::{self, Paint, Path, Rect, Shader},
};

use crate::peak_follower::PeakFollower;

#[derive(Lens)]
pub struct PeakMeter {}

impl PeakMeter {
    pub fn new<L>(cx: &mut Context, val: L) -> Handle<Self>
    where
        L: Lens<Target = f32> + Clone,
    {
        let peak_follower = Mutex::new(PeakFollower::new(0.1, 0., 0.03));

        Self {}.build(cx, |cx| {
            let followed_peak = val.map(move |level| -> f32 {
                if let Ok(mut fol) = peak_follower.try_lock() {
                    fol.process(*level)
                } else {
                    *level
                }
            });
            PeakMeterBar::new(cx, followed_peak).bind(followed_peak, |mut handle, _| {
                handle.needs_redraw();
            });
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
        Self { val }.build(cx, |cx| {})
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

        // Base region
        if val < 0.01 {
            return;
        }
        let width = bounds.w;
        let height = bounds.h;

        let base_color = cx.background_color();
        let base_rect = Rect::new(
            bounds.x,
            bounds.y + (1. - val) * height,
            bounds.x + width,
            bounds.y + height,
        );
        let mut paint = Paint::default();
        paint.set_color(base_color);
        paint.set_style(vg::PaintStyle::Fill);

        let mut path = Path::new();

        path.add_rect(base_rect, None);

        canvas.draw_path(&path, &paint);

        if val < 0.85 {
            return;
        }
        let loud_color = cx.selection_color();
        let loud_rect = Rect::new(
            bounds.x,
            bounds.y + (1. - val) * height,
            bounds.x + width,
            bounds.y + height * 0.15,
        );
        let mut paint = Paint::default();
        paint.set_color(loud_color);
        paint.set_style(vg::PaintStyle::Fill);

        let mut path = Path::new();

        path.add_rect(loud_rect, None);

        canvas.draw_path(&path, &paint);
        let peak_color = cx.caret_color();
    }
}
