use std::{thread, time::Duration};

use nih_plug::prelude::AtomicF32;
use std::sync::atomic::Ordering;
use std::time::Instant;
use vizia_plug::vizia::vg::{RuntimeEffect, runtime_effect::Options};

// Your global atomic time value
pub static TIME: AtomicF32 = AtomicF32::new(0.0);

pub fn spawn_time_thread() {
    thread::spawn(|| {
        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs_f32();
            TIME.store(elapsed, Ordering::SeqCst);
            // Optional: sleep a bit to avoid busy-looping
            std::thread::sleep(Duration::from_millis(1));
        }
    });
}

pub fn make_effect(sksl: &str) -> Result<RuntimeEffect, String> {
    struct NoneOpts;

    impl<'a, 'b> From<NoneOpts> for Option<&'a Options<'b>> {
        fn from(val: NoneOpts) -> Self {
            None
        }
    }

    RuntimeEffect::make_for_shader(sksl, NoneOpts)
}
