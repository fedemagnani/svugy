//! Render models at the edges validation admits and check the output stays sound.

use svugy::{RenderSvg, StaticSvg};
use svugy_model::builder::{ModelBuilder, StepRef, WorkflowRef};
use svugy_model::model::{Element, MAX_MAGNITUDE, Position, Shape};
use svugy_model::workflow::{ElementOverride, Step, StepId, Workflow, WorkflowId};

const MAX_BOUNDED_DOCUMENT_BYTES: usize = 200_000;

#[test]
fn a_pin_at_the_magnitude_bound_renders_a_small_parseable_document() {
    let mut builder = ModelBuilder::new();
    let far_position = Position {
        x: MAX_MAGNITUDE,
        y: MAX_MAGNITUDE,
    };
    let far_element = Element::new("far").with_style("stroke-dash", "1000000");
    let far = builder.add_top_level("far", far_element);
    let _near = builder.add_top_level("near", Element::new("near"));
    let (main, every_step) = add_main_step(&mut builder);
    builder.list_every_element(&every_step);
    let far_pinning = ElementOverride::new().with_position(far_position);
    builder.override_element(&main, &far, far_pinning);
    let model = builder.build().model.expect("a pin at the bound is valid");

    let markup = StaticSvg::render(model.showing_first_workflow());

    let byte_count = markup.len();
    assert!(
        byte_count < MAX_BOUNDED_DOCUMENT_BYTES,
        "{byte_count} bytes"
    );
    let options = usvg::Options::default();
    let parse_result = usvg::Tree::from_str(&markup, &options);
    assert!(parse_result.is_ok(), "{parse_result:?}");
}

#[test]
fn a_container_of_any_shape_renders_exactly_as_the_slab_it_defaults_to() {
    let slab_markup = render_pond(None);
    for shape in [Shape::Box, Shape::Slab, Shape::Cylinder, Shape::Disc] {
        let shaped_markup = render_pond(Some(shape));
        assert_eq!(shaped_markup, slab_markup, "{shape:?}");
    }
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
    let _fish = builder.add_child(&pond, "fish", Element::new("fish"));
    let _frog = builder.add_child(&pond, "frog", Element::new("frog"));
    let (_main, every_step) = add_main_step(&mut builder);
    builder.list_every_element(&every_step);
    let model = builder
        .build()
        .model
        .expect("the model is valid, its warnings aside");
    StaticSvg::render(model.showing_first_workflow())
}

/// Add workflow `main` holding the one step `main`.
fn add_main_step(builder: &mut ModelBuilder) -> (WorkflowRef, StepRef) {
    let main_id = WorkflowId::new("main");
    let main_workflow = Workflow::new(main_id, "main");
    let main = builder.add_workflow(main_workflow);
    let main_step_id = StepId::new("main");
    let main_step = Step::new(main_step_id, "main");
    let step = builder.add_step(&main, main_step);
    (main, step)
}
