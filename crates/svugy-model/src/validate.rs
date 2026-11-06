//! Validate a model and report what the author must fix, with did-you-mean suggestions.

use std::collections::BTreeSet;
use std::fmt;

use miette::{Diagnostic, JSONReportHandler, Severity, SourceSpan};
use thiserror::Error;

use crate::legend::Legend;
use crate::link::Link;
use crate::model::{ByteRange, EdgeId, Element, Model, PlacementHint, Position, Shape};
use crate::path::ElementPath;
use crate::style::{StyleKey, StyleValueKind};

/// Largest magnitude a numeric style value or a position coordinate may take, so no valid model can explode the output.
pub const MAX_MAGNITUDE: f64 = 1_000_000.0;

impl PlacementHint {
    /// Most columns or rows a packed level holds along one axis: grid lines are counted in an i16, less one.
    pub const MAX_PACKED_TRACKS: u32 = 32_766;
}

/// Admit a model unless its report holds an error, handing out that full report either way, warnings included.
#[derive(Debug)]
pub struct ModelAdmission {
    pub model: Option<Model>,
    pub report: ModelReport,
}

impl Model {
    /// Admit a model only when neither the caller's construction problems nor any check holds an error.
    pub(crate) fn admit(self, construction_problems: Vec<Problem>) -> ModelAdmission {
        let check_report = self.validate();
        let mut problems = construction_problems;
        problems.extend(check_report.problems);
        let report = ModelReport { problems };
        let has_errors = report.has_errors();
        if has_errors {
            return ModelAdmission {
                model: None,
                report,
            };
        }
        ModelAdmission {
            model: Some(self),
            report,
        }
    }

    /// Run every semantic check and report each problem found, errors and warnings alike.
    fn validate(&self) -> ModelReport {
        let missing_ancestors = self.find_missing_ancestors();
        let dangling_endpoints = self.find_dangling_edge_endpoints();
        let ancestor_descendant_edges = self.find_ancestor_descendant_edges();
        let style_problems = self.find_style_problems();
        let placement_problems = self.find_impossible_placement_hints();
        let height_problems = self.find_impossible_heights();
        let ignored_container_bodies = self.find_ignored_container_bodies();
        let arcs_into_badge_targets = self.find_arcs_from_containers_into_badge_targets();
        let unresolved_links = self.find_unresolved_links();
        let legend_problems = self.find_legend_problems();
        let problems = missing_ancestors
            .into_iter()
            .chain(dangling_endpoints)
            .chain(ancestor_descendant_edges)
            .chain(style_problems)
            .chain(placement_problems)
            .chain(height_problems)
            .chain(ignored_container_bodies)
            .chain(arcs_into_badge_targets)
            .chain(unresolved_links)
            .chain(legend_problems)
            .collect();
        ModelReport { problems }
    }

    /// Report each container's authored body, a shape other than a slab, a height or a badge mark, which v1 ignores since every container is an enterable slab.
    fn find_ignored_container_bodies(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let containers = elements.filter(|(path, _element)| self.is_container(path));
        let mut problems = Vec::new();
        for (path, element) in containers {
            let ignored_shape = Self::check_container_shape_is_slab(path, element);
            let ignored_height = Self::check_container_height_is_unset(path, element);
            let ignored_badge = Self::check_container_is_unbadged(path, element);
            problems.extend(ignored_shape);
            problems.extend(ignored_height);
            problems.extend(ignored_badge);
        }
        problems
    }

    fn check_container_shape_is_slab(path: &ElementPath, element: &Element) -> Option<Problem> {
        let shape = element.shape()?;
        let is_slab = shape == Shape::Slab;
        if is_slab {
            return None;
        }
        let span = element.span().map(SourceSpan::from);
        let ignored_shape = IgnoredContainerShape {
            element: path.clone(),
            shape,
            span,
        };
        let problem = Problem::IgnoredContainerShape(ignored_shape);
        Some(problem)
    }

    fn check_container_height_is_unset(path: &ElementPath, element: &Element) -> Option<Problem> {
        let height = element.height()?;
        let span = element.span().map(SourceSpan::from);
        let ignored_height = IgnoredContainerHeight {
            element: path.clone(),
            height,
            span,
        };
        let problem = Problem::IgnoredContainerHeight(ignored_height);
        Some(problem)
    }

    fn check_container_is_unbadged(path: &ElementPath, element: &Element) -> Option<Problem> {
        let is_unmarked = !element.is_badge_target();
        if is_unmarked {
            return None;
        }
        let span = element.span().map(SourceSpan::from);
        let ignored_badge = IgnoredContainerBadge {
            element: path.clone(),
            span,
        };
        let problem = Problem::IgnoredContainerBadge(ignored_badge);
        Some(problem)
    }

    /// Report each edge from a container into a leaf marked badge, drawn as an arc since a container's top holds its interior.
    fn find_arcs_from_containers_into_badge_targets(&self) -> Vec<Problem> {
        let edges = self.edges().iter();
        let arcs_into_badge_targets =
            edges.filter(|(id, _edge)| self.is_arc_from_container_into_badge_target(id));
        let problems = arcs_into_badge_targets.map(|(id, edge)| {
            let span = edge.span().map(SourceSpan::from);
            let arc_into_badge_target = ArcIntoBadgeTarget {
                edge: id.clone(),
                span,
            };
            Problem::ArcIntoBadgeTarget(arc_into_badge_target)
        });
        problems.collect()
    }

    fn is_arc_from_container_into_badge_target(&self, id: &EdgeId) -> bool {
        let is_badged_target = self.is_badged_leaf(id.to());
        let is_source_container = self.is_container(id.from());
        is_badged_target && is_source_container
    }

    /// Report each legend dash key that is not a dash length's shortest spelling, then each key row the legend describes but the diagram never draws.
    fn find_legend_problems(&self) -> Vec<Problem> {
        let Some(legend) = self.legend() else {
            return Vec::new();
        };
        let dash_keys = legend.dashes().keys();
        let malformed_dashes = dash_keys
            .clone()
            .filter(|dash_key| !Legend::is_dash_key(dash_key))
            .map(|dash_key| {
                let malformed_dash = MalformedLegendDash {
                    dash: dash_key.clone(),
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

    /// Report a well-formed dash key no arc is drawn with.
    fn check_dash_key_is_drawn(&self, dash_key: &str) -> Option<Problem> {
        let stroke_dash = StyleValueKind::parse_number(dash_key)?;
        let mut arc_edges = self.arc_edges();
        let is_drawn = arc_edges.any(|(_id, edge)| edge.stroke_dash() == stroke_dash);
        if is_drawn {
            return None;
        }
        let key = LegendKey::Dash(dash_key.to_owned());
        let undrawn_key = UndrawnLegendKey {
            key,
            suggestion: None,
        };
        let problem = Problem::UndrawnLegendKey(undrawn_key);
        Some(problem)
    }

    /// Report a badge key no badge edge points to, suggesting the closest element marked badge.
    fn check_badge_key_is_drawn(&self, target: &ElementPath) -> Option<Problem> {
        let mut badge_edges = self.badge_edges();
        let is_drawn = badge_edges.any(|id| id.to() == target);
        if is_drawn {
            return None;
        }
        let target_text = target.to_string();
        let elements = self.elements().iter();
        let badge_targets = elements
            .filter(|(_path, element)| element.is_badge_target())
            .map(|(path, _element)| path.to_string());
        let suggestion = Suggestion::closest_to(&target_text, badge_targets);
        let key = LegendKey::Badge(target.clone());
        let undrawn_key = UndrawnLegendKey { key, suggestion };
        let problem = Problem::UndrawnLegendKey(undrawn_key);
        Some(problem)
    }

    /// Report each authored height that is not a positive number within `MAX_MAGNITUDE`, NaN and infinities included.
    fn find_impossible_heights(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let problems =
            elements.filter_map(|(path, element)| Self::check_height_is_bounded(path, element));
        problems.collect()
    }

    fn check_height_is_bounded(path: &ElementPath, element: &Element) -> Option<Problem> {
        let height = element.height()?;
        let is_positive = height > 0.0;
        let is_within_magnitude = height <= MAX_MAGNITUDE;
        let is_bounded = is_positive && is_within_magnitude;
        if is_bounded {
            return None;
        }
        let span = element.span().map(SourceSpan::from);
        let impossible_height = ImpossibleHeight {
            element: path.clone(),
            height,
            span,
        };
        let problem = Problem::ImpossibleHeight(impossible_height);
        Some(problem)
    }

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
                let is_already_reported = reported_ancestors.contains(&ancestor);
                if is_already_reported {
                    continue;
                }
                let suggestion = self.suggest_closest_element(&ancestor);
                let span = element.span().map(SourceSpan::from);
                reported_ancestors.insert(ancestor.clone());
                let missing_ancestor = MissingAncestor {
                    element: path.clone(),
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
            let is_self_loop = id.from() == id.to();
            let endpoints = if is_self_loop {
                vec![id.from()]
            } else {
                vec![id.from(), id.to()]
            };
            for endpoint in endpoints {
                let is_present = elements.contains_key(endpoint);
                if is_present {
                    continue;
                }
                let suggestion = self.suggest_closest_element(endpoint);
                let span = edge.span().map(SourceSpan::from);
                let dangling_endpoint = DanglingEdgeEndpoint {
                    edge: id.clone(),
                    endpoint: endpoint.clone(),
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
            let span = edge.span().map(SourceSpan::from);
            let ancestor_descendant_edge = AncestorDescendantEdge {
                edge: id.clone(),
                span,
            };
            Problem::AncestorDescendantEdge(ancestor_descendant_edge)
        });
        problems.collect()
    }

    /// Report each style entry, on elements then edges, whose key is not v1 or whose value does not fit its key.
    fn find_style_problems(&self) -> Vec<Problem> {
        let element_styles = self.elements().iter().map(|(path, element)| {
            let owner = StyleOwner::Element(path.clone());
            (owner, element.style(), element.span())
        });
        let edge_styles = self.edges().iter().map(|(id, edge)| {
            let owner = StyleOwner::Edge(id.clone());
            (owner, edge.style(), edge.span())
        });
        let mut problems = Vec::new();
        for (owner, style, byte_range) in element_styles.chain(edge_styles) {
            for (key, value) in style {
                let style_problem = Self::check_style_entry(&owner, key, value, byte_range);
                problems.extend(style_problem);
            }
        }
        problems
    }

    fn check_style_entry(
        owner: &StyleOwner,
        key: &str,
        value: &str,
        byte_range: Option<ByteRange>,
    ) -> Option<Problem> {
        let span = byte_range.map(SourceSpan::from);
        let Some(style_key) = StyleKey::find(key) else {
            let known_names = StyleKey::ALL.iter().map(StyleKey::name);
            let suggestion = Suggestion::closest_to(key, known_names);
            let unknown_style_key = UnknownStyleKey {
                owner: owner.clone(),
                key: key.to_owned(),
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
        let mismatched_style_value = MismatchedStyleValue {
            owner: owner.clone(),
            key: style_key,
            value: value.to_owned(),
            expected,
            span,
        };
        let problem = Problem::MismatchedStyleValue(mismatched_style_value);
        Some(problem)
    }

    /// Report each placement hint no layout can honour: an empty or uncountable grid, a hint with no children, an unbounded position.
    fn find_impossible_placement_hints(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (path, element) in self.elements() {
            let empty_grid = Self::check_grid_has_columns(path, element);
            let uncountable_tracks = self.check_packed_tracks_are_countable(path, element);
            let placement_without_children = self.check_child_placement_has_children(path, element);
            let unbounded_position = Self::check_position_is_bounded(path, element);
            problems.extend(empty_grid);
            problems.extend(uncountable_tracks);
            problems.extend(placement_without_children);
            problems.extend(unbounded_position);
        }
        problems
    }

    fn check_grid_has_columns(path: &ElementPath, element: &Element) -> Option<Problem> {
        let Some(PlacementHint::Grid { columns: 0 }) = element.child_placement() else {
            return None;
        };
        let span = element.span().map(SourceSpan::from);
        let empty_grid = EmptyGridHint {
            element: path.clone(),
            span,
        };
        let problem = Problem::EmptyGridHint(empty_grid);
        Some(problem)
    }

    fn check_packed_tracks_are_countable(
        &self,
        path: &ElementPath,
        element: &Element,
    ) -> Option<Problem> {
        let child_placement = element.child_placement()?;
        let child_count = self.children(path).count();
        let (columns, rows) = Self::packed_tracks(child_placement, child_count)?;
        let max_tracks = PlacementHint::MAX_PACKED_TRACKS as usize;
        let is_column_count_countable = columns <= max_tracks;
        let is_row_count_countable = rows <= max_tracks;
        let is_countable = is_column_count_countable && is_row_count_countable;
        if is_countable {
            return None;
        }
        let span = element.span().map(SourceSpan::from);
        let uncountable_tracks = UncountablePackedTracks {
            element: path.clone(),
            child_count,
            columns,
            rows,
            span,
        };
        let problem = Problem::UncountablePackedTracks(uncountable_tracks);
        Some(problem)
    }

    /// Return the (columns, rows) a packing hint lays n children in, rows = ⌈n / columns⌉; `None` for layered or an empty grid.
    fn packed_tracks(child_placement: PlacementHint, child_count: usize) -> Option<(usize, usize)> {
        match child_placement {
            PlacementHint::Layered | PlacementHint::Grid { columns: 0 } => None,
            PlacementHint::Grid { columns } => {
                let column_count = usize::try_from(columns).unwrap_or(usize::MAX);
                let row_count = child_count.div_ceil(column_count);
                Some((column_count, row_count))
            }
            PlacementHint::Row => Some((child_count, 1)),
            PlacementHint::Column => Some((1, child_count)),
        }
    }

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
        let span = element.span().map(SourceSpan::from);
        let placement_without_children = ChildPlacementWithoutChildren {
            element: path.clone(),
            child_placement,
            span,
        };
        let problem = Problem::ChildPlacementWithoutChildren(placement_without_children);
        Some(problem)
    }

    fn check_position_is_bounded(path: &ElementPath, element: &Element) -> Option<Problem> {
        let position = element.position()?;
        let is_x_bounded = position.x.abs() <= MAX_MAGNITUDE;
        let is_y_bounded = position.y.abs() <= MAX_MAGNITUDE;
        let is_bounded = is_x_bounded && is_y_bounded;
        if is_bounded {
            return None;
        }
        let span = element.span().map(SourceSpan::from);
        let unbounded_position = UnboundedPosition {
            element: path.clone(),
            position,
            span,
        };
        let problem = Problem::UnboundedPosition(unbounded_position);
        Some(problem)
    }

    /// Report each link shaped like an element path that names no element, the likely typo of a drill-down target.
    fn find_unresolved_links(&self) -> Vec<Problem> {
        let elements = self.elements().iter();
        let problems =
            elements.filter_map(|(path, element)| self.check_link_resolves(path, element));
        problems.collect()
    }

    fn check_link_resolves(&self, path: &ElementPath, element: &Element) -> Option<Problem> {
        let raw_link = element.link()?;
        let link = Link::resolve(raw_link, self);
        let target = link.path_shaped_url()?;
        let suggestion = self.suggest_closest_element(&target);
        let link_or_declaration_range = element.link_span().or(element.span());
        let span = link_or_declaration_range.map(SourceSpan::from);
        let unresolved_link = UnresolvedLink {
            element: path.clone(),
            link: raw_link.to_owned(),
            suggestion,
            span,
        };
        let problem = Problem::UnresolvedLink(unresolved_link);
        Some(problem)
    }

    fn suggest_closest_element(&self, unknown: &ElementPath) -> Option<Suggestion<ElementPath>> {
        let unknown_text = unknown.to_string();
        let known_paths = self.elements().keys().cloned();
        Suggestion::closest_to(&unknown_text, known_paths)
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

/// Report one problem: something the author must fix (an error) or should look at (a warning).
#[derive(Debug, Error, Diagnostic)]
pub enum Problem {
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingAncestor(MissingAncestor),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DanglingEdgeEndpoint(DanglingEdgeEndpoint),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateElement(DuplicateElement),
    #[error(transparent)]
    #[diagnostic(transparent)]
    AncestorDescendantEdge(AncestorDescendantEdge),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnknownStyleKey(UnknownStyleKey),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MismatchedStyleValue(MismatchedStyleValue),
    #[error(transparent)]
    #[diagnostic(transparent)]
    EmptyGridHint(EmptyGridHint),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UncountablePackedTracks(UncountablePackedTracks),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ChildPlacementWithoutChildren(ChildPlacementWithoutChildren),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnboundedPosition(UnboundedPosition),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnresolvedLink(UnresolvedLink),
    #[error(transparent)]
    #[diagnostic(transparent)]
    IgnoredContainerShape(IgnoredContainerShape),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ImpossibleHeight(ImpossibleHeight),
    #[error(transparent)]
    #[diagnostic(transparent)]
    IgnoredContainerHeight(IgnoredContainerHeight),
    #[error(transparent)]
    #[diagnostic(transparent)]
    IgnoredContainerBadge(IgnoredContainerBadge),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ArcIntoBadgeTarget(ArcIntoBadgeTarget),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedLegendDash(MalformedLegendDash),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UndrawnLegendKey(UndrawnLegendKey),
}

impl Problem {
    /// Tell whether the problem blocks a build or a load; a problem without a severity is an error.
    pub fn is_error(&self) -> bool {
        let severity = self.severity();
        let severity_or_error = severity.unwrap_or(Severity::Error);
        severity_or_error == Severity::Error
    }
}

/// Report an element whose ancestor is missing: its enclosing container was never declared.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` is inside `{ancestor}`, which is not an element")]
#[diagnostic(code(svugy::dangling_reference))]
pub struct MissingAncestor {
    pub element: ElementPath,
    pub ancestor: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a dangling edge endpoint: a path that is not an element.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` points to `{endpoint}`, which is not an element")]
#[diagnostic(code(svugy::dangling_reference))]
pub struct DanglingEdgeEndpoint {
    pub edge: EdgeId,
    pub endpoint: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("referenced here")]
    pub span: Option<SourceSpan>,
}

/// Report a duplicate element: one declared twice at the same path.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{path}` is declared more than once")]
#[diagnostic(code(svugy::duplicate_id))]
pub struct DuplicateElement {
    pub path: ElementPath,
    #[label("declared again here")]
    pub span: Option<SourceSpan>,
}

/// Report an edge joining an element to its own ancestor or descendant, which layout leaves out.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` joins an element to its own ancestor, so layout leaves it out")]
#[diagnostic(code(svugy::ancestor_descendant_edge), severity(Warning))]
pub struct AncestorDescendantEdge {
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report an unknown style key: one outside the v1 set.
#[derive(Debug, Error, Diagnostic)]
#[error("{owner} has style `{key}`, which is not a v1 style key")]
#[diagnostic(code(svugy::unknown_style))]
pub struct UnknownStyleKey {
    pub owner: StyleOwner,
    pub key: String,
    #[help]
    pub suggestion: Option<Suggestion<&'static str>>,
    #[label("styled here")]
    pub span: Option<SourceSpan>,
}

/// Report a mismatched style value: a v1 style key set to a value of the wrong kind.
#[derive(Debug, Error, Diagnostic)]
#[error("{owner} sets style `{key}` to `{value}`, which is not {expected}")]
#[diagnostic(code(svugy::mismatched_style_value))]
pub struct MismatchedStyleValue {
    pub owner: StyleOwner,
    pub key: StyleKey,
    pub value: String,
    pub expected: StyleValueKind,
    #[label("styled here")]
    pub span: Option<SourceSpan>,
}

/// Report an empty grid hint: zero columns, so no cell to place a child in.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` asks for a grid of 0 columns, which has no cell for any child")]
#[diagnostic(code(svugy::impossible_placement))]
pub struct EmptyGridHint {
    pub element: ElementPath,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a packing hint laying children in more tracks, columns or rows, than a packed level can count.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` packs its {child_count} children into {columns} columns and {rows} rows, but a packed level holds at most {max} of either",
    max = PlacementHint::MAX_PACKED_TRACKS
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UncountablePackedTracks {
    pub element: ElementPath,
    pub child_count: usize,
    pub columns: usize,
    pub rows: usize,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a child placement hint on an element without children, which has nothing to place.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` has child placement `{child_placement:?}` but no children to place")]
#[diagnostic(code(svugy::impossible_placement), severity(Warning))]
pub struct ChildPlacementWithoutChildren {
    pub element: ElementPath,
    pub child_placement: PlacementHint,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a pinned position with a coordinate that is not a finite number within `MAX_MAGNITUDE` of its level's origin.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is pinned at ({}, {}), which is not within {MAX_MAGNITUDE} plan units of its level's origin",
    .position.x,
    .position.y
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UnboundedPosition {
    pub element: ElementPath,
    pub position: Position,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's authored shape, which v1 ignores: a container is a slab, whose rectangular top holds its interior.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container, so it is drawn as a slab and its shape `{shape:?}` is ignored in v1"
)]
#[diagnostic(code(svugy::ignored_container_shape), severity(Warning))]
pub struct IgnoredContainerShape {
    pub element: ElementPath,
    pub shape: Shape,
    #[label("shaped here")]
    pub span: Option<SourceSpan>,
}

/// Report a height that is not a positive number within `MAX_MAGNITUDE` plan units, so no solid can stand that tall.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` stands {height} plan units tall, which is not a positive number no greater than {MAX_MAGNITUDE}"
)]
#[diagnostic(code(svugy::impossible_height))]
pub struct ImpossibleHeight {
    pub element: ElementPath,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's authored height, which v1 ignores: a container is a slab, whose tag hangs from its low front face onto the ground.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container, so it is drawn as a slab and its height {height} is ignored in v1"
)]
#[diagnostic(code(svugy::ignored_container_height), severity(Warning))]
pub struct IgnoredContainerHeight {
    pub element: ElementPath,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a container marked badge, which v1 ignores: a container is entered, so edges into it stay arcs.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container, so it is entered rather than badged and edges into it are drawn as arcs"
)]
#[diagnostic(code(svugy::ignored_container_badge), severity(Warning))]
pub struct IgnoredContainerBadge {
    pub element: ElementPath,
    #[label("marked here")]
    pub span: Option<SourceSpan>,
}

/// Report an edge from a container into a leaf marked badge, drawn as an arc since the container's top holds its interior.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "edge `{edge}` starts at a container, whose top holds its interior, so it is drawn as an arc instead of a badge"
)]
#[diagnostic(code(svugy::arc_into_badge_target), severity(Warning))]
pub struct ArcIntoBadgeTarget {
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a legend dash key that is not a dash length spelled in its shortest form, so no arc's dash can match it.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "the legend describes dash `{dash}`, which is not a dash length in its shortest spelling, e.g. `5` or `1.5`"
)]
#[diagnostic(code(svugy::malformed_legend))]
pub struct MalformedLegendDash {
    pub dash: String,
}

/// Report a key row the legend describes but the diagram never draws, so the description is never shown.
#[derive(Debug, Error, Diagnostic)]
#[error("the legend describes {key}, which the diagram never draws")]
#[diagnostic(code(svugy::undrawn_legend_key), severity(Warning))]
pub struct UndrawnLegendKey {
    pub key: LegendKey,
    #[help]
    pub suggestion: Option<Suggestion<String>>,
}

/// Name a key row of the legend: the arcs of one dash length, or the badges of one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendKey {
    Dash(String),
    Badge(ElementPath),
}

impl fmt::Display for LegendKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dash(dash) => write!(formatter, "the arcs dashed `{dash}`"),
            Self::Badge(target) => write!(formatter, "the badges of `{target}`"),
        }
    }
}

/// Report an unresolved link: one shaped like an element path that names no element, the likely typo of a drill-down target.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` links to `{link}`, which is not an element; write `./{link}` if it is a relative URL"
)]
#[diagnostic(code(svugy::unresolved_link), severity(Warning))]
pub struct UnresolvedLink {
    pub element: ElementPath,
    pub link: String,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("linked here")]
    pub span: Option<SourceSpan>,
}

/// Identify the owner of a style entry: an element or an edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleOwner {
    Element(ElementPath),
    Edge(EdgeId),
}

impl fmt::Display for StyleOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Edge(id) => write!(formatter, "edge `{id}`"),
        }
    }
}

/// Suggest the known name the author most likely meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion<T>(pub T);

impl<T: fmt::Display> Suggestion<T> {
    /// Suggest c* = argmin over c of levenshtein(lower(unknown), lower(c)), kept iff that distance ≤ ⌊len(unknown) / 3⌋.
    pub fn closest_to(unknown: &str, candidates: impl IntoIterator<Item = T>) -> Option<Self> {
        let lowercase_unknown = unknown.to_lowercase();
        let unknown_length = unknown.chars().count();
        let max_distance = unknown_length / 3;
        let distances = candidates.into_iter().map(|candidate| {
            let candidate_text = candidate.to_string();
            let lowercase_candidate = candidate_text.to_lowercase();
            let distance = strsim::levenshtein(&lowercase_unknown, &lowercase_candidate);
            (distance, candidate)
        });
        let (closest_distance, closest) =
            distances.min_by_key(|(distance, _candidate)| *distance)?;
        let is_too_far = closest_distance > max_distance;
        if is_too_far {
            return None;
        }
        let suggestion = Suggestion(closest);
        Some(suggestion)
    }
}

impl<T: fmt::Display> fmt::Display for Suggestion<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "did you mean `{}`?", self.0)
    }
}
