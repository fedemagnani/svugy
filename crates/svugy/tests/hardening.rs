//! Render models at the edges validation admits and check the output stays sound.

use svugy::{RenderSvg, StaticSvg};
use svugy_model::builder::ModelBuilder;
use svugy_model::model::{Element, MAX_MAGNITUDE, Position, Shape};

const MAX_BOUNDED_DOCUMENT_BYTES: usize = 200_000;

#[test]
fn a_pin_at_the_magnitude_bound_renders_a_small_parseable_document() {
    let mut builder = ModelBuilder::new();
    let far_position = Position {
        x: MAX_MAGNITUDE,
        y: MAX_MAGNITUDE,
    };
    let far = Element::new("far")
        .with_position(far_position)
        .with_style("stroke-dash", "1000000");
    builder.add_top_level("far", far);
    builder.add_top_level("near", Element::new("near"));
    let model = builder.build().model.expect("a pin at the bound is valid");

    let markup = StaticSvg::render(&model);

    let byte_count = markup.len();
    assert!(
        byte_count < MAX_BOUNDED_DOCUMENT_BYTES,
        "{byte_count} bytes"
    );
    let options = usvg::Options::default();
    let parse_result = usvg::Tree::from_str(&markup, &options);
    assert!(parse_result.is_ok(), "{parse_result:?}");
}

/// Render container `pond` holding `fish` and `frog`, shaped as `shape` when given.
fn render_pond(shape: Option<Shape>) -> String {
    let mut builder = ModelBuilder::new();
    let unshaped_pond = Element::new("pond");
    let pond_element = match shape {
        Some(shape) => unshaped_pond.with_shape(shape),
        None => unshaped_pond,
    };
    let pond = builder.add_top_level("pond", pond_element);
    builder.add_child(&pond, "fish", Element::new("fish"));
    builder.add_child(&pond, "frog", Element::new("frog"));
    let model = builder
        .build()
        .model
        .expect("an ignored shape is only a warning");
    StaticSvg::render(&model)
}

#[test]
fn a_container_of_any_shape_renders_exactly_as_the_slab_it_defaults_to() {
    let slab_markup = render_pond(None);
    for shape in [Shape::Box, Shape::Slab, Shape::Cylinder, Shape::Disc] {
        let shaped_markup = render_pond(Some(shape));
        assert_eq!(shaped_markup, slab_markup, "{shape:?}");
    }
}
