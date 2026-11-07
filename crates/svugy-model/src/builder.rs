//! Build a model through element handles, so ancestors and edge endpoints exist by construction.

use crate::legend::Legend;
use crate::model::{Edge, EdgeId, Element, UncheckedModel};
use crate::path::ElementPath;
use crate::validate::ModelAdmission;

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
        self.unchecked.content.title = Some(title);
        self
    }

    /// Subtitle the diagram, as its top-level view shows it under the title.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        let subtitle = subtitle.into();
        self.unchecked.content.subtitle = Some(subtitle);
        self
    }

    /// Give the diagram a legend, as its top-level view shows it top-right.
    pub fn with_legend(mut self, legend: Legend) -> Self {
        self.unchecked.content.legend = Some(legend);
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
        let elements = &mut self.unchecked.content.elements;
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
        let edges = &mut self.unchecked.content.edges;
        let from_path = from.path.clone();
        let to_path = to.path.clone();
        let first_parallel_id = EdgeId::new(from_path, to_path, 0);
        let last_parallel_id = first_parallel_id.parallel(u32::MAX);
        let parallel_ids = &first_parallel_id..=&last_parallel_id;
        let mut parallel_edges = edges.range::<EdgeId, _>(parallel_ids);
        let last_parallel_edge = parallel_edges.next_back();
        let ordinal = last_parallel_edge.map_or(0, |(id, _edge)| id.ordinal() + 1);
        let id = first_parallel_id.parallel(ordinal);
        let inserted_id = id.clone();
        edges.insert(inserted_id, edge);
        id
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
