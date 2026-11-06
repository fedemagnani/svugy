//! Describe svugy's own architecture as a model, authored through the builder top level first.

use svugy_model::builder::ModelBuilder;
use svugy_model::legend::Legend;
use svugy_model::model::{Edge, Element, Model, Position, Shape};
use svugy_model::path::ElementPath;

const IMPORT_STAGE_FILL: &str = "#f0a020";
const MODEL_MODULE_FILL: &str = "#14f195";
const PIPELINE_STAGE_FILL: &str = "#1f3566";
const PIPELINE_MODULE_FILL: &str = "#3478f6";
const STORE_FILL: &str = "#d5dcea";
const NEUTRAL_FILL: &str = "#3a3f4b";
const EMBEDDED_DASH: &str = "2";
const STORY_STEPS: [&str; 4] = [
    "① your text becomes a model",
    "② the model becomes boxes in plan space",
    "③ boxes become isometric shapes",
    "④ shapes become one SVG file, runtime inside",
];

/// Golden source whose renders the snapshot tests pin: svugy drawn by svugy.
pub struct SvugyExample;

impl SvugyExample {
    /// Build the golden model, each element pinned at its footprint's back corner.
    pub fn model() -> Model {
        let pipeline_inter_path = ElementPath::from_segments(["pipeline", "inter"]);
        let unstepped_legend = Legend::new()
            .with_title("Read it as a story")
            .with_dash_description(Edge::DEFAULT_STROKE_DASH, "data flows this way")
            .with_dash_description(2.0, "embedded at build time")
            .with_badge_description(pipeline_inter_path, "reads the bundled font");
        let legend = STORY_STEPS
            .into_iter()
            .fold(unstepped_legend, Legend::with_entry);
        let mut builder = ModelBuilder::new()
            .with_title("svugy · how it is built")
            .with_subtitle("text in → model → layout → geometry → SVG out")
            .with_legend(legend);

        let text_element = Element::new("your text")
            .with_caption("d2 · JSON · Rust builder")
            .with_style("fill", NEUTRAL_FILL)
            .with_position(Position { x: 0.0, y: 40.0 });
        let text = builder.add_top_level("text", text_element);

        let cli_element = Element::new("svugy-cli")
            .with_caption("render · validate · import")
            .with_height(70.0)
            .with_style("fill", "#9945ff")
            .with_link("pipeline")
            .with_position(Position { x: 0.0, y: 330.0 });
        let cli = builder.add_top_level("cli", cli_element);

        let d2_element = Element::new("d2 v0.9.0")
            .with_caption("reference grammar")
            .with_style("fill", "transparent")
            .with_style("stroke", "#5b6170")
            .with_style("stroke-dash", "5")
            .with_link("https://d2lang.com")
            .with_position(Position {
                x: 260.0,
                y: -220.0,
            });
        let d2 = builder.add_top_level("d2", d2_element);

        let import_element = Element::new("svugy-import")
            .with_caption("d2 → model")
            .with_style("fill", "#2a2418")
            .with_position(Position { x: 260.0, y: 0.0 });
        let import = builder.add_top_level("import", import_element);

        let model_element = Element::new("svugy-model")
            .with_caption("the hub")
            .with_style("fill", "#17271f")
            .with_position(Position { x: 260.0, y: 330.0 });
        let model = builder.add_top_level("model", model_element);

        let pipeline_element = Element::new("svugy")
            .with_caption("the pipeline")
            .with_style("fill", "#1b2436")
            .with_position(Position { x: 760.0, y: 0.0 });
        let pipeline = builder.add_top_level("pipeline", pipeline_element);

        let contract_element = Element::new("runtime contract")
            .with_caption("data-svugy-*")
            .with_shape(Shape::Slab)
            .with_style("fill", "#334566")
            .with_position(Position {
                x: 1560.0,
                y: 760.0,
            });
        let contract = builder.add_top_level("contract", contract_element);

        let runtime_element = Element::new("runtime")
            .with_caption("svugy.js · svugy.css")
            .with_style("fill", NEUTRAL_FILL)
            .with_position(Position {
                x: 1900.0,
                y: 620.0,
            });
        let runtime = builder.add_top_level("runtime", runtime_element);

        let svg_element = Element::new("one .svg file")
            .with_shape(Shape::Disc)
            .with_style("fill", STORE_FILL)
            .with_position(Position {
                x: 1900.0,
                y: 330.0,
            });
        let svg = builder.add_top_level("svg", svg_element);

        let import_lex_element = Element::new("lex")
            .with_caption("logos tokens")
            .with_style("fill", IMPORT_STAGE_FILL)
            .with_position(Position { x: 18.0, y: 18.0 });
        let import_lex = builder.add_child(&import, "lex", import_lex_element);

        let import_parse_element = Element::new("parse")
            .with_caption("chumsky + recovery")
            .with_style("fill", IMPORT_STAGE_FILL)
            .with_position(Position { x: 150.0, y: 18.0 });
        let import_parse = builder.add_child(&import, "parse", import_parse_element);

        let import_lower_element = Element::new("lower")
            .with_caption("AST → model")
            .with_style("fill", IMPORT_STAGE_FILL)
            .with_position(Position { x: 282.0, y: 18.0 });
        let import_lower = builder.add_child(&import, "lower", import_lower_element);

        let model_path_element = Element::new("path")
            .with_caption("ids")
            .with_style("fill", MODEL_MODULE_FILL)
            .with_position(Position { x: 18.0, y: 18.0 });
        let model_path = builder.add_child(&model, "path", model_path_element);

        let model_graph_element = Element::new("model")
            .with_caption("elements · edges")
            .with_style("fill", MODEL_MODULE_FILL)
            .with_position(Position { x: 150.0, y: 18.0 });
        let model_graph = builder.add_child(&model, "graph", model_graph_element);

        let model_validate_element = Element::new("validate")
            .with_caption("problems · bounds")
            .with_style("fill", MODEL_MODULE_FILL)
            .with_position(Position { x: 18.0, y: 180.0 });
        let model_validate = builder.add_child(&model, "validate", model_validate_element);

        let model_json_element = Element::new("json")
            .with_caption("id-keyed schema")
            .with_style("fill", MODEL_MODULE_FILL)
            .with_position(Position { x: 150.0, y: 180.0 });
        let model_json = builder.add_child(&model, "json", model_json_element);

        let pipeline_metrics_element = Element::new("metrics")
            .with_caption("measures labels")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 600.0, y: 36.0 });
        let pipeline_metrics = builder.add_child(&pipeline, "metrics", pipeline_metrics_element);

        let pipeline_inter_element = Element::new("Inter")
            .with_shape(Shape::Cylinder)
            .with_badge()
            .with_style("fill", STORE_FILL)
            .with_position(Position { x: 780.0, y: 54.0 });
        let pipeline_inter = builder.add_child(&pipeline, "inter", pipeline_inter_element);

        let pipeline_layout_element = Element::new("layout")
            .with_caption("plan space")
            .with_style("fill", PIPELINE_STAGE_FILL)
            .with_position(Position { x: 36.0, y: 36.0 });
        let pipeline_layout = builder.add_child(&pipeline, "layout", pipeline_layout_element);

        let pipeline_geometry_element = Element::new("geometry")
            .with_caption("screen space")
            .with_style("fill", PIPELINE_STAGE_FILL)
            .with_position(Position { x: 36.0, y: 300.0 });
        let pipeline_geometry = builder.add_child(&pipeline, "geometry", pipeline_geometry_element);

        let pipeline_render_element = Element::new("render")
            .with_caption("markup")
            .with_style("fill", PIPELINE_STAGE_FILL)
            .with_position(Position { x: 560.0, y: 300.0 });
        let pipeline_render = builder.add_child(&pipeline, "render", pipeline_render_element);

        let pipeline_layout_place_element = Element::new("place")
            .with_caption("dagre · taffy · pins")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 18.0, y: 18.0 });
        let pipeline_layout_place =
            builder.add_child(&pipeline_layout, "place", pipeline_layout_place_element);

        let pipeline_layout_route_element = Element::new("route")
            .with_caption("ports")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 150.0, y: 18.0 });
        let pipeline_layout_route =
            builder.add_child(&pipeline_layout, "route", pipeline_layout_route_element);

        let pipeline_layout_stub_element = Element::new("stub")
            .with_caption("boundary slots")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 282.0, y: 18.0 });
        let pipeline_layout_stub =
            builder.add_child(&pipeline_layout, "stub", pipeline_layout_stub_element);

        let pipeline_geometry_projection_element = Element::new("projection")
            .with_caption("exact constants")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 18.0, y: 18.0 });
        let pipeline_geometry_projection = builder.add_child(
            &pipeline_geometry,
            "projection",
            pipeline_geometry_projection_element,
        );

        let pipeline_geometry_primitive_element = Element::new("primitive")
            .with_caption("4 primitives")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 150.0, y: 18.0 });
        let pipeline_geometry_primitive = builder.add_child(
            &pipeline_geometry,
            "primitive",
            pipeline_geometry_primitive_element,
        );

        let pipeline_geometry_depth_element = Element::new("depth")
            .with_caption("painter's order")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 282.0, y: 18.0 });
        let pipeline_geometry_depth =
            builder.add_child(&pipeline_geometry, "depth", pipeline_geometry_depth_element);

        let pipeline_render_theme_element = Element::new("theme")
            .with_caption("tokens · Oklab")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 18.0, y: 18.0 });
        let pipeline_render_theme =
            builder.add_child(&pipeline_render, "theme", pipeline_render_theme_element);

        let pipeline_render_profile_element = Element::new("profile")
            .with_caption("static · interactive")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 150.0, y: 18.0 });
        let pipeline_render_profile =
            builder.add_child(&pipeline_render, "profile", pipeline_render_profile_element);

        let pipeline_render_writer_element = Element::new("writer")
            .with_caption("escaping · decimals")
            .with_style("fill", PIPELINE_MODULE_FILL)
            .with_position(Position { x: 282.0, y: 18.0 });
        let pipeline_render_writer =
            builder.add_child(&pipeline_render, "writer", pipeline_render_writer_element);

        let import_lex_to_import_parse = Edge::new();
        builder.add_edge(&import_lex, &import_parse, import_lex_to_import_parse);
        let import_parse_to_import_lower = Edge::new();
        builder.add_edge(&import_parse, &import_lower, import_parse_to_import_lower);
        let model_path_to_model_graph = Edge::new();
        builder.add_edge(&model_path, &model_graph, model_path_to_model_graph);
        let model_graph_to_model_validate = Edge::new();
        builder.add_edge(&model_graph, &model_validate, model_graph_to_model_validate);
        let model_graph_to_model_json = Edge::new();
        builder.add_edge(&model_graph, &model_json, model_graph_to_model_json);
        let pipeline_layout_place_to_pipeline_layout_route = Edge::new();
        builder.add_edge(
            &pipeline_layout_place,
            &pipeline_layout_route,
            pipeline_layout_place_to_pipeline_layout_route,
        );
        let pipeline_layout_route_to_pipeline_layout_stub = Edge::new();
        builder.add_edge(
            &pipeline_layout_route,
            &pipeline_layout_stub,
            pipeline_layout_route_to_pipeline_layout_stub,
        );
        let pipeline_geometry_projection_to_pipeline_geometry_primitive = Edge::new();
        builder.add_edge(
            &pipeline_geometry_projection,
            &pipeline_geometry_primitive,
            pipeline_geometry_projection_to_pipeline_geometry_primitive,
        );
        let pipeline_geometry_primitive_to_pipeline_geometry_depth = Edge::new();
        builder.add_edge(
            &pipeline_geometry_primitive,
            &pipeline_geometry_depth,
            pipeline_geometry_primitive_to_pipeline_geometry_depth,
        );
        let pipeline_render_theme_to_pipeline_render_profile = Edge::new();
        builder.add_edge(
            &pipeline_render_theme,
            &pipeline_render_profile,
            pipeline_render_theme_to_pipeline_render_profile,
        );
        let pipeline_render_profile_to_pipeline_render_writer = Edge::new();
        builder.add_edge(
            &pipeline_render_profile,
            &pipeline_render_writer,
            pipeline_render_profile_to_pipeline_render_writer,
        );
        let pipeline_metrics_to_pipeline_layout = Edge::new();
        builder.add_edge(
            &pipeline_metrics,
            &pipeline_layout,
            pipeline_metrics_to_pipeline_layout,
        );
        let pipeline_layout_to_pipeline_geometry = Edge::new().with_label("③");
        builder.add_edge(
            &pipeline_layout,
            &pipeline_geometry,
            pipeline_layout_to_pipeline_geometry,
        );
        let pipeline_geometry_to_pipeline_render = Edge::new();
        builder.add_edge(
            &pipeline_geometry,
            &pipeline_render,
            pipeline_geometry_to_pipeline_render,
        );
        let pipeline_metrics_to_pipeline_inter = Edge::new().with_label("measures");
        builder.add_edge(
            &pipeline_metrics,
            &pipeline_inter,
            pipeline_metrics_to_pipeline_inter,
        );
        let pipeline_render_profile_to_pipeline_inter = Edge::new().with_label("outlines");
        builder.add_edge(
            &pipeline_render_profile,
            &pipeline_inter,
            pipeline_render_profile_to_pipeline_inter,
        );
        let text_to_cli = Edge::new();
        builder.add_edge(&text, &cli, text_to_cli);
        let cli_to_import = Edge::new();
        builder.add_edge(&cli, &import, cli_to_import);
        let cli_to_pipeline = Edge::new();
        builder.add_edge(&cli, &pipeline, cli_to_pipeline);
        let import_to_model = Edge::new().with_label("①");
        builder.add_edge(&import, &model, import_to_model);
        let model_to_pipeline = Edge::new().with_label("②");
        builder.add_edge(&model, &pipeline, model_to_pipeline);
        let pipeline_to_svg = Edge::new().with_label("④");
        builder.add_edge(&pipeline, &svg, pipeline_to_svg);
        let runtime_to_svg = Edge::new().with_style("stroke-dash", EMBEDDED_DASH);
        builder.add_edge(&runtime, &svg, runtime_to_svg);
        let d2_to_import = Edge::new()
            .with_label("grammar")
            .with_style("stroke", "#5b6170")
            .with_style("stroke-dash", "5");
        builder.add_edge(&d2, &import, d2_to_import);
        let pipeline_render_writer_to_contract = Edge::new()
            .with_label("emits")
            .with_style("stroke-dash", EMBEDDED_DASH);
        builder.add_edge(
            &pipeline_render_writer,
            &contract,
            pipeline_render_writer_to_contract,
        );
        let runtime_to_contract = Edge::new()
            .with_label("reads")
            .with_style("stroke-dash", EMBEDDED_DASH);
        builder.add_edge(&runtime, &contract, runtime_to_contract);

        builder
            .build()
            .model
            .expect("the svugy example declares every element once and every endpoint")
    }
}
