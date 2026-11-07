use std::collections::BTreeSet;

use svugy_model::builder::ModelBuilder;
use svugy_model::disclose::DisclosedModel;
use svugy_model::model::{Edge, EdgeId, Element};

#[test]
fn each_layer_opens_one_more_depth_and_lifts_what_its_closed_containers_hide() {
    let mut builder = ModelBuilder::new();
    let desk = builder.add_top_level("desk", Element::new("desk"));
    let engine = builder.add_child(&desk, "engine", Element::new("engine"));
    let book = builder.add_child(&engine, "book", Element::new("book"));
    let ledger = builder.add_top_level("ledger", Element::new("ledger"));
    let store = builder.add_child(&ledger, "store", Element::new("store"));
    let settle = builder.add_edge(&engine, &store, Edge::new().with_label("settle"));
    let audit_badge = Edge::new().with_label("audit").with_badge();
    let audit = builder.add_edge(&book, &store, audit_badge);
    let teller = builder.add_top_level("teller", Element::new("teller"));
    let deposit_badge = Edge::new().with_label("deposit").with_badge();
    let deposit = builder.add_edge(&teller, &store, deposit_badge);
    let model = builder.build().model.expect("valid model");
    let desk_path = desk.path().clone();
    let engine_path = engine.path().clone();
    let ledger_path = ledger.path().clone();
    let store_path = store.path().clone();
    let teller_path = teller.path().clone();

    let layers = DisclosedModel::every_layer_of(&model);

    let [tops_closed, engine_closed, whole] = layers.as_slice() else {
        panic!("the deepest container, desk.engine, sits at depth 2, so layers 0 to 2 exist");
    };
    let desk_to_ledger = EdgeId::new(desk_path.clone(), ledger_path.clone(), 0);
    let teller_to_ledger = EdgeId::new(teller_path, ledger_path.clone(), 0);
    let desk_and_ledger = BTreeSet::from([desk_path, ledger_path]);
    let settle_and_audit = BTreeSet::from([settle.clone(), audit.clone()]);
    let tops_closed_model = tops_closed.model();
    let tops_closed_edges: Vec<&EdgeId> = tops_closed_model.edges().keys().collect();
    let desk_to_ledger_edge = tops_closed_model.edge(&desk_to_ledger);
    let desk_to_ledger_label = desk_to_ledger_edge.and_then(Edge::label);
    let desk_to_ledger_origins = &tops_closed.lifted_origins()[&desk_to_ledger];
    let teller_to_ledger_edge = tops_closed_model.edge(&teller_to_ledger);
    let is_teller_to_ledger_arc = teller_to_ledger_edge.is_some_and(|edge| !edge.is_badge());
    assert_eq!(tops_closed.closed_containers(), &desk_and_ledger);
    assert_eq!(tops_closed_edges, [&desk_to_ledger, &teller_to_ledger]);
    assert_eq!(desk_to_ledger_label, None);
    assert_eq!(desk_to_ledger_origins, &settle_and_audit);
    assert!(
        is_teller_to_ledger_arc,
        "a badge whose target is lifted onto a closed container is drawn as an arc, never as a mini of the whole container"
    );

    let lifted_audit = EdgeId::new(engine_path.clone(), store_path, 1);
    let engine_alone = BTreeSet::from([engine_path]);
    let audit_alone = BTreeSet::from([audit]);
    let engine_closed_model = engine_closed.model();
    let engine_closed_edges: Vec<&EdgeId> = engine_closed_model.edges().keys().collect();
    let lifted_audit_edge = engine_closed_model.edge(&lifted_audit);
    let lifted_audit_label = lifted_audit_edge.and_then(Edge::label);
    let is_lifted_audit_badge = lifted_audit_edge.is_some_and(Edge::is_badge);
    let lifted_audit_origins = &engine_closed.lifted_origins()[&lifted_audit];
    assert_eq!(engine_closed.closed_containers(), &engine_alone);
    assert_eq!(engine_closed_edges, [&settle, &lifted_audit, &deposit]);
    assert_eq!(lifted_audit_label, Some("audit"));
    assert!(
        is_lifted_audit_badge,
        "a badge whose source is lifted onto a closed container stays a badge, its target's mini on the closed container"
    );
    assert_eq!(lifted_audit_origins, &audit_alone);

    assert_eq!(whole.model(), &model);
    assert!(whole.closed_containers().is_empty());
}
