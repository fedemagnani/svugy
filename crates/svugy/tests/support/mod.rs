//! Rasterize SVGs and compare their rasters against pixel goldens.

/// Rasterize an SVG to PNG bytes with no fonts loaded, so a stray `<text>` vanishes from the raster.
pub fn rasterize_to_png(svg: &str) -> Vec<u8> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).expect("svg parses");
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).expect("non-empty size");
    let mut pixmap_view = pixmap.as_mut();
    let identity = resvg::tiny_skia::Transform::default();
    resvg::render(&tree, identity, &mut pixmap_view);
    pixmap.encode_png().expect("pixmap encodes as png")
}

/// Define a test comparing an SVG's raster to its golden, ignored with a reason off linux x86_64.
// Regenerate goldens elsewhere: unzip the failing CI check job's `pending-pixel-snapshots` artifact into tests/snapshots, then `cargo insta accept`.
macro_rules! pixel_snapshot_test {
    ($test_name:ident, $svg:expr) => {
        #[test]
        #[cfg_attr(
            not(all(target_os = "linux", target_arch = "x86_64")),
            ignore = "pixel goldens compare only on linux x86_64"
        )]
        fn $test_name() {
            let svg: &str = &$svg;
            let png_bytes = $crate::support::rasterize_to_png(svg);
            let snapshot_name_and_extension = concat!(stringify!($test_name), ".png");
            insta::assert_binary_snapshot!(snapshot_name_and_extension, png_bytes);
        }
    };
}

pub(crate) use pixel_snapshot_test;
