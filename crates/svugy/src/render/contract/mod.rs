//! Define the data contract between rendering and the runtime.

mod annotation;

pub(crate) use annotation::{ContractAnnotator, WorkflowDetails};

/// Names every attribute the runtime reads from a rendered document, spelled once for both sides; crates/svugy/runtime/CONTRACT.md gives their meaning.
pub(crate) struct RuntimeContract;

impl RuntimeContract {
    /// Version the contract, bumped by any rename, removal or change of meaning.
    pub(crate) const VERSION: &'static str = "4";

    /// Mark the root with the contract version.
    pub(crate) const CONTRACT: &'static str = "data-svugy-contract";
    /// Mark a workflow's group with its id.
    pub(crate) const WORKFLOW: &'static str = "data-svugy-workflow";
    /// Mark a camera's group with the angle it sees the diagram from.
    pub(crate) const CAMERA: &'static str = "data-svugy-camera";
    /// Mark a layer's group with its disclosure depth.
    pub(crate) const LAYER: &'static str = "data-svugy-layer";
    /// Mark the workflow, camera and layer a viewer sees, rendered on the shown workflow, the author's camera and every camera's deepest layer, the layer moved to layer 0 when the runtime starts.
    pub(crate) const SHOWN: &'static str = "data-svugy-shown";
    /// Mark a level group with its level path.
    pub(crate) const LEVEL: &'static str = "data-svugy-level";
    /// Mark a level group with its view in document coordinates.
    pub(crate) const VIEW: &'static str = "data-svugy-view";
    /// Mark a solid's group with its element path.
    pub(crate) const ELEMENT: &'static str = "data-svugy-element";
    /// Mark the group of a container its layer draws closed.
    pub(crate) const CLOSED: &'static str = "data-svugy-closed";
    /// Mark a connection or badge group with its edge.
    pub(crate) const EDGE: &'static str = "data-svugy-edge";
    /// Mark a connection or badge group with the endpoint its layer draws it from.
    pub(crate) const FROM: &'static str = "data-svugy-from";
    /// Mark a connection or badge group with the endpoint its layer draws it to.
    pub(crate) const TO: &'static str = "data-svugy-to";
    /// Mark a connection's stroke, whose dash offset the runtime animates as flow.
    pub(crate) const FLOW: &'static str = "data-svugy-flow";
    /// Mark a face's hachure strokes, which shade it but never take pointer events or highlights.
    pub(crate) const HACHURE: &'static str = "data-svugy-hachure";
    /// Mark a hatched face's fill paths, its canvas underlay and shade wash, which a highlight never restyles.
    pub(crate) const FACE_FILL: &'static str = "data-svugy-face-fill";
    /// Mark a connection's moving traveler with its name.
    pub(crate) const TRAVELER: &'static str = "data-svugy-traveler";
    /// Mark an element, edge or stub group with the ids of the workflow's steps it takes part in.
    pub(crate) const STEPS: &'static str = "data-svugy-steps";
    /// Mark a stub marker's group with its external endpoint.
    pub(crate) const STUB: &'static str = "data-svugy-stub";
    /// Mark a group with the level a click on it goes to.
    pub(crate) const GOTO: &'static str = "data-svugy-goto";
    /// Mark a level's overlay, shown only while that level is entered.
    pub(crate) const OVERLAY: &'static str = "data-svugy-overlay";
    /// Mark the group of the workflow's title and subtitle, which opens the workflow menu.
    pub(crate) const HEADING: &'static str = "data-svugy-heading";
    /// Mark a group's details JSON, or a workflow group's workflow details JSON.
    pub(crate) const DETAILS: &'static str = "data-svugy-details";
    /// Names the runtime-owned marker of the entered level, which rendering never emits.
    #[cfg(test)]
    pub(crate) const ENTERED: &'static str = "data-svugy-entered";
    /// Names the runtime-owned marker of a group the focused step or the hovered group dims, which rendering never emits.
    #[cfg(test)]
    pub(crate) const DIMMED: &'static str = "data-svugy-dimmed";
    /// Names the runtime-owned marker of the groups drawing the selected item, which rendering never emits.
    #[cfg(test)]
    pub(crate) const SELECTED: &'static str = "data-svugy-selected";

    /// Lists the attributes rendering emits; `ENTERED`, `DIMMED` and `SELECTED` are left out because only the runtime sets them.
    #[cfg(test)]
    pub(crate) const RENDERED_ATTRIBUTES: [&'static str; 20] = [
        Self::CONTRACT,
        Self::WORKFLOW,
        Self::CAMERA,
        Self::LAYER,
        Self::SHOWN,
        Self::LEVEL,
        Self::VIEW,
        Self::ELEMENT,
        Self::CLOSED,
        Self::EDGE,
        Self::FROM,
        Self::TO,
        Self::FLOW,
        Self::TRAVELER,
        Self::STEPS,
        Self::STUB,
        Self::GOTO,
        Self::OVERLAY,
        Self::HEADING,
        Self::DETAILS,
    ];
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::iter;

    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element, Model, Traveler};
    use svugy_model::properties::ComponentProperties;
    use svugy_model::workflow::{Step, StepId, Workflow, WorkflowId};

    use super::RuntimeContract;
    use crate::one_step::OneStepModelBuilder;
    use crate::render::profile::Runtime;
    use crate::{InteractiveSvg, RenderSvg, StaticSvg};

    const CDATA_END: &str = "]]>";

    #[test]
    fn every_workflow_draws_its_own_group_whose_groups_name_the_steps_they_take_part_in() {
        let mut builder = ModelBuilder::new();
        let shop = builder.add_top_level("shop", Element::new("shop"));
        let api = builder.add_child(&shop, "api", Element::new("api"));
        let ledger = builder.add_top_level("ledger", Element::new("ledger"));
        let checkout_id = WorkflowId::new("checkout");
        let checkout_workflow = Workflow::new(checkout_id, "A customer pays");
        let checkout = builder.add_workflow(checkout_workflow);
        let order_id = StepId::new("order");
        let order_step = Step::new(order_id, "The shop takes the order");
        let order = builder.add_step(&checkout, order_step);
        builder.list_element(&order, &api);
        let charge_id = StepId::new("charge");
        let charge_step = Step::new(charge_id, "The ledger records it");
        let charge = builder.add_step(&checkout, charge_step);
        let posts = Edge::new().with_label("posts");
        builder.add_edge(&charge, &api, &ledger, posts);
        let refund_id = WorkflowId::new("refund");
        let refund_workflow = Workflow::new(refund_id, "A customer is refunded");
        let refunding = builder.add_workflow(refund_workflow);
        let refund_step_id = StepId::new("refund");
        let refund_step = Step::new(refund_step_id, "The ledger reverses it");
        let refund = builder.add_step(&refunding, refund_step);
        let reverses = Edge::new();
        builder.add_edge(&refund, &ledger, &api, reverses);
        let model = builder.build().model.expect("valid model");
        let refund_workflow = model
            .showing_each_workflow()
            .nth(1)
            .expect("the second workflow refunds");

        let markup = InteractiveSvg::render(refund_workflow);

        let hidden_checkout = "<g data-svugy-workflow=\"checkout\">";
        let shown_refund = "<g data-svugy-shown=\"\" data-svugy-workflow=\"refund\">";
        let api_in_both_steps =
            "<g data-svugy-element=\"shop.api\" data-svugy-steps=\"order charge\">";
        let ledger_in_charge = "<g data-svugy-element=\"ledger\" data-svugy-steps=\"charge\">";
        let closed_shop_in_both_steps = "<g data-svugy-closed=\"\" data-svugy-element=\"shop\" data-svugy-steps=\"order charge\">";
        let lifted_post_in_charge = "data-svugy-steps=\"charge\" data-svugy-to=\"ledger\">";
        let order_details = "{\"id\":\"order\",\"title\":\"The shop takes the order\",\"layer\":1,\"items\":[{\"element\":\"shop.api\"}]}";
        for expected in [
            hidden_checkout,
            shown_refund,
            api_in_both_steps,
            ledger_in_charge,
            closed_shop_in_both_steps,
            lifted_post_in_charge,
            order_details,
        ] {
            assert!(markup.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn descriptions_reach_the_interactive_details_numbered_in_their_workflow_and_never_the_static_document()
     {
        let mut builder = ModelBuilder::new();
        let shop_element = Element::new("shop").with_description("shop story");
        let shop = builder.add_top_level("shop", shop_element);
        let ledger = builder.add_top_level("ledger", Element::new("ledger"));
        let main_id = WorkflowId::new("main");
        let main_workflow = Workflow::new(main_id, "main").with_description("workflow **story**");
        let main = builder.add_workflow(main_workflow);
        let posting_id = StepId::new("post");
        let posting_step = Step::new(posting_id, "post").with_description("step story");
        let post = builder.add_step(&main, posting_step);
        let posting = Edge::new().with_description("posting story, see [the post](#shop->ledger)");
        builder.add_edge(&post, &shop, &ledger, posting);
        let model = builder.build().model.expect("valid model");
        let main_workflow = model.showing_first_workflow();

        let interactive_markup = InteractiveSvg::render(main_workflow);
        let static_markup = StaticSvg::render(main_workflow);

        let workflow_carrier = "<desc data-svugy-details=\"\">{\"kind\":\"workflow\",\"id\":\"main\",\"title\":\"main\",\"description\":\"workflow **story**\"";
        assert!(
            interactive_markup.contains(workflow_carrier),
            "{interactive_markup}"
        );
        let numbered_link = "[the post](#shop-&gt;ledger[0])";
        for story in [
            "workflow **story**",
            "step story",
            "shop story",
            numbered_link,
        ] {
            assert!(
                interactive_markup.contains(story),
                "interactive misses {story}"
            );
            assert!(!static_markup.contains(story), "static carries {story}");
        }
    }

    #[test]
    fn runtime_sources_mention_every_contract_attribute() {
        let unknown_attributes = RuntimeContract::attributes_unknown_to_runtime();
        assert_eq!(unknown_attributes, Vec::<&str>::new());
    }

    #[test]
    fn interactive_document_carries_every_rendered_contract_attribute() {
        let model = desk_with_a_linked_client();
        let main_workflow = model.showing_first_workflow();
        let markup = InteractiveSvg::render(main_workflow);
        let options = usvg::Options::default();
        let parse_result = usvg::Tree::from_str(&markup, &options);
        assert!(parse_result.is_ok(), "{parse_result:?}");
        let attribute_names = attribute_names_of(&markup);
        let missing_attributes: Vec<&str> = RuntimeContract::RENDERED_ATTRIBUTES
            .into_iter()
            .filter(|attribute| !attribute_names.contains(*attribute))
            .collect();
        assert_eq!(missing_attributes, Vec::<&str>::new());
        let is_entered_rendered = attribute_names.contains(RuntimeContract::ENTERED);
        assert!(!is_entered_rendered, "only the runtime enters a level");
        let client_entering_desk =
            "<g data-svugy-element=\"client\" data-svugy-goto=\"desk\" data-svugy-steps=\"main\">";
        assert!(markup.contains(client_entering_desk), "{markup}");
    }

    #[test]
    fn an_envelope_edge_and_a_default_edge_each_move_their_own_traveler_live_from_every_camera() {
        let mut builder = OneStepModelBuilder::new();
        let client = builder.add_top_level("client", Element::new("client"));
        let api = builder.add_top_level("api", Element::new("api"));
        let db = builder.add_top_level("db", Element::new("db"));
        let mailing = Edge::new().with_traveler(Traveler::Envelope);
        builder.add_edge(&client, &api, mailing);
        builder.add_edge(&api, &db, Edge::new());
        let model = builder.build();
        let main_workflow = model.showing_first_workflow();

        let interactive_markup = InteractiveSvg::render(main_workflow);
        let static_markup = StaticSvg::render(main_workflow);

        let (symbol_ids, traveler_uses) = symbol_ids_and_traveler_uses(&interactive_markup);
        let (static_symbol_ids, static_traveler_uses) =
            symbol_ids_and_traveler_uses(&static_markup);
        let envelope_use = ("envelope".to_owned(), "#traveler-envelope".to_owned());
        let dot_use = ("dot".to_owned(), "#traveler-dot".to_owned());
        let camera_count = interactive_markup.matches("<g data-svugy-camera=").count();
        let dot_uses = iter::repeat_n(dot_use, camera_count);
        let envelope_uses = iter::repeat_n(envelope_use, camera_count);
        let expected_traveler_uses: Vec<(String, String)> = dot_uses.chain(envelope_uses).collect();
        assert_eq!(symbol_ids, ["traveler-dot", "traveler-envelope"]);
        assert_eq!(traveler_uses, expected_traveler_uses);
        assert_eq!(static_symbol_ids, Vec::<String>::new());
        assert_eq!(static_traveler_uses, Vec::<(String, String)>::new());
    }

    #[test]
    fn runtime_sources_never_close_their_cdata_wrapper() {
        let is_script_free_of_cdata_end = !Runtime::SCRIPT.contains(CDATA_END);
        let is_style_free_of_cdata_end = !Runtime::STYLE.contains(CDATA_END);
        assert!(
            is_script_free_of_cdata_end,
            "runtime/svugy.js contains {CDATA_END}"
        );
        assert!(
            is_style_free_of_cdata_end,
            "runtime/svugy.css contains {CDATA_END}"
        );
    }

    /// Build a `desk` container holding `api` and `db`, and a `client` linked to `desk.api` calling into it, so every contract group appears.
    fn desk_with_a_linked_client() -> Model {
        let mut builder = OneStepModelBuilder::new();
        let desk = builder.add_top_level("desk", Element::new("desk"));
        let api = builder.add_child(&desk, "api", Element::new("api"));
        let db = builder.add_child(&desk, "db", Element::new("db"));
        let desk_api_link = ComponentProperties::new().with_link("desk.api");
        let client_element = Element::new("client").with_properties(desk_api_link);
        let client = builder.add_top_level("client", client_element);
        let calls = Edge::new().with_label("calls");
        builder.add_edge(&client, &api, calls);
        builder.add_edge(&api, &db, Edge::new());
        builder.build()
    }

    /// Collect the name of every attribute any element of `markup` carries.
    fn attribute_names_of(markup: &str) -> BTreeSet<String> {
        let document = roxmltree::Document::parse(markup).expect("well-formed XML");
        document
            .descendants()
            .flat_map(|node| node.attributes())
            .map(|attribute| attribute.name().to_owned())
            .collect()
    }

    /// Collect the id of every `<symbol>` and the traveler name and reference of every `<use>` in `markup`, the uses sorted by name.
    fn symbol_ids_and_traveler_uses(markup: &str) -> (Vec<String>, Vec<(String, String)>) {
        let document = roxmltree::Document::parse(markup).expect("well-formed XML");
        let elements: Vec<roxmltree::Node> = document
            .descendants()
            .filter(roxmltree::Node::is_element)
            .collect();
        let symbol_ids = elements
            .iter()
            .filter(|node| node.has_tag_name("symbol"))
            .filter_map(|symbol| symbol.attribute("id"))
            .map(str::to_owned)
            .collect();
        let mut traveler_uses: Vec<(String, String)> = elements
            .iter()
            .filter(|node| node.has_tag_name("use"))
            .map(|traveler_use| {
                let name_attribute = traveler_use.attribute(RuntimeContract::TRAVELER);
                let href_attribute = traveler_use.attribute("href");
                let traveler_name = name_attribute.unwrap_or_default().to_owned();
                let symbol_reference = href_attribute.unwrap_or_default().to_owned();
                (traveler_name, symbol_reference)
            })
            .collect();
        traveler_uses.sort();
        (symbol_ids, traveler_uses)
    }

    impl RuntimeContract {
        /// Lists the contract attributes that neither runtime source mentions.
        fn attributes_unknown_to_runtime() -> Vec<&'static str> {
            let runtime_owned_attributes = [Self::ENTERED, Self::DIMMED, Self::SELECTED];
            let runtime_attributes = Self::RENDERED_ATTRIBUTES
                .into_iter()
                .chain(runtime_owned_attributes);
            runtime_attributes
                .filter(|attribute| {
                    let is_missing_from_script = !Runtime::SCRIPT.contains(attribute);
                    let is_missing_from_style = !Runtime::STYLE.contains(attribute);
                    is_missing_from_script && is_missing_from_style
                })
                .collect()
        }
    }
}
