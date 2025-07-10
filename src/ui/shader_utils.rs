use vizia_plug::vizia::vg::{runtime_effect::Options, RuntimeEffect};

pub fn make_shader(sksl: &str) -> Result<RuntimeEffect, String> {
    struct NoneOpts;

    impl<'a, 'b> Into<Option<&'a Options<'b>>> for NoneOpts {
        fn into(self) -> Option<&'a Options<'b>> {
            None
        }
    }

    RuntimeEffect::make_for_shader(sksl, NoneOpts)
}