//! Define the data contract between rendering and the runtime.

mod annotation;

pub(crate) use annotation::ContractAnnotator;

/// Names every attribute the runtime reads from a rendered document, spelled once for both sides; docs/src/reference/runtime-contract.md gives their meaning.
pub(crate) struct RuntimeContract;

impl RuntimeContract {
    /// Version the contract, bumped by any rename, removal or change of meaning.
    pub(crate) const VERSION: &'static str = "1";

    /// Mark the root with the contract version.
    pub(crate) const CONTRACT: &'static str = "data-svugy-contract";
    /// Mark a level group with its level path.
    pub(crate) const LEVEL: &'static str = "data-svugy-level";
    /// Mark a level group with its view in document coordinates.
    pub(crate) const VIEW: &'static str = "data-svugy-view";
    /// Mark a solid's group with its element path.
    pub(crate) const ELEMENT: &'static str = "data-svugy-element";
    /// Mark a connection or badge group with its edge.
    pub(crate) const EDGE: &'static str = "data-svugy-edge";
    /// Mark a connection's stroke, whose dash offset the runtime animates as flow.
    pub(crate) const FLOW: &'static str = "data-svugy-flow";
    /// Mark a connection's animated dot.
    pub(crate) const DOT: &'static str = "data-svugy-dot";
    /// Mark a stub marker's group with its external endpoint.
    pub(crate) const STUB: &'static str = "data-svugy-stub";
    /// Mark a group with the level a click on it goes to.
    pub(crate) const GOTO: &'static str = "data-svugy-goto";
    /// Mark a level's overlay, shown only while that level is entered.
    pub(crate) const OVERLAY: &'static str = "data-svugy-overlay";
    /// Mark a group's details JSON.
    pub(crate) const DETAILS: &'static str = "data-svugy-details";
    /// Names the runtime-owned marker of the entered level, which rendering never emits.
    #[cfg(test)]
    pub(crate) const ENTERED: &'static str = "data-svugy-entered";

    /// Lists the attributes rendering emits; `ENTERED` is left out because only the runtime sets it.
    #[cfg(test)]
    pub(crate) const RENDERED_ATTRIBUTES: [&'static str; 11] = [
        Self::CONTRACT,
        Self::LEVEL,
        Self::VIEW,
        Self::ELEMENT,
        Self::EDGE,
        Self::FLOW,
        Self::DOT,
        Self::STUB,
        Self::GOTO,
        Self::OVERLAY,
        Self::DETAILS,
    ];
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element, Model};

    use super::RuntimeContract;
    use crate::InteractiveSvg;
    use crate::render::profile::Interactive;

    const CDATA_END: &str = "]]>";

    /// Build a `desk` container holding `api` and `db`, and a `client` linked to `desk.api` calling into it, so every contract group appears.
    fn desk_with_a_linked_client() -> Model {
        let mut builder = ModelBuilder::new();
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

    impl RuntimeContract {
        /// Lists the contract attributes that neither runtime source mentions.
        fn attributes_unknown_to_runtime() -> Vec<&'static str> {
            let runtime_attributes = Self::RENDERED_ATTRIBUTES.into_iter().chain([Self::ENTERED]);
            runtime_attributes
                .filter(|attribute| {
                    let is_missing_from_script = !Interactive::RUNTIME_SCRIPT.contains(attribute);
                    let is_missing_from_style = !Interactive::RUNTIME_STYLE.contains(attribute);
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
    fn runtime_sources_never_close_their_cdata_wrapper() {
        let is_script_free_of_cdata_end = !Interactive::RUNTIME_SCRIPT.contains(CDATA_END);
        let is_style_free_of_cdata_end = !Interactive::RUNTIME_STYLE.contains(CDATA_END);
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
