use std::{f32::consts::PI, sync::Arc};

use nih_plug::{nih_dbg, prelude::Param};
use nih_plug_vizia::{
    vizia::{
        prelude::*,
        vg::{LineCap, Paint, Path, Solidity},
    },
    widgets::param_base::ParamWidgetBase,
};


#[derive(Lens)]
pub struct PeakMeter {
}


impl PeakMeter {
    pub fn new<L> (
        cx: &mut Context,
        val: L,
    ) -> Handle<Self>
    where
        L: Lens<Target = f32> + Clone,
    {
        Self {
        }
        .build(cx ,|cx| {
            PeakMeterBar::new(cx, val.clone());
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
    L: Lens<Target = f32> + Clone,{
    val: L,
}

impl<L> PeakMeterBar<L>
where
    L: Lens<Target = f32> + Clone,
{
    pub fn new(
        cx: &mut Context,
        val: L,
    ) -> Handle<Self> {
        Self {
            val,
        }
        .build(cx, |cx| {})
    }
}

impl<L> View for PeakMeterBar<L>
where
    L: Lens<Target = f32> + Clone, {
        fn element(&self) -> Option<&'static str> {
            Some("peak-meter-bar")
        }

        fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
            let val = self.val.get(cx);
            let bounds = cx.bounds();

            if bounds.w <= f32::EPSILON || bounds.h <= f32::EPSILON {
                return;
            }

            let width = bounds.w;
            let height = bounds.h;

            let mut paint = Paint::color(Color::red().into());

            let mut path = Path::new();
            path.rect(bounds.x, bounds.y + height / 2., width, height * val);

            canvas.fill_path(&path, &paint);
        }
}