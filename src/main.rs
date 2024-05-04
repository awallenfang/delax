use delax::Delax;
use nih_plug::prelude::*;

pub fn main() {
    nih_export_standalone::<Delax>();
    println!("Currently no standalone active!")
}
