//! Print the bytes that must be identical on linux x86_64, macOS aarch64 and wasm32-wasip1: the golden's static render.

#[path = "../tests/golden/svugy.rs"]
mod svugy_example;

use svugy::StaticSvg;
use svugy_example::SvugyExample;

fn main() {
    let model = SvugyExample::model();
    let static_svg = StaticSvg::render(&model);
    print!("{static_svg}");
}
