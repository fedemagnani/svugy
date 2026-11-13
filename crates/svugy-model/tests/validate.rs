use std::collections::BTreeMap;

use svugy_model::builder::{DescribedRef, ModelBuilder, StepRef, WorkflowRef};
use svugy_model::model::{
    ByteRange, Edge, EdgeId, Element, MAX_MAGNITUDE, PlacementHint, Position, Shape,
};
use svugy_model::path::ElementPath;
use svugy_model::problem::{
    ElementSite, ErrorSeverity, Problem, StyleOwner, Suggestion, UnresolvedTargetReason,
};
use svugy_model::properties::ComponentProperties;
use svugy_model::style::AnyStyleKey;
use svugy_model::validate::ModelReport;
use svugy_model::workflow::{ElementOverride, Step, StepId, StepItem, Workflow, WorkflowId};

#[test]
fn a_clean_model_validates_to_an_empty_report() {
    let mut builder = ModelBuilder::new();
    let styled = Element::new("api")
        .with_style("fill", "#3478F6")
        .with_style("font", "mono")
        .with_style("stroke-width", "2")
        .with_style("shadow", "true");
    let api = builder.add_top_level("api", styled);
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &api);

    let report = admitted_report(builder);

    assert!(report.problems().is_empty());
    assert!(!report.has_errors());
}

#[test]
fn description_targets_resolve_in_the_scope_their_description_is_written_in() {
    let mut builder = ModelBuilder::new();
    let web = builder.add_top_level("web", Element::new("web"));
    let api = builder.add_top_level("api", Element::new("api"));
    let vault = builder.add_top_level("vault", Element::new("vault"));
    let checkout_workflow = Workflow::new(WorkflowId::new("checkout"), "A customer pays");
    let checkout = builder.add_workflow(checkout_workflow);
    let order_step = Step::new(StepId::new("order"), "The shop takes the order");
    let order = builder.add_step(&checkout, order_step);
    let _places_order = builder.add_edge(&order, &web, &api, Edge::new());
    let _retries = builder.add_edge(&order, &web, &api, Edge::new());
    let refund_workflow = Workflow::new(WorkflowId::new("refund"), "A customer is refunded");
    let refund = builder.add_workflow(refund_workflow);
    let give_back_step = Step::new(StepId::new("give-back"), "The vault gives the money back");
    let give_back = builder.add_step(&refund, give_back_step);
    let _repays = builder.add_edge(&give_back, &vault, &web, Edge::new());
    let web_owner = DescribedRef::Element(web);
    let order_owner = DescribedRef::Step(order);
    builder.describe(
        &web_owner,
        "Sends [the cart](#web->api) and is [repaid](#vault->web).",
    );
    builder.describe(
        &order_owner,
        "[The vault](#vault) waits; see [the refund](#refund/give-back).",
    );

    let report = admitted_report(builder);

    let reasons: Vec<(String, &UnresolvedTargetReason)> = report
        .problems()
        .iter()
        .map(|problem| {
            let Problem::UnresolvedDescriptionTarget(unresolved_target) = problem else {
                panic!("expected only unresolved targets, got {problem:?}");
            };
            (unresolved_target.target.clone(), &unresolved_target.reason)
        })
        .collect();
    let checkout_id = WorkflowId::new("checkout");
    let refund_id = WorkflowId::new("refund");
    let expected_reasons = [
        (
            "web->api".to_owned(),
            &UnresolvedTargetReason::NamesSeveralEdgesIn(checkout_id.clone()),
        ),
        (
            "web->api".to_owned(),
            &UnresolvedTargetReason::NamesNothingIn(refund_id),
        ),
        (
            "vault->web".to_owned(),
            &UnresolvedTargetReason::NamesNothingIn(checkout_id.clone()),
        ),
        (
            "vault".to_owned(),
            &UnresolvedTargetReason::IsNotShownBy(checkout_id),
        ),
    ];
    assert_eq!(
        reasons, expected_reasons,
        "a component's edge target resolves in each workflow showing it; a step's targets resolve in its workflow, steps of any workflow included"
    );
}

#[test]
fn a_style_value_of_the_wrong_kind_fails_the_build() {
    let mut builder = ModelBuilder::new();
    let a = builder.add_top_level("a", Element::new("a"));
    let b = builder.add_top_level("b", Element::new("b"));
    let (_main, all) = main_step(&mut builder);
    let dashed = Edge::new().with_style("stroke-dash", "dashed");
    let edge_ref = builder.add_edge(&all, &a, &b, dashed);
    let edge = edge_ref.id().clone();

    let report = rejected_report(builder);

    let [Problem::MismatchedStyleValue(problem)] = report.problems() else {
        panic!("expected one mismatched style value, got {report:?}");
    };
    let stroke_dash = AnyStyleKey::find("stroke-dash").expect("v1 key");
    let workflow = WorkflowId::new("main");
    assert_eq!(problem.owner, StyleOwner::Edge { workflow, edge });
    assert_eq!(problem.key, stroke_dash);
    assert_eq!(problem.value, "dashed");
}

#[test]
fn a_grid_of_more_columns_than_a_packed_level_holds_fails_the_build() {
    let too_wide = PlacementHint::MAX_PACKED_TRACKS + 2;
    let wide_grid =
        ElementOverride::new().with_child_placement(PlacementHint::Grid { columns: too_wide });

    let report = rejected_report(cluster_overridden_by(wide_grid, 3));

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
        let line = ElementOverride::new().with_child_placement(hint);

        let report = rejected_report(cluster_overridden_by(line, child_count));

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
fn a_round_container_warns_that_its_shape_is_ignored_where_it_was_written() {
    let mut builder = ModelBuilder::new();
    let round_element = Element::new("pond")
        .with_shape(Shape::Cylinder)
        .with_span(ByteRange::new(5, 9));
    let pond = builder.add_top_level("pond", round_element);
    let fish = builder.add_child(&pond, "fish", Element::new("fish"));
    let store = builder.add_top_level("store", Element::new("store"));
    let (main, all) = main_step(&mut builder);
    builder.list_element(&all, &fish);
    builder.list_element(&all, &store);
    let disc = ComponentProperties::new().with_shape(Shape::Disc);
    let round_store = ElementOverride::new().with_properties(disc);
    builder.override_element(&main, &store, round_store);

    let report = admitted_report(builder);

    let [Problem::IgnoredContainerShape(problem)] = report.problems() else {
        panic!("expected one ignored container shape, got {report:?}");
    };
    assert_eq!(problem.element, parse("pond"));
    assert_eq!(problem.shape, Shape::Cylinder);
    assert_eq!(problem.span, Some((5..9).into()));
}

#[test]
fn a_container_height_warns_that_it_is_ignored_only_where_the_element_is_a_container() {
    let mut builder = ModelBuilder::new();
    let tall_desk = Element::new("desk")
        .with_height(40.0)
        .with_span(ByteRange::new(3, 7));
    let desk = builder.add_top_level("desk", tall_desk);
    let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &quoter);
    let closed_workflow = Workflow::new(WorkflowId::new("closed"), "The desk alone");
    let closed = builder.add_workflow(closed_workflow);
    let alone_step = Step::new(StepId::new("alone"), "Alone");
    let alone = builder.add_step(&closed, alone_step);
    builder.list_element(&alone, &desk);

    let report = admitted_report(builder);

    let [Problem::IgnoredContainerHeight(problem)] = report.problems() else {
        panic!("expected one ignored container height, got {report:?}");
    };
    assert_eq!(problem.workflow, WorkflowId::new("main"));
    assert_eq!(problem.height, 40.0);
    assert_eq!(problem.span, Some((3..7).into()));
}

#[test]
fn a_height_that_is_not_a_positive_bounded_number_fails_the_build_and_one_at_the_bound_passes() {
    let impossible_heights = [0.0, -1.0, f64::NAN, f64::INFINITY, MAX_MAGNITUDE * 2.0];
    for impossible_height in impossible_heights {
        let raised = Element::new("api").with_height(impossible_height);

        let report = report_of_listed_top_level(raised);

        let [Problem::ImpossibleHeight(problem)] = report.problems() else {
            panic!("expected one impossible height, got {report:?}");
        };
        assert_eq!(problem.site, ElementSite::Declaration(parse("api")));
        let message = problem.to_string();
        assert!(message.contains("no greater than 1000000"), "{message}");
    }
    let mut builder = ModelBuilder::new();
    let tallest = Element::new("api").with_height(MAX_MAGNITUDE);
    let api = builder.add_top_level("api", tallest);
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &api);
    let _report = admitted_report(builder);
}

#[test]
fn a_position_not_within_the_magnitude_bound_fails_the_build_and_one_at_it_passes() {
    let unreachable_positions = [
        Position {
            x: f64::NAN,
            y: 0.0,
        },
        Position {
            x: 0.0,
            y: f64::INFINITY,
        },
        Position { x: 1e7, y: 0.0 },
    ];
    for unreachable_position in unreachable_positions {
        let pinned = ElementOverride::new().with_position(unreachable_position);

        let report = rejected_report(cluster_overridden_by(pinned, 0));

        let [Problem::UnboundedPosition(problem)] = report.problems() else {
            panic!("expected one unbounded position, got {report:?}");
        };
        assert_eq!(problem.element, parse("cluster"));
    }
    let edge_position = Position {
        x: -MAX_MAGNITUDE,
        y: MAX_MAGNITUDE,
    };
    let pinned_at_bound = ElementOverride::new().with_position(edge_position);
    let _report = admitted_report(cluster_overridden_by(pinned_at_bound, 0));
}

#[test]
fn a_style_number_beyond_the_magnitude_bound_fails_the_build() {
    let huge_dash = Element::new("api").with_style("stroke-dash", "10000000000000");

    let report = report_of_listed_top_level(huge_dash);

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
    assert_eq!(problem.site, ElementSite::Declaration(parse("docs")));
    assert_eq!(problem.suggestion, Some(expected_suggestion));
    assert!(!report.has_errors());
}

#[test]
fn an_unresolved_link_labels_its_link_value_else_its_declaration() {
    let mut builder = ModelBuilder::new();
    let link_value_span = Some(ByteRange::new(12, 19));
    let spanned_link = ComponentProperties::new().with_link_written_at("nowhere", link_value_span);
    let with_link_span = Element::new("docs")
        .with_properties(spanned_link)
        .with_span(ByteRange::new(0, 4));
    let unspanned_link = ComponentProperties::new().with_link("nowhere");
    let declaration_only = Element::new("guide")
        .with_properties(unspanned_link)
        .with_span(ByteRange::new(30, 35));
    let docs = builder.add_top_level("docs", with_link_span);
    let guide = builder.add_top_level("guide", declaration_only);
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &docs);
    builder.list_element(&all, &guide);

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
fn a_model_with_only_warnings_is_admitted() {
    let mut builder = ModelBuilder::new();
    let misspelled_link = ComponentProperties::new().with_link("dsk");
    let desk_element = Element::new("desk").with_properties(misspelled_link);
    let desk = builder.add_top_level("desk", desk_element);
    let _idle = builder.add_top_level("idle", Element::new("idle"));
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &desk);

    let admission = builder.build();

    let report = admission.report;
    let is_error_free = report.problems().iter().all(|problem| !problem.is_error());
    assert!(admission.model.is_some());
    assert_eq!(report.problems().len(), 2);
    assert!(is_error_free);
}

#[test]
fn every_problem_kind_reaches_the_machine_readable_report_with_its_severity() {
    let mut other_builder = ModelBuilder::new();
    let foreign_parent = other_builder.add_top_level("cache", Element::new("cache"));
    let foreign_endpoint = other_builder.add_top_level("dbb", Element::new("dbb"));
    let mut builder = ModelBuilder::new();
    let desk_element = Element::new("desk")
        .with_style("fil", "#1b2436")
        .with_style("bold", "very")
        .with_shape(Shape::Disc)
        .with_height(30.0)
        .with_span(ByteRange::new(0, 4))
        .with_description("Feeds [the ledger](#desk->ledger).");
    let desk = builder.add_top_level("desk", desk_element);
    let wide = builder.add_top_level("wide", Element::new("wide"));
    let cell = builder.add_child(&wide, "cell", Element::new("cell"));
    let db_link = ComponentProperties::new().with_link("db");
    let ledger = builder.add_top_level("ledger", Element::new("ledger").with_properties(db_link));
    let misspelled_link = ComponentProperties::new().with_link("desk.quoterr");
    let quoter_element = Element::new("quoter").with_properties(misspelled_link);
    let quoter = builder.add_child(&desk, "quoter", quoter_element);
    let _duplicate = builder.add_child(
        &desk,
        "quoter",
        Element::new("again").with_span(ByteRange::new(20, 26)),
    );
    let code_value_span = Some(ByteRange::new(80, 97));
    let spanned_code =
        ComponentProperties::new().with_code_written_at("../store/db.rs#Db", code_value_span);
    let db_element = Element::new("db").with_properties(spanned_code);
    let db = builder.add_top_level("db", db_element);
    let _hot = builder.add_child(&foreign_parent, "hot", Element::new("hot").with_height(0.0));
    let main_workflow =
        Workflow::new(WorkflowId::new("main"), "Main").with_description("See [the vault](#vault).");
    let main = builder.add_workflow(main_workflow);
    let all = builder.add_step(&main, Step::new(StepId::new("all"), "All"));
    let _wide_writes = builder.add_edge(
        &all,
        &wide,
        &ledger,
        Edge::new().with_badge().with_span(ByteRange::new(60, 74)),
    );
    let _dangling = builder.add_edge(&all, &quoter, &foreign_endpoint, Edge::new());
    let _down_edge = builder.add_edge(
        &all,
        &desk,
        &quoter,
        Edge::new().with_span(ByteRange::new(40, 52)),
    );
    let _self_loop = builder.add_edge(&all, &quoter, &quoter, Edge::new());
    let _badge_into_container = builder.add_edge(&all, &ledger, &wide, Edge::new().with_badge());
    builder.list_element(&all, &cell);
    builder.list_element(&all, &cell);
    builder.list_edge(&all, EdgeId::new(parse("desk"), parse("ledger"), 3));
    let desk_override = ElementOverride::new()
        .with_child_placement(PlacementHint::Grid { columns: 0 })
        .with_position(Position {
            x: f64::INFINITY,
            y: 0.0,
        });
    builder.override_element(&main, &desk, desk_override);
    builder.override_element(
        &main,
        &desk,
        ElementOverride::new().with_span(ByteRange::new(90, 95)),
    );
    let wide_override =
        ElementOverride::new().with_child_placement(PlacementHint::Grid { columns: 40_000 });
    builder.override_element(&main, &wide, wide_override);
    let cell_override = ElementOverride::new().with_child_placement(PlacementHint::Column);
    builder.override_element(&main, &cell, cell_override);
    builder.override_element(&main, &db, ElementOverride::new());
    let _empty = builder.add_step(&main, Step::new(StepId::new("all"), "All again"));
    let malformed = Workflow::new(WorkflowId::new("not an id"), "Malformed");
    let malformed_workflow = builder.add_workflow(malformed);
    let _bad_step = builder.add_step(&malformed_workflow, Step::new(StepId::new("-x"), "Bad"));
    let _again = builder.add_workflow(Workflow::new(WorkflowId::new("main"), "Main again"));
    let unlisted_edge = EdgeId::new(parse("ledger"), parse("wide"), 0);
    let only_ledger =
        Step::new(StepId::new("only"), "Only").with_item(StepItem::Element(parse("ledger")));
    let loose_edges = BTreeMap::from([(unlisted_edge, Edge::new())]);
    let loose = Workflow::new(WorkflowId::new("loose"), "Loose")
        .with_edges(loose_edges)
        .with_step(only_ledger);
    let _loose = builder.add_workflow(loose);
    let report = rejected_report(builder);

    let rendered = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("report is JSON");
    let pretty = serde_json::to_string_pretty(&parsed).expect("re-serializes");

    insta::assert_snapshot!(pretty);
}

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

fn main_step(builder: &mut ModelBuilder) -> (WorkflowRef, StepRef) {
    let main_workflow = Workflow::new(WorkflowId::new("main"), "Main");
    let main = builder.add_workflow(main_workflow);
    let all_step = Step::new(StepId::new("all"), "All");
    let all = builder.add_step(&main, all_step);
    (main, all)
}

fn cluster_overridden_by(cluster_override: ElementOverride, member_count: usize) -> ModelBuilder {
    let mut builder = ModelBuilder::new();
    let cluster = builder.add_top_level("cluster", Element::new("cluster"));
    let (main, all) = main_step(&mut builder);
    builder.list_element(&all, &cluster);
    for member_index in 0..member_count {
        let member_id = format!("m{member_index}");
        let member = builder.add_child(&cluster, &member_id, Element::new("member"));
        builder.list_element(&all, &member);
    }
    builder.override_element(&main, &cluster, cluster_override);
    builder
}

fn report_of_listed_top_level(element: Element) -> ModelReport {
    let mut builder = ModelBuilder::new();
    let api = builder.add_top_level("api", element);
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &api);
    rejected_report(builder)
}

fn report_of_link(link: &str) -> ModelReport {
    let mut builder = ModelBuilder::new();
    let backend = builder.add_top_level("backend", Element::new("backend"));
    let api = builder.add_child(&backend, "api", Element::new("API"));
    let link_property = ComponentProperties::new().with_link(link);
    let linked = Element::new("docs").with_properties(link_property);
    let docs = builder.add_top_level("docs", linked);
    let (_main, all) = main_step(&mut builder);
    builder.list_element(&all, &api);
    builder.list_element(&all, &docs);
    admitted_report(builder)
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
