//! Build a model through element handles, so ancestors and edge endpoints exist by construction.

use crate::legend::Legend;
use crate::model::{Edge, EdgeId, Element};
use crate::path::ElementPath;
use crate::validate::{ModelAdmission, UncheckedModel};

/// Grow a model through element handles, then admit it through every check.
#[derive(Debug, Default)]
pub struct ModelBuilder {
    unchecked: UncheckedModel,
}

impl ModelBuilder {
    /// Create a new builder holding an empty model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Title the diagram, as its top-level view shows it top-left.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.unchecked.model.title = Some(title);
        self
    }

    /// Subtitle the diagram, as its top-level view shows it under the title.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        let subtitle = subtitle.into();
        self.unchecked.model.subtitle = Some(subtitle);
        self
    }

    /// Describe the diagram in Markdown, shown in the side panel while nothing is focused.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.unchecked.model.description = Some(description);
        self
    }

    /// Give the diagram a legend, as its top-level view shows it top-right.
    pub fn with_legend(mut self, legend: Legend) -> Self {
        self.unchecked.model.legend = Some(legend);
        self
    }

    /// Add `element` as a top-level element named `id`, returning its handle.
    pub fn add_top_level(&mut self, id: &str, element: Element) -> ElementRef {
        let path = ElementPath::root();
        let root = ElementRef { path };
        self.add_child(&root, id, element)
    }

    /// Add `element` as a child named `id` of `parent`, keeping the first declaration and recording any later one as a duplicate.
    pub fn add_child(&mut self, parent: &ElementRef, id: &str, element: Element) -> ElementRef {
        let path = parent.path.child(id);
        let elements = &mut self.unchecked.model.elements;
        let is_duplicate = elements.contains_key(&path);
        if is_duplicate {
            let duplicate_path = path.clone();
            let duplicate_declaration = (duplicate_path, element);
            self.unchecked
                .duplicate_declarations
                .push(duplicate_declaration);
            return ElementRef { path };
        }
        let inserted_path = path.clone();
        elements.insert(inserted_path, element);
        ElementRef { path }
    }

    /// Add an edge with the next free ordinal for its endpoint pair, so parallel edges stay distinct.
    pub fn add_edge(&mut self, from: &ElementRef, to: &ElementRef, edge: Edge) -> EdgeId {
        let from_path = from.path.clone();
        let to_path = to.path.clone();
        self.unchecked
            .model
            .insert_edge_with_next_free_ordinal(from_path, to_path, edge)
    }

    /// Admit the built model unless a check finds an error, with the report of every problem found, warnings included.
    pub fn build(self) -> ModelAdmission {
        self.unchecked.admit()
    }
}

/// Handle to an element already added to a builder, the only way to name a parent or an edge endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementRef {
    path: ElementPath,
}

impl ElementRef {
    /// Return the path of the element this handle names.
    pub fn path(&self) -> &ElementPath {
        &self.path
    }
}
