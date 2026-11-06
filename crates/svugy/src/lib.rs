//! Render a svugy model as an isometric, explorable SVG.

mod geometry;
mod layout;
mod metrics;
mod render;
#[cfg(test)]
#[path = "../tests/golden/svugy.rs"]
mod svugy_example;

use svugy_model::model::Model;

use crate::geometry::Scene;
use crate::layout::LevelPlans;
use crate::metrics::BundledFont;
use crate::render::{InteractiveRenderer, Profile, Renderer, StaticRenderer};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime.
pub struct InteractiveSvg;

/// Renders a model as a static SVG: self-contained, for viewers without CSS, script or fonts.
pub struct StaticSvg;

impl InteractiveSvg {
    pub fn render(model: &Model) -> String {
        InteractiveRenderer::render_model(model)
    }
}

impl StaticSvg {
    pub fn render(model: &Model) -> String {
        StaticRenderer::render_model(model)
    }
}

impl<P: Profile + Default> Renderer<'_, '_, P> {
    /// Render `model` as a document in profile `P`: lay it out and project it in the bundled font, then write the scene.
    fn render_model(model: &Model) -> String {
        let font = BundledFont::new();
        let plans = LevelPlans::lay_out(model, &font);
        let scene = Scene::project(model, &plans, &font);
        let renderer = Renderer::<P>::new(model, &font);
        renderer.write_document(&scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svugy_example::SvugyExample;
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};
    use svugy_model::path::ElementPath;

    use crate::geometry::{Badge, Connection};

    /// Collect the connections of every level of `scene`, the root's first, then each interior's in paint order.
    fn connections_of_every_level(scene: &Scene) -> Vec<&Connection> {
        let root_connections = scene.root.connections.iter();
        let nested_connections = scene
            .root
            .nested_interiors()
            .flat_map(|interior| &interior.group.connections);
        root_connections.chain(nested_connections).collect()
    }

    fn rasterize(svg: &str) -> Vec<u8> {
        let options = usvg::Options::default();
        let tree = usvg::Tree::from_str(svg, &options).expect("svg parses");
        let size = tree.size().to_int_size();
        let mut pixmap =
            resvg::tiny_skia::Pixmap::new(size.width(), size.height()).expect("non-empty size");
        let identity = resvg::tiny_skia::Transform::default();
        let mut pixmap_view = pixmap.as_mut();
        resvg::render(&tree, identity, &mut pixmap_view);
        pixmap.data().to_vec()
    }

    /// Drop every `data-svugy-view` attribute, the only text a nested stub plate reaches in the static profile.
    fn without_level_views(svg: &str) -> String {
        let view_opening = " data-svugy-view=\"";
        let mut remaining = svg;
        let mut stripped = String::new();
        while let Some(view_start) = remaining.find(view_opening) {
            let (before_view, from_view) = remaining.split_at(view_start);
            stripped.push_str(before_view);
            let value_start = view_opening.len();
            let value_length = from_view[value_start..].find('"').expect("closed value");
            let view_end = value_start + value_length + 1;
            remaining = &from_view[view_end..];
        }
        stripped.push_str(remaining);
        stripped
    }

    #[test]
    fn stubs_move_no_drawn_arc_and_no_static_pixel() {
        let model = SvugyExample::model();
        let font = BundledFont::new();
        let plans = LevelPlans::lay_out(&model, &font);
        let stubless_plans = plans.clone().without_stubs();

        let scene = Scene::project(&model, &plans, &font);
        let stubless_scene = Scene::project(&model, &stubless_plans, &font);

        let stubbed_level_count = scene
            .root
            .nested_interiors()
            .filter(|interior| !interior.group.stub_markers.is_empty())
            .count();
        assert!(stubbed_level_count > 0);
        let connections = connections_of_every_level(&scene);
        let stubless_connections = connections_of_every_level(&stubless_scene);
        assert_eq!(connections, stubless_connections);
        let renderer = StaticRenderer::new(&model, &font);
        let static_svg = renderer.write_document(&scene);
        let stubless_static_svg = renderer.write_document(&stubless_scene);
        let static_svg_without_views = without_level_views(&static_svg);
        let stubless_static_svg_without_views = without_level_views(&stubless_static_svg);
        assert!(static_svg_without_views == stubless_static_svg_without_views);
        let static_raster = rasterize(&static_svg);
        let stubless_static_raster = rasterize(&stubless_static_svg);
        assert!(static_raster == stubless_static_raster);
    }

    /// Reload `model` without the edges pointing to `target`, as if they had never been written.
    fn without_edges_into(model: &Model, target: &str) -> Model {
        let saved = model.to_json();
        let mut model_value: serde_json::Value = serde_json::from_str(&saved).expect("JSON");
        let edges = model_value["edges"]
            .as_array_mut()
            .expect("a model saves its edges as an array");
        edges.retain(|edge| edge["to"] != target);
        let pruned = model_value.to_string();
        let admission = Model::load_json(&pruned).expect("well-formed JSON");
        admission.model.expect("dropping edges keeps a sound model")
    }

    /// Collect every badge of `scene`, the root's solids first, then each interior's in paint order.
    fn badges_of_every_level(scene: &Scene) -> Vec<&Badge> {
        let root_solids = scene.root.solids.iter();
        let nested_solids = scene
            .root
            .nested_interiors()
            .flat_map(|interior| &interior.group.solids);
        let solids = root_solids.chain(nested_solids);
        solids.flat_map(|solid| &solid.badges).collect()
    }

    #[test]
    fn badges_draw_no_arc_and_move_no_other_arc_than_removing_their_edges_would() {
        let model = SvugyExample::model();
        let unbadged_model = without_edges_into(&model, "pipeline.inter");
        let font = BundledFont::new();
        let plans = LevelPlans::lay_out(&model, &font);
        let unbadged_plans = LevelPlans::lay_out(&unbadged_model, &font);

        let scene = Scene::project(&model, &plans, &font);
        let unbadged_scene = Scene::project(&unbadged_model, &unbadged_plans, &font);

        let stubless_plans = plans.clone().without_stubs();
        let unbadged_stubless_plans = unbadged_plans.clone().without_stubs();
        assert_eq!(stubless_plans, unbadged_stubless_plans);
        let connections = connections_of_every_level(&scene);
        let unbadged_connections = connections_of_every_level(&unbadged_scene);
        assert_eq!(connections, unbadged_connections);
        let badge_edges: Vec<String> = badges_of_every_level(&scene)
            .iter()
            .map(|badge| badge.edge.to_string())
            .collect();
        let badge_edge_count = model.badge_edges().count();
        assert_eq!(badge_edges.len(), badge_edge_count);
        assert_eq!(badge_edge_count, 2);
        assert!(badges_of_every_level(&unbadged_scene).is_empty());
    }

    /// Chain `a -> b -> c` unpinned beside the badge target `store`, written to from `a` and `c` unless `is_written`.
    fn layered_model_writing_to_a_badge(is_written: bool) -> Model {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("bus"));
        let c = builder.add_top_level("c", Element::new("crank"));
        let store = builder.add_top_level("store", Element::new("store").with_badge());
        builder.add_edge(&a, &b, Edge::new());
        builder.add_edge(&b, &c, Edge::new().with_label("calls"));
        if is_written {
            builder.add_edge(&a, &store, Edge::new());
            builder.add_edge(&c, &store, Edge::new());
        }
        builder.build().model.expect("fixture model is valid")
    }

    #[test]
    fn badge_edges_take_no_part_in_automatic_placement() {
        let font = BundledFont::new();
        let badged_model = layered_model_writing_to_a_badge(true);
        let unwritten_model = layered_model_writing_to_a_badge(false);

        let badged_plans = LevelPlans::lay_out(&badged_model, &font);
        let unwritten_plans = LevelPlans::lay_out(&unwritten_model, &font);

        assert_eq!(badged_plans, unwritten_plans);
        let badged_scene = Scene::project(&badged_model, &badged_plans, &font);
        let unwritten_scene = Scene::project(&unwritten_model, &unwritten_plans, &font);
        let badged_connections = connections_of_every_level(&badged_scene);
        let unwritten_connections = connections_of_every_level(&unwritten_scene);
        assert_eq!(badged_connections, unwritten_connections);
        assert_eq!(badges_of_every_level(&badged_scene).len(), 2);
    }

    #[test]
    fn a_cross_container_badge_is_seated_on_its_nested_source_and_stubbed_in_its_container() {
        let mut builder = ModelBuilder::new();
        let desk = builder.add_top_level("desk", Element::new("desk"));
        let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
        let store = builder.add_top_level("store", Element::new("store").with_badge());
        let writes = builder.add_edge(&quoter, &store, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");
        let font = BundledFont::new();

        let plans = LevelPlans::lay_out(&model, &font);
        let scene = Scene::project(&model, &plans, &font);

        let root_plan = plans
            .level(&ElementPath::root())
            .expect("the root is planned");
        let desk_plan = plans.level(desk.path()).expect("desk is planned");
        assert!(root_plan.routes.is_empty());
        let store_stub = &desk_plan.stubs[store.path()];
        assert!(store_stub.edges.contains(&writes));
        assert!(connections_of_every_level(&scene).is_empty());
        let badges = badges_of_every_level(&scene);
        let [badge] = badges[..] else {
            panic!("one badge: {badges:?}");
        };
        assert_eq!(badge.edge, writes);
        let desk_interior = scene
            .root
            .nested_interiors()
            .next()
            .expect("desk has an interior");
        let quoter_solid = &desk_interior.group.solids[0];
        assert_eq!(&quoter_solid.element, quoter.path());
        assert_eq!(quoter_solid.badges.len(), 1);
        let stub_marker = &desk_interior.group.stub_markers[store.path()];
        assert!(stub_marker.edges.contains(&writes));
    }
}
