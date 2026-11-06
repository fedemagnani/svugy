//! Build a model through element handles, so ancestors and edge endpoints exist by construction.

use miette::SourceSpan;

use crate::legend::Legend;
use crate::model::{Edge, EdgeId, Element, Model};
use crate::path::ElementPath;
use crate::validate::{DuplicateElement, ModelAdmission, Problem};

/// Grow a model, recording duplicate declarations instead of overwriting them.
#[derive(Debug, Default)]
pub struct ModelBuilder {
    model: Model,
    duplicate_elements: Vec<DuplicateElement>,
}

/// Handle to an element already added to a builder, the only way to name a parent or an edge endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementRef {
    path: ElementPath,
}

impl ModelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Title the diagram, as its top-level view shows it top-left.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.model = self.model.with_title(title);
        self
    }

    /// Subtitle the diagram, as its top-level view shows it under the title.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.model = self.model.with_subtitle(subtitle);
        self
    }

    /// Give the diagram a legend, as its top-level view shows it top-right.
    pub fn with_legend(mut self, legend: Legend) -> Self {
        self.model = self.model.with_legend(legend);
        self
    }

    pub fn add_top_level(&mut self, id: &str, element: Element) -> ElementRef {
        let root = ElementPath::root();
        let path = root.child(id);
        self.insert_element(path, element)
    }

    pub fn add_child(&mut self, parent: &ElementRef, id: &str, element: Element) -> ElementRef {
        let path = parent.path.child(id);
        self.insert_element(path, element)
    }

    /// Add an edge with the next free ordinal for its endpoint pair, so parallel edges stay distinct.
    pub fn add_edge(&mut self, from: &ElementRef, to: &ElementRef, edge: Edge) -> EdgeId {
        let ordinal = self.next_ordinal(&from.path, &to.path);
        let id = EdgeId::new(from.path.clone(), to.path.clone(), ordinal);
        self.model.insert_edge(id.clone(), edge);
        id
    }

    /// Admit the built model unless a check finds an error, with the report of every problem found, warnings included.
    pub fn build(self) -> ModelAdmission {
        let duplicate_problems = self
            .duplicate_elements
            .into_iter()
            .map(Problem::DuplicateElement)
            .collect();
        self.model.admit(duplicate_problems)
    }

    /// Insert an element, keeping the first declaration and recording any later one as a duplicate.
    fn insert_element(&mut self, path: ElementPath, element: Element) -> ElementRef {
        let existing_element = self.model.element(&path);
        let is_duplicate = existing_element.is_some();
        if is_duplicate {
            let span = element.span().map(SourceSpan::from);
            let duplicate = DuplicateElement {
                path: path.clone(),
                span,
            };
            self.duplicate_elements.push(duplicate);
            return ElementRef { path };
        }
        self.model.insert_element(path.clone(), element);
        ElementRef { path }
    }

    fn next_ordinal(&self, from: &ElementPath, to: &ElementPath) -> u32 {
        let first_id = EdgeId::new(from.clone(), to.clone(), 0);
        let last_id = EdgeId::new(from.clone(), to.clone(), u32::MAX);
        let edges = self.model.edges();
        let mut parallel_edges = edges.range(first_id..=last_id);
        let last_parallel_edge = parallel_edges.next_back();
        last_parallel_edge.map_or(0, |(id, _edge)| id.ordinal() + 1)
    }
}

impl ElementRef {
    pub fn path(&self) -> &ElementPath {
        &self.path
    }
}
