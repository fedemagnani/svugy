use std::collections::BTreeSet;

use svugy_model::builder::{ModelBuilder, StepRef, WorkflowRef};
use svugy_model::model::{
    Edge, EdgeId, Element, Model, PlacementHint, Position, ProjectedWorkflow,
};
use svugy_model::path::ElementPath;
use svugy_model::problem::Problem;
use svugy_model::properties::ComponentProperties;
use svugy_model::workflow::{ElementOverride, Step, StepId, StepIndex, Workflow, WorkflowId};

#[test]
fn each_workflow_projects_the_graph_its_steps_show_with_its_own_edges_and_overrides() {
    let mut builder = ModelBuilder::new();
    let pool = builder.add_top_level("pool", Element::new("Pool"));
    let subscription = builder.add_child(&pool, "subscription", Element::new("Subscription"));
    let worker_element = Element::new("Worker").with_caption("tokio task");
    let worker = builder.add_top_level("worker", worker_element);
    let commands = builder.add_child(&worker, "commands", Element::new("Commands"));
    let seeder = builder.add_child(&worker, "seeder", Element::new("Seeder"));
    let yellowstone = builder.add_top_level("yellowstone", Element::new("Yellowstone"));
    let rpc = builder.add_top_level("rpc", Element::new("RPC"));
    let fetcher = builder.add_child(&rpc, "fetcher", Element::new("Fetcher"));
    let subscribe_workflow = Workflow::new(WorkflowId::new("subscribe"), "A pool subscribes");
    let subscribe = builder.add_workflow(subscribe_workflow);
    let place = add_step(&mut builder, &subscribe, "place");
    let retarget = builder.add_edge(
        &place,
        &subscription,
        &commands,
        Edge::new().with_label("Retarget"),
    );
    let seed = add_step(&mut builder, &subscribe, "seed");
    let _filter = builder.add_edge(&seed, &commands, &yellowstone, Edge::new());
    builder.list_element(&seed, &seeder);
    builder.list_element(&seed, &fetcher);
    let ack = add_step(&mut builder, &subscribe, "ack");
    builder.list_edge(&ack, retarget.id().clone());
    let column = ElementOverride::new().with_child_placement(PlacementHint::Column);
    builder.override_element(&subscribe, &worker, column);
    let failover_workflow = Workflow::new(WorkflowId::new("failover"), "A stream fails over");
    let failover = builder.add_workflow(failover_workflow);
    let detect = add_step(&mut builder, &failover, "detect");
    let _drop = builder.add_edge(&detect, &yellowstone, &commands, Edge::new());
    let failover_retarget = builder.add_edge(&detect, &subscription, &commands, Edge::new());
    builder.list_element(&detect, &rpc);
    let restarting = ComponentProperties::new().with_caption("restarting");
    let pinned_worker = ElementOverride::new()
        .with_properties(restarting)
        .with_position(Position { x: 420.0, y: 180.0 });
    builder.override_element(&failover, &worker, pinned_worker);
    let model = builder.build().model.expect("valid model");
    let subscribe_id = WorkflowId::new("subscribe");
    let failover_id = WorkflowId::new("failover");

    let subscribe_projection = shown_projection(&model, &subscribe_id);
    let failover_projection = shown_projection(&model, &failover_id);

    let subscribe_graph = subscribe_projection.graph();
    let failover_graph = failover_projection.graph();

    let subscribe_paths: Vec<&ElementPath> = subscribe_graph.elements().keys().collect();
    let failover_paths: Vec<&ElementPath> = failover_graph.elements().keys().collect();
    let place_seed_ack = BTreeSet::from([StepIndex::new(0), StepIndex::new(1), StepIndex::new(2)]);
    let place_and_ack = BTreeSet::from([StepIndex::new(0), StepIndex::new(2)]);
    let failover_worker = failover_graph.element(worker.path()).expect("shown");
    assert_eq!(
        subscribe_paths,
        [
            pool.path(),
            subscription.path(),
            rpc.path(),
            fetcher.path(),
            worker.path(),
            commands.path(),
            seeder.path(),
            yellowstone.path()
        ],
        "a workflow shows what its steps list, edge ends and bare components alike, plus their ancestors"
    );
    assert_eq!(
        failover_paths,
        [
            pool.path(),
            subscription.path(),
            rpc.path(),
            worker.path(),
            commands.path(),
            yellowstone.path()
        ],
        "a component no step of the workflow lists is left out of it"
    );
    assert_eq!(
        retarget.id(),
        failover_retarget.id(),
        "each workflow numbers its own edges from 0"
    );
    assert_ne!(
        retarget, failover_retarget,
        "an edge handle tells the workflows numbering the same id apart"
    );
    assert_eq!(subscribe_graph.steps_of_edge(retarget.id()), &place_and_ack);
    assert_eq!(
        subscribe_graph.steps_of_element(worker.path()),
        &place_seed_ack
    );
    assert_eq!(
        subscribe_graph.child_placement(worker.path()),
        Some(PlacementHint::Column)
    );
    assert_eq!(subscribe_graph.position(worker.path()), None);
    assert_eq!(
        failover_graph.position(worker.path()),
        Some(Position { x: 420.0, y: 180.0 })
    );
    let failover_worker_properties = failover_worker.properties();
    assert_eq!(failover_worker_properties.caption(), Some("restarting"));
    assert!(subscribe_graph.is_container(rpc.path()));
    assert!(
        !failover_graph.is_container(rpc.path()),
        "a container none of whose children the workflow shows is a leaf there"
    );
}

#[test]
fn edges_marked_badge_are_badges_and_every_other_edge_an_arc() {
    let mut builder = ModelBuilder::new();
    let desk = builder.add_top_level("desk", Element::new("desk"));
    let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
    let store = builder.add_child(&desk, "sqlite", Element::new("SQLite"));
    let app = builder.add_top_level("app", Element::new("app"));
    let main_workflow = Workflow::new(WorkflowId::new("main"), "Main");
    let main = builder.add_workflow(main_workflow);
    let all = add_step(&mut builder, &main, "all");
    let writes = builder.add_edge(&all, &quoter, &store, Edge::new().with_badge());
    let app_writes = builder.add_edge(&all, &app, &store, Edge::new().with_badge());
    let reads = builder.add_edge(&all, &store, &quoter, Edge::new());
    let model = builder.build().model.expect("valid model");
    let main_id = WorkflowId::new("main");
    let projection = shown_projection(&model, &main_id);
    let graph = projection.graph();

    let quoter_badges: Vec<_> = graph.badge_edges_from(quoter.path()).collect();
    let badges: Vec<_> = graph.badge_edges().collect();
    let arcs: Vec<_> = graph.arc_edges().map(|(id, _edge)| id).collect();

    assert!(graph.is_badge_edge(writes.id()));
    assert!(!graph.is_badge_edge(reads.id()));
    assert_eq!(quoter_badges, [writes.id()]);
    assert_eq!(badges, [app_writes.id(), writes.id()]);
    assert_eq!(arcs, [reads.id()]);
}

#[test]
fn an_edge_is_homed_in_the_lowest_common_ancestor_of_its_endpoints() {
    let between_siblings = edge_id("a.b", "a.c");
    let between_cousins = edge_id("a.b.c", "a.d.e");
    let between_unrelated = edge_id("a", "b.c");

    assert_eq!(
        between_siblings.home_level(),
        parse("a"),
        "siblings: their parent"
    );
    assert_eq!(
        between_cousins.home_level(),
        parse("a"),
        "cousins: their lowest common ancestor"
    );
    assert_eq!(
        between_unrelated.home_level(),
        ElementPath::root(),
        "unrelated: the root"
    );
}

#[test]
fn children_are_one_level_down_in_path_order() {
    let mut builder = ModelBuilder::new();
    let container = builder.add_top_level("a", Element::new("a"));
    let second = builder.add_child(&container, "c", Element::new("c"));
    let first = builder.add_child(&container, "b", Element::new("b"));
    let grandchild = builder.add_child(&first, "deep", Element::new("deep"));
    let other = builder.add_top_level("z", Element::new("z"));
    let main_workflow = Workflow::new(WorkflowId::new("main"), "Main");
    let main = builder.add_workflow(main_workflow);
    let all = add_step(&mut builder, &main, "all");
    for listed in [&second, &grandchild, &other] {
        builder.list_element(&all, listed);
    }
    let model = builder.build().model.expect("valid model");
    let main_id = WorkflowId::new("main");
    let projection = shown_projection(&model, &main_id);
    let graph = projection.graph();
    let root = ElementPath::root();

    let children: Vec<&ElementPath> = graph
        .children(container.path())
        .map(|(path, _element)| path)
        .collect();
    let top_level: Vec<&ElementPath> = graph.children(&root).map(|(path, _element)| path).collect();

    assert_eq!(children, [first.path(), second.path()]);
    assert_eq!(top_level, [container.path(), other.path()]);
}

#[test]
fn a_duplicate_declaration_fails_the_build_instead_of_overwriting() {
    let mut builder = ModelBuilder::new();
    let first = builder.add_top_level("api", Element::new("first"));
    let _second = builder.add_top_level("api", Element::new("second"));
    let main_workflow = Workflow::new(WorkflowId::new("main"), "Main");
    let main = builder.add_workflow(main_workflow);
    let all = add_step(&mut builder, &main, "all");
    builder.list_element(&all, &first);

    let admission = builder.build();

    let problems = admission.report.problems();
    assert!(admission.model.is_none());
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(&problems[0], Problem::DuplicateElement(duplicate) if duplicate.element == parse("api"))
    );
}

#[test]
fn a_handle_from_another_builder_is_reported_as_dangling() {
    let mut other_builder = ModelBuilder::new();
    let foreign = other_builder.add_top_level("backend", Element::new("backend"));
    let mut builder = ModelBuilder::new();
    let local = builder.add_top_level("frontend", Element::new("frontend"));
    let main_workflow = Workflow::new(WorkflowId::new("main"), "Main");
    let main = builder.add_workflow(main_workflow);
    let all = add_step(&mut builder, &main, "all");
    let _edge = builder.add_edge(&all, &local, &foreign, Edge::new());

    let admission = builder.build();

    let problems = admission.report.problems();
    assert!(admission.model.is_none());
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(&problems[0], Problem::DanglingEdgeEndpoint(dangling) if dangling.endpoint == parse("backend"))
    );
}

fn add_step(builder: &mut ModelBuilder, workflow: &WorkflowRef, id: &str) -> StepRef {
    let step = Step::new(StepId::new(id), id);
    builder.add_step(workflow, step)
}

fn shown_projection<'m>(model: &'m Model, id: &WorkflowId) -> ProjectedWorkflow<'m> {
    let shown_workflow = model.showing(id).expect("the workflow was added");
    shown_workflow.projected()
}

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

fn edge_id(from: &str, to: &str) -> EdgeId {
    EdgeId::new(parse(from), parse(to), 0)
}
