use svugy_model::builder::ModelBuilder;
use svugy_model::model::{Edge, EdgeId, Element};
use svugy_model::path::ElementPath;
use svugy_model::validate::Problem;

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

fn edge_id(from: &str, to: &str) -> EdgeId {
    EdgeId::new(parse(from), parse(to), 0)
}

#[test]
fn home_level_of_siblings_is_their_parent() {
    let id = edge_id("a.b", "a.c");

    assert_eq!(id.home_level(), parse("a"));
}

#[test]
fn home_level_of_cousins_is_their_lowest_common_ancestor() {
    let id = edge_id("a.b.c", "a.d.e");

    assert_eq!(id.home_level(), parse("a"));
}

#[test]
fn home_level_of_unrelated_top_level_elements_is_the_root() {
    let id = edge_id("a", "b.c");

    assert_eq!(id.home_level(), ElementPath::root());
}

#[test]
fn home_level_of_a_self_loop_is_its_nodes_parent() {
    let nested_loop = edge_id("a.b", "a.b");
    let top_level_loop = edge_id("a", "a");

    assert_eq!(nested_loop.home_level(), parse("a"));
    assert_eq!(top_level_loop.home_level(), ElementPath::root());
}

#[test]
fn home_level_of_an_ancestor_edge_is_the_ancestor() {
    let id = edge_id("a", "a.b.c");

    assert_eq!(id.home_level(), parse("a"));
}

#[test]
fn layoutable_edges_leave_out_only_ancestor_and_descendant_edges() {
    let mut builder = ModelBuilder::new();
    let container = builder.add_top_level("desk", Element::new("desk"));
    let quoter = builder.add_child(&container, "quoter", Element::new("quoter"));
    let hedge = builder.add_child(&container, "hedge", Element::new("hedge desk"));
    let browser = builder.add_top_level("browser", Element::new("Browser"));
    let sibling_edge = builder.add_edge(&quoter, &hedge, Edge::new());
    let cross_edge = builder.add_edge(&hedge, &browser, Edge::new());
    let self_loop = builder.add_edge(&hedge, &hedge, Edge::new());
    let _down_edge = builder.add_edge(&container, &quoter, Edge::new());
    let _up_edge = builder.add_edge(&hedge, &container, Edge::new());
    let model = builder.build().model.expect("valid model");

    let layoutable: Vec<&EdgeId> = model.layoutable_edges().map(|(id, _edge)| id).collect();

    let mut expected = vec![&sibling_edge, &cross_edge, &self_loop];
    expected.sort();
    assert_eq!(layoutable, expected);
}

#[test]
fn parallel_edges_between_the_same_pair_stay_distinct() {
    let mut builder = ModelBuilder::new();
    let client = builder.add_top_level("client", Element::new("client"));
    let server = builder.add_top_level("server", Element::new("server"));
    let request = builder.add_edge(&client, &server, Edge::new().with_label("request"));
    let response = builder.add_edge(&client, &server, Edge::new().with_label("response"));
    let reverse = builder.add_edge(&server, &client, Edge::new().with_label("push"));
    let model = builder.build().model.expect("valid model");

    let request_label = model.edge(&request).and_then(Edge::label);
    let response_label = model.edge(&response).and_then(Edge::label);
    assert_eq!(request.ordinal(), 0);
    assert_eq!(response.ordinal(), 1);
    assert_eq!(reverse.ordinal(), 0);
    assert_eq!(request_label, Some("request"));
    assert_eq!(response_label, Some("response"));
    assert_eq!(model.edges().len(), 3);
}

#[test]
fn an_element_is_a_container_iff_it_has_children() {
    let mut builder = ModelBuilder::new();
    let container = builder.add_top_level("a", Element::new("a"));
    let child = builder.add_child(&container, "b", Element::new("b"));
    let _lookalike = builder.add_top_level("ab", Element::new("ab"));
    let model = builder.build().model.expect("valid model");

    assert!(model.is_container(container.path()));
    assert!(!model.is_container(child.path()));
    assert!(!model.is_container(&parse("ab")));
}

#[test]
fn children_are_one_level_down_in_path_order() {
    let mut builder = ModelBuilder::new();
    let container = builder.add_top_level("a", Element::new("a"));
    let second = builder.add_child(&container, "c", Element::new("c"));
    let first = builder.add_child(&container, "b", Element::new("b"));
    let _grandchild = builder.add_child(&first, "deep", Element::new("deep"));
    let other = builder.add_top_level("z", Element::new("z"));
    let model = builder.build().model.expect("valid model");
    let root = ElementPath::root();

    let children: Vec<&ElementPath> = model
        .children(container.path())
        .map(|(path, _element)| path)
        .collect();
    let top_level: Vec<&ElementPath> = model.children(&root).map(|(path, _element)| path).collect();

    assert_eq!(children, [first.path(), second.path()]);
    assert_eq!(top_level, [container.path(), other.path()]);
}

#[test]
fn a_duplicate_declaration_fails_the_build_instead_of_overwriting() {
    let mut builder = ModelBuilder::new();
    let _first = builder.add_top_level("api", Element::new("first"));
    let _second = builder.add_top_level("api", Element::new("second"));

    let admission = builder.build();

    let problems = admission.report.problems();
    assert!(admission.model.is_none());
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(&problems[0], Problem::DuplicateElement(duplicate) if duplicate.path == parse("api"))
    );
}

#[test]
fn a_handle_from_another_builder_is_reported_as_dangling() {
    let mut other_builder = ModelBuilder::new();
    let foreign = other_builder.add_top_level("backend", Element::new("backend"));
    let mut builder = ModelBuilder::new();
    let local = builder.add_top_level("frontend", Element::new("frontend"));
    let _edge = builder.add_edge(&local, &foreign, Edge::new());

    let admission = builder.build();

    let problems = admission.report.problems();
    assert!(admission.model.is_none());
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(&problems[0], Problem::DanglingEdgeEndpoint(dangling) if dangling.endpoint == parse("backend"))
    );
}
