use miette::Diagnostic;
use svugy_model::builder::ModelBuilder;
use svugy_model::legend::Legend;
use svugy_model::model::{
    ByteRange, Edge, Element, MAX_MAGNITUDE, Model, PlacementHint, Position, Shape,
};
use svugy_model::path::ElementPath;
use svugy_model::problem::{ErrorSeverity, Problem, StyleOwner, Suggestion};
use svugy_model::style::AnyStyleKey;
use svugy_model::validate::ModelReport;

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

fn rejected_report(builder: ModelBuilder) -> ModelReport {
    let admission = builder.build();
    let is_admitted = admission.model.is_some();
    assert!(!is_admitted, "the model has errors: {:?}", admission.report);
    admission.report
}

fn admitted_report(builder: ModelBuilder) -> ModelReport {
    let admission = builder.build();
    let is_admitted = admission.model.is_some();
    assert!(
        is_admitted,
        "the model has no error: {:?}",
        admission.report
    );
    admission.report
}

fn report_of_top_level(element: Element) -> ModelReport {
    let mut builder = ModelBuilder::new();
    let _element = builder.add_top_level("api", element);
    rejected_report(builder)
}

fn report_of_link(link: &str) -> ModelReport {
    let mut builder = ModelBuilder::new();
    let backend = builder.add_top_level("backend", Element::new("backend"));
    let _api = builder.add_child(&backend, "api", Element::new("API"));
    let linked = Element::new("docs").with_link(link);
    let _docs = builder.add_top_level("docs", linked);
    admitted_report(builder)
}

#[test]
fn a_clean_model_validates_to_an_empty_report() {
    let mut builder = ModelBuilder::new();
    let styled = Element::new("api")
        .with_style("fill", "#3478F6")
        .with_style("font", "mono")
        .with_style("stroke-width", "2")
        .with_style("shadow", "true");
    let _api = builder.add_top_level("api", styled);

    let report = admitted_report(builder);

    assert!(report.problems().is_empty());
    assert!(!report.has_errors());
}

#[test]
fn an_ancestor_descendant_edge_is_a_warning_that_keeps_its_span() {
    let mut builder = ModelBuilder::new();
    let desk = builder.add_top_level("desk", Element::new("desk"));
    let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
    let into_child = Edge::new().with_span(ByteRange::new(3, 17));
    let down_edge = builder.add_edge(&desk, &quoter, into_child);

    let report = admitted_report(builder);

    let [Problem::AncestorDescendantEdge(problem)] = report.problems() else {
        panic!("expected one ancestor-descendant edge, got {report:?}");
    };
    assert_eq!(problem.edge, down_edge);
    assert_eq!(problem.span, Some((3..17).into()));
    assert!(!report.has_errors());
}

#[test]
fn an_unknown_style_key_fails_the_build_with_a_suggestion() {
    let typo = Element::new("api").with_style("fil", "#fff");

    let report = report_of_top_level(typo);

    let [Problem::UnknownStyleKey(problem)] = report.problems() else {
        panic!("expected one unknown style key, got {report:?}");
    };
    let help = problem.help().map(|help| help.to_string());
    assert_eq!(problem.owner, StyleOwner::Element(parse("api")));
    assert_eq!(problem.suggestion, Some(Suggestion("fill")));
    assert_eq!(help.as_deref(), Some("did you mean `fill`?"));
}

#[test]
fn a_style_value_of_the_wrong_kind_fails_the_build() {
    let mut builder = ModelBuilder::new();
    let a = builder.add_top_level("a", Element::new("a"));
    let b = builder.add_top_level("b", Element::new("b"));
    let dashed = Edge::new().with_style("stroke-dash", "dashed");
    let edge = builder.add_edge(&a, &b, dashed);

    let report = rejected_report(builder);

    let [Problem::MismatchedStyleValue(problem)] = report.problems() else {
        panic!("expected one mismatched style value, got {report:?}");
    };
    let stroke_dash = AnyStyleKey::find("stroke-dash").expect("v1 key");
    assert_eq!(problem.owner, StyleOwner::Edge(edge));
    assert_eq!(problem.key, stroke_dash);
    assert_eq!(problem.value, "dashed");
}

#[test]
fn a_grid_of_zero_columns_fails_the_build() {
    let mut builder = ModelBuilder::new();
    let empty_grid =
        Element::new("cluster").with_child_placement(PlacementHint::Grid { columns: 0 });
    let cluster = builder.add_top_level("cluster", empty_grid);
    let _member = builder.add_child(&cluster, "member", Element::new("member"));

    let report = rejected_report(builder);

    let [Problem::EmptyGridHint(problem)] = report.problems() else {
        panic!("expected one empty grid, got {report:?}");
    };
    assert_eq!(problem.element, parse("cluster"));
}

#[test]
fn a_grid_of_more_columns_than_a_packed_level_holds_fails_the_build() {
    let mut builder = ModelBuilder::new();
    let too_wide = PlacementHint::MAX_PACKED_TRACKS + 2;
    let wide_grid =
        Element::new("cluster").with_child_placement(PlacementHint::Grid { columns: too_wide });
    let cluster = builder.add_top_level("cluster", wide_grid);
    for name in ["a", "b", "c"] {
        builder.add_child(&cluster, name, Element::new(name));
    }

    let report = rejected_report(builder);

    let [Problem::UncountablePackedTracks(problem)] = report.problems() else {
        panic!("expected one uncountable grid, got {report:?}");
    };
    assert_eq!(problem.element, parse("cluster"));
    assert_eq!(problem.columns, 32_768);
    assert_eq!(problem.rows, 1);
}

#[test]
fn a_row_or_column_longer_than_a_packed_level_holds_fails_the_build() {
    let child_count = PlacementHint::MAX_PACKED_TRACKS as usize + 1;
    for hint in [PlacementHint::Row, PlacementHint::Column] {
        let mut builder = ModelBuilder::new();
        let line_element = Element::new("line").with_child_placement(hint);
        let line = builder.add_top_level("line", line_element);
        for child_index in 0..child_count {
            let child_id = format!("c{child_index}");
            builder.add_child(&line, &child_id, Element::new("c"));
        }

        let report = rejected_report(builder);

        let [Problem::UncountablePackedTracks(problem)] = report.problems() else {
            panic!(
                "expected one uncountable line, got {} problems",
                report.problems().len()
            );
        };
        assert_eq!(problem.child_count, child_count);
    }
}

#[test]
fn a_round_container_warns_that_its_shape_is_ignored_with_its_span() {
    let mut builder = ModelBuilder::new();
    let round_element = Element::new("pond")
        .with_shape(Shape::Cylinder)
        .with_span(ByteRange::new(5, 9));
    let pond = builder.add_top_level("pond", round_element);
    builder.add_child(&pond, "fish", Element::new("fish"));
    let round_leaf = Element::new("store").with_shape(Shape::Disc);
    builder.add_top_level("store", round_leaf);
    let slab_element = Element::new("desk").with_shape(Shape::Slab);
    let desk = builder.add_top_level("desk", slab_element);
    builder.add_child(&desk, "quoter", Element::new("quoter"));

    let report = admitted_report(builder);

    let [Problem::IgnoredContainerShape(problem)] = report.problems() else {
        panic!("expected one ignored container shape, got {report:?}");
    };
    assert_eq!(problem.element, parse("pond"));
    assert_eq!(problem.shape, Shape::Cylinder);
    assert_eq!(problem.span, Some((5..9).into()));
}

#[test]
fn a_container_height_warns_that_it_is_ignored_with_its_span_and_a_leaf_height_passes() {
    let mut builder = ModelBuilder::new();
    let tall_container = Element::new("desk")
        .with_height(40.0)
        .with_span(ByteRange::new(3, 7));
    let desk = builder.add_top_level("desk", tall_container);
    builder.add_child(&desk, "quoter", Element::new("quoter").with_height(40.0));
    let tall_leaf = Element::new("app").with_height(84.0);
    builder.add_top_level("app", tall_leaf);

    let report = admitted_report(builder);

    let [Problem::IgnoredContainerHeight(problem)] = report.problems() else {
        panic!("expected one ignored container height, got {report:?}");
    };
    assert_eq!(problem.element, parse("desk"));
    assert_eq!(problem.height, 40.0);
    assert_eq!(problem.span, Some((3..7).into()));
}

#[test]
fn a_height_that_is_not_a_positive_bounded_number_fails_the_build_and_one_at_the_bound_passes() {
    let impossible_heights = [0.0, -1.0, f64::NAN, f64::INFINITY, MAX_MAGNITUDE * 2.0];
    for impossible_height in impossible_heights {
        let raised = Element::new("api").with_height(impossible_height);

        let report = report_of_top_level(raised);

        let [Problem::ImpossibleHeight(problem)] = report.problems() else {
            panic!("expected one impossible height, got {report:?}");
        };
        assert_eq!(problem.element, parse("api"));
        let message = problem.to_string();
        assert!(message.contains("no greater than 1000000"), "{message}");
    }
    let mut builder = ModelBuilder::new();
    let tallest = Element::new("api").with_height(MAX_MAGNITUDE);
    let _api = builder.add_top_level("api", tallest);
    let _report = admitted_report(builder);
}

#[test]
fn a_child_placement_on_a_leaf_is_only_a_warning() {
    let mut builder = ModelBuilder::new();
    let row_without_children = Element::new("leaf").with_child_placement(PlacementHint::Row);
    let _leaf = builder.add_top_level("leaf", row_without_children);

    let report = admitted_report(builder);

    let [Problem::ChildPlacementWithoutChildren(problem)] = report.problems() else {
        panic!("expected one child placement without children, got {report:?}");
    };
    assert_eq!(problem.child_placement, PlacementHint::Row);
}

#[test]
fn a_non_finite_position_fails_the_build() {
    let unreachable_position = Position {
        x: f64::NAN,
        y: f64::INFINITY,
    };
    let pinned = Element::new("api").with_position(unreachable_position);

    let report = report_of_top_level(pinned);

    let [Problem::UnboundedPosition(problem)] = report.problems() else {
        panic!("expected one unbounded position, got {report:?}");
    };
    assert_eq!(problem.element, parse("api"));
}

#[test]
fn a_position_beyond_the_magnitude_bound_fails_the_build_and_one_at_it_passes() {
    let far_position = Position { x: 1e7, y: 0.0 };
    let far_pinned = Element::new("api").with_position(far_position);

    let report = report_of_top_level(far_pinned);

    let [Problem::UnboundedPosition(problem)] = report.problems() else {
        panic!("expected one unbounded position, got {report:?}");
    };
    assert_eq!(problem.position, far_position);
    let mut builder = ModelBuilder::new();
    let edge_position = Position {
        x: -MAX_MAGNITUDE,
        y: MAX_MAGNITUDE,
    };
    let edge_pinned = Element::new("api").with_position(edge_position);
    let _api = builder.add_top_level("api", edge_pinned);
    let _report = admitted_report(builder);
}

#[test]
fn a_style_number_beyond_the_magnitude_bound_fails_the_build() {
    let huge_dash = Element::new("api").with_style("stroke-dash", "10000000000000");

    let report = report_of_top_level(huge_dash);

    let [Problem::MismatchedStyleValue(problem)] = report.problems() else {
        panic!("expected one mismatched style value, got {report:?}");
    };
    let message = problem.to_string();
    assert!(message.contains("no greater than 1000000"), "{message}");
}

#[test]
fn a_suggestion_ignores_case_and_allows_one_edit_per_three_characters() {
    let cases = [
        ("b", vec!["a"], None),
        ("ab", vec!["ac"], None),
        ("A", vec!["a"], Some("a")),
        ("Db", vec!["dc", "db"], Some("db")),
        ("fil", vec!["fill"], Some("fill")),
        ("FILL", vec!["font", "fill"], Some("fill")),
        ("apixx", vec!["api"], None),
        ("strok-wdth", vec!["stroke-width"], Some("stroke-width")),
    ];
    for (unknown, candidates, expected) in cases {
        let suggestion = Suggestion::closest_to(unknown, candidates);

        let expected_suggestion = expected.map(Suggestion);
        assert_eq!(suggestion, expected_suggestion, "{unknown}");
    }
}

#[test]
fn a_path_shaped_link_to_no_element_warns_with_a_suggestion() {
    let report = report_of_link("backend.apii");

    let [Problem::UnresolvedLink(problem)] = report.problems() else {
        panic!("expected one unresolved link, got {report:?}");
    };
    let expected_suggestion = Suggestion(parse("backend.api"));
    assert_eq!(problem.element, parse("docs"));
    assert_eq!(problem.suggestion, Some(expected_suggestion));
    assert!(!report.has_errors());
}

#[test]
fn an_unresolved_link_labels_its_link_value_else_its_declaration() {
    let mut builder = ModelBuilder::new();
    let with_link_span = Element::new("docs")
        .with_link("nowhere")
        .with_span(ByteRange::new(0, 4))
        .with_link_span(ByteRange::new(12, 19));
    let declaration_only = Element::new("guide")
        .with_link("nowhere")
        .with_span(ByteRange::new(30, 35));
    let _docs = builder.add_top_level("docs", with_link_span);
    let _guide = builder.add_top_level("guide", declaration_only);

    let report = admitted_report(builder);

    let spans: Vec<_> = report
        .problems()
        .iter()
        .map(|problem| {
            let Problem::UnresolvedLink(unresolved_link) = problem else {
                panic!("expected only unresolved links, got {problem:?}");
            };
            unresolved_link.span
        })
        .collect();
    assert_eq!(spans, [Some((12..19).into()), Some((30..35).into())]);
}

#[test]
fn resolved_links_urls_and_relative_paths_raise_nothing() {
    let links = [
        "backend.api",
        "https://example.com/api",
        "mailto:team@example.com",
        "./guide.html",
        "docs/guide.html",
        "#section",
    ];
    for link in links {
        let report = report_of_link(link);

        assert!(report.problems().is_empty(), "{link}: {report:?}");
    }
}

#[test]
fn a_json_model_with_only_warnings_loads() {
    let warned = r#"{
        "elements": {
            "desk": { "label": "desk", "link": "dsk" },
            "desk.quoter": { "label": "quoter" }
        },
        "edges": [ { "from": "desk", "to": "desk.quoter" } ]
    }"#;

    let admission = Model::load_json(warned).expect("well-formed JSON");

    let report = admission.report;
    let is_error_free = report.problems().iter().all(|problem| !problem.is_error());
    assert!(admission.model.is_some());
    assert_eq!(report.problems().len(), 2);
    assert!(is_error_free);
}

#[test]
fn edges_from_leaves_into_a_leaf_marked_badge_are_badges_and_every_other_edge_an_arc() {
    let mut builder = ModelBuilder::new();
    let desk = builder.add_top_level("desk", Element::new("desk"));
    let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
    let store_element = Element::new("SQLite").with_badge_target();
    let store = builder.add_child(&desk, "sqlite", store_element);
    let app = builder.add_top_level("app", Element::new("app"));
    let writes = builder.add_edge(&quoter, &store, Edge::new());
    let app_writes = builder.add_edge(&app, &store, Edge::new());
    let desk_writes = builder.add_edge(&desk, &store, Edge::new());
    let reads = builder.add_edge(&store, &quoter, Edge::new());
    let admission = builder.build();
    let model = admission.model.expect("the model has no error");

    let quoter_path = parse("desk.quoter");
    let quoter_badges: Vec<_> = model.badge_edges_from(&quoter_path).collect();
    let badges: Vec<_> = model.badge_edges().collect();
    let arcs: Vec<_> = model.arc_edges().map(|(id, _edge)| id).collect();

    assert!(model.is_badge_edge(&writes));
    assert!(model.is_badge_edge(&app_writes));
    assert!(!model.is_badge_edge(&desk_writes));
    assert!(!model.is_badge_edge(&reads));
    assert_eq!(quoter_badges, [&writes]);
    assert_eq!(badges, [&app_writes, &writes]);
    assert_eq!(arcs, [&reads]);
    let problems = admission.report.problems();
    let [first_problem, second_problem] = problems else {
        panic!("a down edge and an arc into a badge target: {problems:?}");
    };
    let is_ancestor_descendant_edge = matches!(
        first_problem,
        Problem::AncestorDescendantEdge(_ancestor_descendant_edge)
    );
    assert!(is_ancestor_descendant_edge);
    let Problem::ArcIntoBadgeTarget(arc_into_badge_target) = second_problem else {
        panic!("an edge from a container into a badge target warns: {second_problem:?}");
    };
    assert_eq!(arc_into_badge_target.edge, desk_writes);
    assert!(!second_problem.is_error());
}

#[test]
fn a_container_marked_badge_warns_that_it_is_entered_and_its_edges_stay_arcs() {
    let mut builder = ModelBuilder::new();
    let desk_element = Element::new("desk")
        .with_badge_target()
        .with_span(ByteRange::new(0, 4));
    let desk = builder.add_top_level("desk", desk_element);
    let _quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
    let app = builder.add_top_level("app", Element::new("app"));
    let calls = builder.add_edge(&app, &desk, Edge::new());

    let admission = builder.build();

    let model = admission.model.expect("the model has no error");
    assert!(!model.is_badge_edge(&calls));
    let problems = admission.report.problems();
    let [Problem::IgnoredContainerBadge(ignored_badge)] = problems else {
        panic!("only the ignored badge mark is reported: {problems:?}");
    };
    assert_eq!(ignored_badge.element, parse("desk"));
    let labels: Vec<_> = problems[0]
        .labels()
        .expect("the warning is labelled")
        .collect();
    assert_eq!(labels[0].offset(), 0);
}

#[test]
fn a_legend_describing_what_the_diagram_never_draws_warns_with_the_closest_badge_target() {
    let mut builder = ModelBuilder::new();
    let legend = Legend::new()
        .with_dash_description(5.0, "HTTP")
        .with_dash_description(3.0, "never dashed")
        .with_badge_description(parse("sqlite"), "writes")
        .with_badge_description(parse("ledgr"), "typo");
    builder = builder.with_legend(legend);
    let app = builder.add_top_level("app", Element::new("app"));
    let api = builder.add_top_level("api", Element::new("api"));
    let sqlite = builder.add_top_level("sqlite", Element::new("SQLite").with_badge_target());
    let _ledger = builder.add_top_level("ledger", Element::new("ledger").with_badge_target());
    let _calls = builder.add_edge(&app, &api, Edge::new());
    let _writes = builder.add_edge(&app, &sqlite, Edge::new());

    let report = admitted_report(builder);

    let undrawn: Vec<(String, Option<String>)> = report
        .problems()
        .iter()
        .filter_map(|problem| match problem {
            Problem::UndrawnLegendKey(undrawn_key) => {
                let suggestion = undrawn_key.suggestion.as_ref();
                let suggested = suggestion.map(|suggestion| suggestion.0.to_string());
                Some((undrawn_key.key.to_string(), suggested))
            }
            _other => None,
        })
        .collect();
    let expected = [
        ("the arcs dashed `3`".to_owned(), None),
        (
            "the badges of `ledgr`".to_owned(),
            Some("ledger".to_owned()),
        ),
    ];
    assert_eq!(undrawn, expected);
    assert_eq!(report.problems().len(), 2);
}

#[test]
fn every_problem_kind_reaches_the_machine_readable_report_with_its_severity() {
    let mut other_builder = ModelBuilder::new();
    let foreign_parent = other_builder.add_top_level("cache", Element::new("cache"));
    let foreign_endpoint = other_builder.add_top_level("dbb", Element::new("dbb"));
    let legend = Legend::new()
        .with_dash_description(3.0, "never dashed")
        .with_badge_description(parse("ledgr"), "typo");
    let mut builder = ModelBuilder::new().with_legend(legend);
    let desk_element = Element::new("desk")
        .with_style("fil", "#1b2436")
        .with_style("bold", "very")
        .with_child_placement(PlacementHint::Grid { columns: 0 })
        .with_shape(Shape::Disc)
        .with_height(30.0)
        .with_badge_target()
        .with_span(ByteRange::new(0, 4));
    let desk = builder.add_top_level("desk", desk_element);
    let wide_grid = PlacementHint::Grid { columns: 40_000 };
    let wide_element = Element::new("wide").with_child_placement(wide_grid);
    let wide = builder.add_top_level("wide", wide_element);
    let _cell = builder.add_child(&wide, "cell", Element::new("cell"));
    let ledger = builder.add_top_level("ledger", Element::new("ledger").with_badge_target());
    let _wide_writes = builder.add_edge(
        &wide,
        &ledger,
        Edge::new().with_span(ByteRange::new(60, 74)),
    );
    let quoter_element = Element::new("quoter")
        .with_child_placement(PlacementHint::Column)
        .with_link("desk.quoterr");
    let quoter = builder.add_child(&desk, "quoter", quoter_element);
    let _duplicate = builder.add_child(
        &desk,
        "quoter",
        Element::new("again").with_span(ByteRange::new(20, 26)),
    );
    let _db = builder.add_top_level("db", Element::new("db"));
    let pinned_nowhere = Position {
        x: f64::INFINITY,
        y: 0.0,
    };
    let _hot = builder.add_child(
        &foreign_parent,
        "hot",
        Element::new("hot")
            .with_position(pinned_nowhere)
            .with_height(0.0),
    );
    let _dangling = builder.add_edge(&quoter, &foreign_endpoint, Edge::new());
    let _down_edge = builder.add_edge(
        &desk,
        &quoter,
        Edge::new().with_span(ByteRange::new(40, 52)),
    );
    let report = rejected_report(builder);

    let rendered = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("report is JSON");
    let pretty = serde_json::to_string_pretty(&parsed).expect("re-serializes");

    insta::assert_snapshot!(pretty);
}
