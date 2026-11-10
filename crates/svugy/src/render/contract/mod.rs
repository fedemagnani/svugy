//! Define the data contract between rendering and the runtime.

mod annotation;

pub(crate) use annotation::{ContractAnnotator, DiagramDetails};

/// Names every attribute the runtime reads from a rendered document, spelled once for both sides; crates/svugy/runtime/CONTRACT.md gives their meaning.
pub(crate) struct RuntimeContract;

impl RuntimeContract {
    /// Version the contract, bumped by any rename, removal or change of meaning.
    pub(crate) const VERSION: &'static str = "2";

    /// Mark the root with the contract version.
    pub(crate) const CONTRACT: &'static str = "data-svugy-contract";
    /// Mark a layer's group with its disclosure depth.
    pub(crate) const LAYER: &'static str = "data-svugy-layer";
    /// Mark the layer a viewer sees, rendered on the deepest layer and moved to layer 0 when the runtime starts.
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
    /// Mark a stub marker's group with its external endpoint.
    pub(crate) const STUB: &'static str = "data-svugy-stub";
    /// Mark a group with the level a click on it goes to.
    pub(crate) const GOTO: &'static str = "data-svugy-goto";
    /// Mark a level's overlay, shown only while that level is entered.
    pub(crate) const OVERLAY: &'static str = "data-svugy-overlay";
    /// Mark the group of the diagram's title and subtitle, a click on which shows the diagram's description.
    pub(crate) const HEADING: &'static str = "data-svugy-heading";
    /// Mark a group's details JSON, or the root's diagram details JSON.
    pub(crate) const DETAILS: &'static str = "data-svugy-details";
    /// Names the runtime-owned marker of the entered level, which rendering never emits.
    #[cfg(test)]
    pub(crate) const ENTERED: &'static str = "data-svugy-entered";
    /// Names the runtime-owned marker of a group the hovered one dims, which rendering never emits.
    #[cfg(test)]
    pub(crate) const DIMMED: &'static str = "data-svugy-dimmed";

    /// Lists the attributes rendering emits; `ENTERED` and `DIMMED` are left out because only the runtime sets them.
    #[cfg(test)]
    pub(crate) const RENDERED_ATTRIBUTES: [&'static str; 17] = [
        Self::CONTRACT,
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

    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element, Model, Traveler};

    use super::RuntimeContract;
    use crate::render::profile::Runtime;
    use crate::{InteractiveSvg, RenderSvg, StaticSvg};

    const CDATA_END: &str = "]]>";

    /// Build a titled diagram of a `desk` container holding `api` and `db`, and a `client` linked to `desk.api` calling into it, so every contract group appears.
    fn desk_with_a_linked_client() -> Model {
        let mut builder = ModelBuilder::new().with_title("desk");
        let desk = builder.add_top_level("desk", Element::new("desk"));
        let api = builder.add_child(&desk, "api", Element::new("api"));
        let db = builder.add_child(&desk, "db", Element::new("db"));
        let client_element = Element::new("client").with_link("desk.api");
        let client = builder.add_top_level("client", client_element);
        let calls = Edge::new().with_label("calls");
        builder.add_edge(&client, &api, calls);
        builder.add_edge(&api, &db, Edge::new());
        builder.build().model.expect("valid model")
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
            let runtime_owned_attributes = [Self::ENTERED, Self::DIMMED];
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

    #[test]
    fn runtime_sources_mention_every_contract_attribute() {
        let unknown_attributes = RuntimeContract::attributes_unknown_to_runtime();
        assert_eq!(unknown_attributes, Vec::<&str>::new());
    }

    #[test]
    fn interactive_document_carries_every_rendered_contract_attribute() {
        let model = desk_with_a_linked_client();
        let markup = InteractiveSvg::render(&model);
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
        let client_entering_desk = "<g data-svugy-element=\"client\" data-svugy-goto=\"desk\">";
        assert!(markup.contains(client_entering_desk), "{markup}");
    }

    #[test]
    fn an_envelope_edge_and_a_default_edge_each_move_their_own_traveler_only_live() {
        let mut builder = ModelBuilder::new();
        let client = builder.add_top_level("client", Element::new("client"));
        let api = builder.add_top_level("api", Element::new("api"));
        let db = builder.add_top_level("db", Element::new("db"));
        let mailing = Edge::new().with_traveler(Traveler::Envelope);
        builder.add_edge(&client, &api, mailing);
        builder.add_edge(&api, &db, Edge::new());
        let model = builder.build().model.expect("valid model");

        let interactive_markup = InteractiveSvg::render(&model);
        let static_markup = StaticSvg::render(&model);

        let (symbol_ids, traveler_uses) = symbol_ids_and_traveler_uses(&interactive_markup);
        let (static_symbol_ids, static_traveler_uses) =
            symbol_ids_and_traveler_uses(&static_markup);
        let envelope_use = ("envelope".to_owned(), "#traveler-envelope".to_owned());
        let dot_use = ("dot".to_owned(), "#traveler-dot".to_owned());
        assert_eq!(symbol_ids, ["traveler-dot", "traveler-envelope"]);
        assert_eq!(traveler_uses, [dot_use, envelope_use]);
        assert_eq!(static_symbol_ids, Vec::<String>::new());
        assert_eq!(static_traveler_uses, Vec::<(String, String)>::new());
    }

    #[test]
    fn descriptions_reach_the_interactive_details_and_never_the_static_document() {
        let mut builder = ModelBuilder::new().with_description("diagram **story**");
        let shop_element = Element::new("shop").with_description("shop story");
        let shop = builder.add_top_level("shop", shop_element);
        let ledger = builder.add_top_level("ledger", Element::new("ledger"));
        let posting = Edge::new().with_description("posting story");
        builder.add_edge(&shop, &ledger, posting);
        let model = builder.build().model.expect("valid model");

        let interactive_markup = InteractiveSvg::render(&model);
        let static_markup = StaticSvg::render(&model);

        let diagram_carrier = "<desc data-svugy-details=\"\">{\"kind\":\"diagram\",\"description\":\"diagram **story**\"}</desc>";
        assert!(
            interactive_markup.contains(diagram_carrier),
            "{interactive_markup}"
        );
        for story in ["diagram **story**", "shop story", "posting story"] {
            assert!(
                interactive_markup.contains(story),
                "interactive misses {story}"
            );
            assert!(!static_markup.contains(story), "static carries {story}");
        }
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
}
