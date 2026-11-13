//! Load the self-portrait, examples/svugy/svugy.md, as the model unit tests run against.

use svugy_model::graph::WorkflowGraph;
use svugy_model::model::Model;
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::markdown::MarkdownLanguage;

const SVUGY_MD: &str = include_str!("../../../examples/svugy/svugy.md");

/// Golden source the unit tests share: svugy drawn by svugy.
pub(crate) struct SvugyExample;

impl SvugyExample {
    /// Parse examples/svugy/svugy.md into its admitted model.
    pub(crate) fn model() -> Model {
        let import = MarkdownLanguage::import(SVUGY_MD);
        let import_parts = import.into_admitted_parts();
        import_parts
            .model
            .expect("examples/svugy/svugy.md is a valid diagram")
    }

    /// Project the self-portrait's first workflow into the graph layout and geometry read.
    pub(crate) fn graph() -> WorkflowGraph {
        let model = Self::model();
        let first_workflow = model.showing_first_workflow();
        let projection = first_workflow.projected();
        projection.into_graph()
    }
}
