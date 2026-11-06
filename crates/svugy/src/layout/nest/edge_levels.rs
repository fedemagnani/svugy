//! Relate every layoutable edge to the levels it touches: the one it is homed in and every container it leaves.

use std::collections::{BTreeMap, BTreeSet};

use svugy_model::model::{EdgeId, Model};
use svugy_model::path::ElementPath;

use super::super::route::EdgeEnd;

/// Indexes layoutable edges by level: the arcs homed in each level, and every edge, badges included, leaving each container.
pub(super) struct EdgeLevels<'a> {
    homed_edges_by_level: BTreeMap<ElementPath, Vec<&'a EdgeId>>,
    leaving_edges_by_container: BTreeMap<ElementPath, Vec<LeavingEdge<'a>>>,
}

/// Names an edge leaving a container by its end inside the container.
pub(super) struct LeavingEdge<'a> {
    edge: &'a EdgeId,
    inner_end: EdgeEnd,
}

impl<'a> EdgeLevels<'a> {
    /// Record every layoutable edge of `model` as leaving every container between its endpoints and their lowest common ancestor, and home every arc there; a badge takes no placement and no route.
    pub(super) fn of(model: &'a Model) -> Self {
        let mut homed_edges_by_level: BTreeMap<ElementPath, Vec<&'a EdgeId>> = BTreeMap::new();
        let mut leaving_edges_by_container: BTreeMap<ElementPath, Vec<LeavingEdge<'a>>> =
            BTreeMap::new();
        for (edge, _attributes) in model.layoutable_edges() {
            let home = edge.home_level();
            let home_depth = home.depth();
            for inner_end in [EdgeEnd::Source, EdgeEnd::Target] {
                let endpoint = inner_end.endpoint_of(edge);
                let left_containers = endpoint
                    .ancestors()
                    .filter(|ancestor| ancestor.depth() > home_depth);
                for container in left_containers {
                    let leaving_edge = LeavingEdge { edge, inner_end };
                    let leaving_edges = leaving_edges_by_container.entry(container).or_default();
                    leaving_edges.push(leaving_edge);
                }
            }
            let is_badge = model.is_badge_edge(edge);
            if is_badge {
                continue;
            }
            let homed_edges = homed_edges_by_level.entry(home).or_default();
            homed_edges.push(edge);
        }
        Self {
            homed_edges_by_level,
            leaving_edges_by_container,
        }
    }

    /// Return the edges homed in `level`, which it routes between their true endpoints.
    pub(super) fn homed_in(&self, level: &ElementPath) -> &[&'a EdgeId] {
        let homed_edges = self.homed_edges_by_level.get(level);
        homed_edges.map_or(&[], Vec::as_slice)
    }

    /// Lift every edge homed in `level` to the pair of its children holding the endpoints, the proxies its placement ranks.
    pub(super) fn lifted_pairs_in(
        &self,
        level: &ElementPath,
    ) -> BTreeSet<(ElementPath, ElementPath)> {
        self.homed_in(level)
            .iter()
            .map(|edge| {
                let from_child = Self::child_of_level_holding(level, edge.from());
                let to_child = Self::child_of_level_holding(level, edge.to());
                (from_child, to_child)
            })
            .collect()
    }

    /// Return the edges leaving `container`, which its entered view stubs.
    pub(super) fn leaving(&self, container: &ElementPath) -> &[LeavingEdge<'a>] {
        let leaving_edges = self.leaving_edges_by_container.get(container);
        leaving_edges.map_or(&[], Vec::as_slice)
    }

    pub(super) fn is_left_by_edges(&self, level: &ElementPath) -> bool {
        self.leaving_edges_by_container.contains_key(level)
    }

    pub(super) fn containers_left_by_edges(&self) -> impl Iterator<Item = &ElementPath> {
        self.leaving_edges_by_container.keys()
    }

    /// Return the child of `level` that is `endpoint` or holds it.
    fn child_of_level_holding(level: &ElementPath, endpoint: &ElementPath) -> ElementPath {
        let child_depth = level.depth() + 1;
        let child_segments = endpoint.segments()[..child_depth].iter().cloned();
        ElementPath::from_segments(child_segments)
    }
}

impl<'a> LeavingEdge<'a> {
    pub(super) fn edge(&self) -> &'a EdgeId {
        self.edge
    }

    pub(super) fn inner_end(&self) -> EdgeEnd {
        self.inner_end
    }

    pub(super) fn external_endpoint(&self) -> &'a ElementPath {
        let external_end = self.inner_end.opposite();
        external_end.endpoint_of(self.edge)
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;

    fn deep_edge_model() -> Model {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("a"));
        let b = builder.add_child(&a, "b", Element::new("b"));
        let c = builder.add_child(&b, "c", Element::new("c"));
        let d = builder.add_top_level("d", Element::new("d"));
        builder.add_edge(&c, &d, Edge::new());
        builder.build().model.expect("fixture model is valid")
    }

    #[test]
    fn an_edge_is_homed_at_its_lowest_common_ancestor_and_lifted_to_its_children_there() {
        let model = deep_edge_model();
        let root = ElementPath::root();

        let edge_levels = EdgeLevels::of(&model);

        assert_eq!(edge_levels.homed_in(&root).len(), 1);
        let a = ElementPath::from_segments(["a"]);
        let d = ElementPath::from_segments(["d"]);
        let lifted_pairs = edge_levels.lifted_pairs_in(&root);
        assert_eq!(lifted_pairs, BTreeSet::from([(a, d)]));
    }

    #[test]
    fn an_edge_leaves_every_container_between_its_endpoint_and_its_home() {
        let model = deep_edge_model();
        let a = ElementPath::from_segments(["a"]);
        let a_b = ElementPath::from_segments(["a", "b"]);
        let d = ElementPath::from_segments(["d"]);

        let edge_levels = EdgeLevels::of(&model);

        let left_containers: Vec<&ElementPath> = edge_levels.containers_left_by_edges().collect();
        assert_eq!(left_containers, [&a, &a_b]);
        for container in [&a, &a_b] {
            let leaving_edges = edge_levels.leaving(container);
            assert_eq!(leaving_edges.len(), 1);
            assert_eq!(*leaving_edges[0].external_endpoint(), d);
            assert_eq!(leaving_edges[0].inner_end(), EdgeEnd::Source);
        }
    }
}
