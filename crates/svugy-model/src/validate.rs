//! Validate a model and report what the author must fix, with did-you-mean suggestions.

use std::collections::BTreeSet;
use std::fmt;

use miette::{Diagnostic, JSONReportHandler, Severity, SourceSpan};

use crate::legend::DashKey;
use crate::link::Link;
use crate::model::{Element, MAX_MAGNITUDE, Model, PlacementHint, Shape};
use crate::path::ElementPath;
use crate::problem::{
    AncestorDescendantEdge, BadgeEdgeOnContainer, ChildPlacementWithoutChildren,
    DanglingEdgeEndpoint, DuplicateElement, EmptyGridHint, ErrorSeverity, IgnoredContainerHeight,
    IgnoredContainerShape, ImpossibleHeight, LegendKey, MalformedLegendDash, MismatchedStyleValue,
    MissingAncestor, Problem, StyleOwner, Suggestion, UnboundedPosition, UncountablePackedTracks,
    UndrawnLegendKey, UnknownStyleKey, UnresolvedLink,
};
use crate::style::AnyStyleKey;

/// Assemble a model whose references are not yet checked, recording duplicate declarations; only `admit` hands it out.
#[derive(Debug, Default)]
pub(crate) struct UncheckedModel {
    pub(crate) model: Model,
    pub(crate) duplicate_declarations: Vec<(ElementPath, Element)>,
}

impl UncheckedModel {
    /// Hold `model` unchecked, with no duplicate declaration recorded.
    pub(crate) fn new(model: Model) -> Self {
        Self {
            model,
            duplicate_declarations: Vec::new(),
        }
    }

    /// Admit the model only when no check holds an error.
    pub(crate) fn admit(self) -> ModelAdmission {
        let report = self.validate();
        let has_errors = report.has_errors();
        if has_errors {
            return ModelAdmission {
                model: None,
                report,
            };
        }
        let admitted_model = Some(self.model);
        ModelAdmission {
            model: admitted_model,
            report,
        }
    }

    /// Run every semantic check and report each problem found, errors and warnings alike.
    fn validate(&self) -> ModelReport {
        let model = &self.model;
        let duplicate_elements = self.find_duplicate_elements();
        let missing_ancestors = model.find_missing_ancestors();
        let dangling_endpoints = model.find_dangling_edge_endpoints();
        let ancestor_descendant_edges = model.find_ancestor_descendant_edges();
        let style_problems = model.find_style_problems();
        let unbounded_positions = model.find_unbounded_positions();
        let placement_problems = model.find_impossible_placement_hints();
        let height_problems = model.find_impossible_heights();
        let ignored_container_bodies = model.find_ignored_container_bodies();
        let badge_edges_on_containers = model.find_badge_edges_on_containers();
        let unresolved_links = model.find_unresolved_links();
        let legend_problems = model.find_legend_problems();
        let problems = duplicate_elements
            .into_iter()
            .chain(missing_ancestors)
            .chain(dangling_endpoints)
            .chain(ancestor_descendant_edges)
            .chain(style_problems)
            .chain(unbounded_positions)
            .chain(placement_problems)
            .chain(height_problems)
            .chain(ignored_container_bodies)
            .chain(badge_edges_on_containers)
            .chain(unresolved_links)
            .chain(legend_problems)
            .collect();
        ModelReport { problems }
    }

    /// Report each later declaration of an element already declared, in declaration order.
    fn find_duplicate_elements(&self) -> Vec<Problem> {
        let duplicate_declarations = self.duplicate_declarations.iter();
        let problems = duplicate_declarations.map(|(path, element)| {
            let span = element.source_span();
            let duplicate_path = path.clone();
            let duplicate_element = DuplicateElement {
                element: duplicate_path,
                span,
            };
            Problem::DuplicateElement(duplicate_element)
        });
        problems.collect()
    }
}

/// Admit a model unless its report holds an error, handing out that full report either way, warnings included.
#[derive(Debug)]
pub struct ModelAdmission {
    pub model: Option<Model>,
    pub report: ModelReport,
}

impl Model {
    /// Report each missing ancestor once, against the first element (in path order) that needs it.
    fn find_missing_ancestors(&self) -> Vec<Problem> {
        let elements = self.elements();
        let mut reported_ancestors = BTreeSet::new();
        let mut problems = Vec::new();
        for (path, element) in elements {
            for ancestor in path.ancestors() {
                let is_present = elements.contains_key(&ancestor);
                if is_present {
                    continue;
                }
                let reported_ancestor = ancestor.clone();
                let is_already_reported = !reported_ancestors.insert(reported_ancestor);
                if is_already_reported {
                    continue;
                }
                let suggestion = self.suggest_closest_element(&ancestor);
                let span = element.source_span();
                let descendant_path = path.clone();
                let missing_ancestor = MissingAncestor {
                    element: descendant_path,
                    ancestor,
                    suggestion,
                    span,
                };
                let problem = Problem::MissingAncestor(missing_ancestor);
                problems.push(problem);
            }
        }
        problems
    }

    /// Report each edge endpoint that is not an element, a self-loop's shared endpoint only once.
    fn find_dangling_edge_endpoints(&self) -> Vec<Problem> {
        let elements = self.elements();
        let mut problems = Vec::new();
        for (id, edge) in self.edges() {
            let from = id.from();
            let to = id.to();
            let is_self_loop = from == to;
            let endpoints = if is_self_loop {
                vec![from]
            } else {
                vec![from, to]
            };
            for endpoint in endpoints {
                let is_present = elements.contains_key(endpoint);
                if is_present {
                    continue;
                }
                let suggestion = self.suggest_closest_element(endpoint);
                let span = edge.source_span();
                let edge_id = id.clone();
                let dangling_path = endpoint.clone();
                let dangling_endpoint = DanglingEdgeEndpoint {
                    edge: edge_id,
                    endpoint: dangling_path,
                    suggestion,
                    span,
                };
                let problem = Problem::DanglingEdgeEndpoint(dangling_endpoint);
                problems.push(problem);
            }
        }
        problems
    }

    /// Report each edge joining an element to its own ancestor or descendant, which layout leaves out.
    fn find_ancestor_descendant_edges(&self) -> Vec<Problem> {
        let ancestor_descendant_edges = self
            .edges()
            .iter()
            .filter(|(id, _edge)| id.joins_ancestor_and_descendant());
        let problems = ancestor_descendant_edges.map(|(id, edge)| {
            let span = edge.source_span();
            let edge_id = id.clone();
            let ancestor_descendant_edge = AncestorDescendantEdge {
                edge: edge_id,
                span,
            };
            Problem::AncestorDescendantEdge(ancestor_descendant_edge)
        });
        problems.collect()
    }

    /// Report each style entry, on elements then edges, whose key is not v1 or whose value does not fit its key.
    fn find_style_problems(&self) -> Vec<Problem> {
        let element_styles = self.elements().iter().map(|(path, element)| {
            let owner_path = path.clone();
            let owner = StyleOwner::Element(owner_path);
            let style = element.style();
            let span = element.source_span();
            (owner, style, span)
        });
        let edge_styles = self.edges().iter().map(|(id, edge)| {
            let owner_id = id.clone();
            let owner = StyleOwner::Edge(owner_id);
            let style = edge.style();
            let span = edge.source_span();
            (owner, style, span)
        });
        let mut problems = Vec::new();
        for (owner, style, span) in element_styles.chain(edge_styles) {
            for (key, value) in style {
                let style_problem = owner.check_style_entry(key, value, span);
                problems.extend(style_problem);
            }
        }
        problems
    }

    /// Report each pinned position that is not a finite point within `MAX_MAGNITUDE` of its level's origin.
    fn find_unbounded_positions(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let problems =
            elements.filter_map(|(path, element)| element.check_position_is_bounded(path));
        problems.collect()
    }

    /// Report each placement hint no layout can honour: an empty or uncountable grid, a hint with no children.
    fn find_impossible_placement_hints(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (path, element) in self.elements() {
            let empty_grid = element.check_grid_has_columns(path);
            let uncountable_tracks = self.check_packed_tracks_are_countable(path, element);
            let placement_without_children = self.check_child_placement_has_children(path, element);
            problems.extend(empty_grid);
            problems.extend(uncountable_tracks);
            problems.extend(placement_without_children);
        }
        problems
    }

    /// Check that the element's packing hint, if any, lays its children in tracks a packed level can count.
    fn check_packed_tracks_are_countable(
        &self,
        path: &ElementPath,
        element: &Element,
    ) -> Option<Problem> {
        let child_placement = element.child_placement()?;
        let child_count = self.children(path).count();
        let (columns, rows) = child_placement.packed_tracks(child_count)?;
        let max_tracks = PlacementHint::MAX_PACKED_TRACKS as usize;
        let is_column_count_countable = columns <= max_tracks;
        let is_row_count_countable = rows <= max_tracks;
        let is_countable = is_column_count_countable && is_row_count_countable;
        if is_countable {
            return None;
        }
        let span = element.source_span();
        let packed_path = path.clone();
        let uncountable_tracks = UncountablePackedTracks {
            element: packed_path,
            child_count,
            columns,
            rows,
            span,
        };
        let problem = Problem::UncountablePackedTracks(uncountable_tracks);
        Some(problem)
    }

    /// Check that the element's child placement hint, if any, has children to place.
    fn check_child_placement_has_children(
        &self,
        path: &ElementPath,
        element: &Element,
    ) -> Option<Problem> {
        let child_placement = element.child_placement()?;
        let has_children = self.is_container(path);
        if has_children {
            return None;
        }
        let span = element.source_span();
        let childless_path = path.clone();
        let placement_without_children = ChildPlacementWithoutChildren {
            element: childless_path,
            child_placement,
            span,
        };
        let problem = Problem::ChildPlacementWithoutChildren(placement_without_children);
        Some(problem)
    }

    /// Report each authored height that is not a positive number within `MAX_MAGNITUDE`, NaN and infinities included.
    fn find_impossible_heights(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let problems = elements.filter_map(|(path, element)| element.check_height_is_bounded(path));
        problems.collect()
    }

    /// Report each container's authored body, a shape other than a slab or a height, which v1 ignores since every container is an enterable slab.
    fn find_ignored_container_bodies(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let containers = elements.filter(|(path, _element)| self.is_container(path));
        let mut problems = Vec::new();
        for (path, element) in containers {
            let ignored_shape = element.check_container_shape_is_slab(path);
            let ignored_height = element.check_container_height_is_unset(path);
            problems.extend(ignored_shape);
            problems.extend(ignored_height);
        }
        problems
    }

    /// Report each badge edge with a container at either end, since a badge needs a leaf at both.
    fn find_badge_edges_on_containers(&self) -> Vec<Problem> {
        let edges = self.edges().iter();
        let badge_edges = edges.filter(|(_id, edge)| edge.is_badge());
        let badge_edges_on_containers = badge_edges.filter(|(id, _edge)| {
            let from = id.from();
            let to = id.to();
            let is_from_container = self.is_container(from);
            let is_to_container = self.is_container(to);
            is_from_container || is_to_container
        });
        let problems = badge_edges_on_containers.map(|(id, edge)| {
            let span = edge.source_span();
            let edge_id = id.clone();
            let badge_edge_on_container = BadgeEdgeOnContainer {
                edge: edge_id,
                span,
            };
            Problem::BadgeEdgeOnContainer(badge_edge_on_container)
        });
        problems.collect()
    }

    /// Report each link shaped like an element path that names no element, the likely typo of a drill-down target.
    fn find_unresolved_links(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let problems =
            elements.filter_map(|(path, element)| self.check_link_resolves(path, element));
        problems.collect()
    }

    /// Check that the element's link, if shaped like an element path, names an element.
    fn check_link_resolves(&self, path: &ElementPath, element: &Element) -> Option<Problem> {
        let raw_link = element.link()?;
        let link = Link::resolve(raw_link, self);
        let target = link.path_shaped_url()?;
        let suggestion = self.suggest_closest_element(&target);
        let link_span = element.link_span().map(SourceSpan::from);
        let span = link_span.or_else(|| element.source_span());
        let unresolved_raw_link = raw_link.to_owned();
        let linking_path = path.clone();
        let unresolved_link = UnresolvedLink {
            element: linking_path,
            link: unresolved_raw_link,
            suggestion,
            span,
        };
        let problem = Problem::UnresolvedLink(unresolved_link);
        Some(problem)
    }

    /// Report each legend dash key that is not a dash length's shortest spelling, then each key row the legend describes but the diagram never draws.
    fn find_legend_problems(&self) -> Vec<Problem> {
        let Some(legend) = self.legend() else {
            return Vec::new();
        };
        let dash_keys = legend.dashes().keys();
        let malformed_dashes = dash_keys
            .clone()
            .filter(|dash_key| dash_key.is_malformed())
            .map(|dash_key| {
                let malformed_key = dash_key.clone();
                let malformed_dash = MalformedLegendDash {
                    dash: malformed_key,
                };
                Problem::MalformedLegendDash(malformed_dash)
            });
        let undrawn_dashes =
            dash_keys.filter_map(|dash_key| self.check_dash_key_is_drawn(dash_key));
        let badge_targets = legend.badges().keys();
        let undrawn_badges =
            badge_targets.filter_map(|target| self.check_badge_key_is_drawn(target));
        malformed_dashes
            .chain(undrawn_dashes)
            .chain(undrawn_badges)
            .collect()
    }

    /// Report a dash key spelling a dash length no arc is drawn with.
    fn check_dash_key_is_drawn(&self, dash_key: &DashKey) -> Option<Problem> {
        let stroke_dash = dash_key.stroke_dash()?;
        let mut arc_edges = self.arc_edges();
        let is_drawn = arc_edges.any(|(_id, edge)| edge.stroke_dash() == stroke_dash);
        if is_drawn {
            return None;
        }
        let undrawn_dash = dash_key.clone();
        let key = LegendKey::Dash(undrawn_dash);
        let undrawn_key = UndrawnLegendKey {
            key,
            suggestion: None,
        };
        let problem = Problem::UndrawnLegendKey(undrawn_key);
        Some(problem)
    }

    /// Report a badge key no badge edge points to, suggesting the closest target a badge edge points to.
    fn check_badge_key_is_drawn(&self, target: &ElementPath) -> Option<Problem> {
        let mut badge_edges = self.badge_edges();
        let is_drawn = badge_edges.any(|id| id.to() == target);
        if is_drawn {
            return None;
        }
        let target_text = target.to_string();
        let badge_targets = self.badge_edges().map(|id| id.to().clone());
        let suggestion = Suggestion::closest_to(&target_text, badge_targets);
        let undrawn_target = target.clone();
        let key = LegendKey::Badge(undrawn_target);
        let undrawn_key = UndrawnLegendKey { key, suggestion };
        let problem = Problem::UndrawnLegendKey(undrawn_key);
        Some(problem)
    }

    /// Suggest the declared element whose path is closest to `unknown`.
    fn suggest_closest_element(&self, unknown: &ElementPath) -> Option<Suggestion<ElementPath>> {
        let unknown_text = unknown.to_string();
        let known_paths = self.elements().keys().cloned();
        Suggestion::closest_to(&unknown_text, known_paths)
    }
}

impl StyleOwner {
    /// Check one style entry of this owner: its key must be v1 and its value must fit that key.
    fn check_style_entry(
        &self,
        key: &str,
        value: &str,
        span: Option<SourceSpan>,
    ) -> Option<Problem> {
        let Some(style_key) = AnyStyleKey::find(key) else {
            let known_names = AnyStyleKey::ALL.iter().map(AnyStyleKey::name);
            let suggestion = Suggestion::closest_to(key, known_names);
            let unknown_key = key.to_owned();
            let unknown_key_owner = self.clone();
            let unknown_style_key = UnknownStyleKey {
                owner: unknown_key_owner,
                key: unknown_key,
                suggestion,
                span,
            };
            let problem = Problem::UnknownStyleKey(unknown_style_key);
            return Some(problem);
        };
        let expected = style_key.value_kind();
        let is_fitting_value = expected.accepts(value);
        if is_fitting_value {
            return None;
        }
        let mismatched_value = value.to_owned();
        let mismatched_value_owner = self.clone();
        let mismatched_style_value = MismatchedStyleValue {
            owner: mismatched_value_owner,
            key: style_key,
            value: mismatched_value,
            expected,
            span,
        };
        let problem = Problem::MismatchedStyleValue(mismatched_style_value);
        Some(problem)
    }
}

impl Element {
    /// Check that this element's pinned position, if any, lies within `MAX_MAGNITUDE` of its level's origin.
    fn check_position_is_bounded(&self, path: &ElementPath) -> Option<Problem> {
        let position = self.position()?;
        let is_x_bounded = position.x.abs() <= MAX_MAGNITUDE;
        let is_y_bounded = position.y.abs() <= MAX_MAGNITUDE;
        let is_bounded = is_x_bounded && is_y_bounded;
        if is_bounded {
            return None;
        }
        let span = self.source_span();
        let unbounded_path = path.clone();
        let unbounded_position = UnboundedPosition {
            element: unbounded_path,
            position,
            span,
        };
        let problem = Problem::UnboundedPosition(unbounded_position);
        Some(problem)
    }

    /// Check that this element's grid hint, if any, has at least one column.
    fn check_grid_has_columns(&self, path: &ElementPath) -> Option<Problem> {
        let Some(PlacementHint::Grid { columns: 0 }) = self.child_placement() else {
            return None;
        };
        let span = self.source_span();
        let grid_path = path.clone();
        let empty_grid = EmptyGridHint {
            element: grid_path,
            span,
        };
        let problem = Problem::EmptyGridHint(empty_grid);
        Some(problem)
    }

    /// Check that this element's authored height, if any, is positive and within `MAX_MAGNITUDE`.
    fn check_height_is_bounded(&self, path: &ElementPath) -> Option<Problem> {
        let height = self.height()?;
        let is_positive = height > 0.0;
        let is_within_magnitude = height <= MAX_MAGNITUDE;
        let is_bounded = is_positive && is_within_magnitude;
        if is_bounded {
            return None;
        }
        let span = self.source_span();
        let height_path = path.clone();
        let impossible_height = ImpossibleHeight {
            element: height_path,
            height,
            span,
        };
        let problem = Problem::ImpossibleHeight(impossible_height);
        Some(problem)
    }

    /// Check that this container's authored shape, if any, is the slab it is drawn as.
    fn check_container_shape_is_slab(&self, path: &ElementPath) -> Option<Problem> {
        let shape = self.shape()?;
        let is_slab = shape == Shape::Slab;
        if is_slab {
            return None;
        }
        let span = self.source_span();
        let container_path = path.clone();
        let ignored_shape = IgnoredContainerShape {
            element: container_path,
            shape,
            span,
        };
        let problem = Problem::IgnoredContainerShape(ignored_shape);
        Some(problem)
    }

    /// Check that this container sets no height, which v1 ignores.
    fn check_container_height_is_unset(&self, path: &ElementPath) -> Option<Problem> {
        let height = self.height()?;
        let span = self.source_span();
        let container_path = path.clone();
        let ignored_height = IgnoredContainerHeight {
            element: container_path,
            height,
            span,
        };
        let problem = Problem::IgnoredContainerHeight(ignored_height);
        Some(problem)
    }
}

/// Collect a model's problems into one diagnostic whose severity is its worst problem's.
#[derive(Debug)]
pub struct ModelReport {
    problems: Vec<Problem>,
}

impl ModelReport {
    /// Tell whether any problem is an error, which is what blocks a build or a load.
    pub fn has_errors(&self) -> bool {
        self.problems.iter().any(Problem::is_error)
    }

    /// Render the report as miette's machine-readable JSON, one entry per problem with its severity.
    pub fn to_json(&self) -> String {
        let handler = JSONReportHandler::new();
        let mut rendered = String::new();
        handler
            .render_report(&mut rendered, self)
            .expect("a report renders into a string: every message is plain text");
        rendered
    }

    /// Return every problem found, in the order the checks ran.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }
}

impl fmt::Display for ModelReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_count = self
            .problems
            .iter()
            .filter(|problem| problem.is_error())
            .count();
        let warning_count = self.problems.len() - error_count;
        write!(
            formatter,
            "the model has {error_count} error(s) and {warning_count} warning(s)"
        )
    }
}

impl std::error::Error for ModelReport {}

impl Diagnostic for ModelReport {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        let code: Box<dyn fmt::Display + 'a> = Box::new("svugy::model_report");
        Some(code)
    }

    fn severity(&self) -> Option<Severity> {
        let has_errors = self.has_errors();
        if has_errors {
            return Some(Severity::Error);
        }
        let is_empty = self.problems.is_empty();
        if is_empty {
            return Some(Severity::Advice);
        }
        Some(Severity::Warning)
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        let problems = self
            .problems
            .iter()
            .map(|problem| problem as &dyn Diagnostic);
        let related: Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a> = Box::new(problems);
        Some(related)
    }
}
