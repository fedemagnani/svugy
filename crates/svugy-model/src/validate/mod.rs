//! Validate a model and report what the author must fix, with did-you-mean suggestions.

mod target;
mod workflow;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use miette::{Diagnostic, JSONReportHandler, Severity, SourceSpan};

use crate::code::CodeReference;
use crate::description::DescriptionOwner;
use crate::graph::WorkflowGraph;
use crate::link::Link;
use crate::model::{ByteRange, Element, MAX_MAGNITUDE, Model, ViewAngle};
use crate::path::ElementPath;
use crate::problem::{
    DuplicateElement, DuplicateOverride, DuplicateWorkflow, ElementSite, ErrorSeverity,
    ImpossibleHeight, MalformedCodeReference, MalformedWorkflowId, MismatchedStyleValue,
    MissingAncestor, NoWorkflow, Problem, StyleOwner, Suggestion, UnknownStyleKey, UnresolvedLink,
    UnshownElement,
};
use crate::properties::ComponentProperties;
use crate::style::AnyStyleKey;
use crate::workflow::{Workflow, WorkflowId};

use self::target::{ComponentScope, TargetScope};
use self::workflow::WorkflowCheck;

/// Assemble a model whose references are not yet checked, recording duplicate declarations and overrides; only `admit` hands it out.
#[derive(Debug)]
pub struct UncheckedModel {
    pub(crate) model: Model,
    pub(crate) duplicate_declarations: Vec<(ElementPath, Option<SourceSpan>)>,
    pub(crate) duplicate_overrides: Vec<(WorkflowId, ElementPath, Option<SourceSpan>)>,
}

/// Admit a model unless its report holds an error, handing out that full report either way, warnings included.
#[derive(Debug)]
pub struct ModelAdmission {
    pub model: Option<Model>,
    pub report: ModelReport,
}

/// Collect a model's problems into one diagnostic whose severity is its worst problem's; the default holds none, as for a source no model came out of.
#[derive(Debug, Default)]
pub struct ModelReport {
    problems: Vec<Problem>,
}

/// Hold one property value written at a component site, with the span labelling it, the input every site check reads.
struct SiteValue<'a, T> {
    site: &'a ElementSite,
    value: Option<T>,
    span: Option<SourceSpan>,
}

impl Default for UncheckedModel {
    fn default() -> Self {
        Self::from_parts(BTreeMap::new(), Vec::new())
    }
}

impl UncheckedModel {
    /// Hold the model of these parts unchecked, elements keyed by path and workflows in written order, with no duplicate recorded.
    pub fn from_parts(elements: BTreeMap<ElementPath, Element>, workflows: Vec<Workflow>) -> Self {
        let model = Model {
            view: None,
            elements,
            workflows,
        };
        Self {
            model,
            duplicate_declarations: Vec::new(),
            duplicate_overrides: Vec::new(),
        }
    }

    /// Show the diagram from `view` first.
    pub fn with_view(mut self, view: ViewAngle) -> Self {
        self.model.view = Some(view);
        self
    }

    /// Admit the model only when no check holds an error.
    pub fn admit(self) -> ModelAdmission {
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

    /// Run every semantic check, model-wide first, then each workflow's, and report each problem found, errors and warnings alike.
    fn validate(&self) -> ModelReport {
        let model = &self.model;
        let graphs: Vec<WorkflowGraph> = model
            .workflows()
            .iter()
            .map(|workflow| WorkflowGraph::of(model, workflow))
            .collect();
        let duplicate_elements = self.find_duplicate_elements();
        let missing_ancestors = model.find_missing_ancestors();
        let element_property_problems = model.find_element_property_problems();
        let workflow_set_problems = model.find_workflow_set_problems();
        let duplicate_overrides = self.find_duplicate_overrides();
        let unshown_elements = model.find_unshown_elements(&graphs);
        let component_description_problems = model.find_component_description_problems(&graphs);
        let workflow_problems =
            model
                .workflows()
                .iter()
                .zip(&graphs)
                .flat_map(|(workflow, graph)| {
                    let workflow_check = WorkflowCheck::new(model, workflow, graph);
                    workflow_check.problems()
                });
        let problems = duplicate_elements
            .into_iter()
            .chain(missing_ancestors)
            .chain(element_property_problems)
            .chain(workflow_set_problems)
            .chain(duplicate_overrides)
            .chain(unshown_elements)
            .chain(component_description_problems)
            .chain(workflow_problems)
            .collect();
        ModelReport { problems }
    }

    /// Report each later declaration of an element already declared, in declaration order.
    fn find_duplicate_elements(&self) -> Vec<Problem> {
        let duplicate_declarations = self.duplicate_declarations.iter();
        let problems = duplicate_declarations.map(|(path, span)| {
            let duplicate_element = DuplicateElement {
                element: path.clone(),
                span: *span,
            };
            Problem::DuplicateElement(duplicate_element)
        });
        problems.collect()
    }

    /// Report each later override of an element a workflow already overrides, in declaration order.
    fn find_duplicate_overrides(&self) -> Vec<Problem> {
        let duplicate_overrides = self.duplicate_overrides.iter();
        let problems = duplicate_overrides.map(|(workflow, path, span)| {
            let duplicate_override = DuplicateOverride {
                workflow: workflow.clone(),
                element: path.clone(),
                span: *span,
            };
            Problem::DuplicateOverride(duplicate_override)
        });
        problems.collect()
    }
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
                let is_already_reported = !reported_ancestors.insert(ancestor.clone());
                if is_already_reported {
                    continue;
                }
                let suggestion = self.suggest_closest_element(&ancestor);
                let span = element.source_span();
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

    /// Report each declared element property no workflow could draw: a style entry, height, code or link value that does not fit.
    fn find_element_property_problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (path, element) in self.elements() {
            let site = ElementSite::Declaration(path.clone());
            let site_span = element.source_span();
            let elements = self.elements();
            let property_problems =
                site.check_properties(element.properties(), site_span, elements);
            problems.extend(property_problems);
        }
        problems
    }

    /// Report a model without workflows, each workflow id an earlier workflow took, and each malformed workflow id.
    fn find_workflow_set_problems(&self) -> Vec<Problem> {
        let workflows = self.workflows();
        let has_no_workflow = workflows.is_empty();
        if has_no_workflow {
            return vec![Problem::NoWorkflow(NoWorkflow)];
        }
        let mut seen_ids = BTreeSet::new();
        let mut problems = Vec::new();
        for workflow in workflows {
            let id = workflow.id();
            let span = workflow.source_span();
            let is_malformed = !id.is_well_formed();
            if is_malformed {
                let malformed_id = MalformedWorkflowId {
                    workflow: id.clone(),
                    span,
                };
                problems.push(Problem::MalformedWorkflowId(malformed_id));
            }
            let is_first_with_id = seen_ids.insert(id);
            if is_first_with_id {
                continue;
            }
            let duplicate_workflow = DuplicateWorkflow {
                workflow: id.clone(),
                span,
            };
            problems.push(Problem::DuplicateWorkflow(duplicate_workflow));
        }
        problems
    }

    /// Report each element no workflow shows, in path order; a model without workflows reports none, as `NoWorkflow` covers it.
    fn find_unshown_elements(&self, graphs: &[WorkflowGraph]) -> Vec<Problem> {
        let has_no_workflow = self.workflows().is_empty();
        if has_no_workflow {
            return Vec::new();
        }
        let elements = self.elements().iter();
        let unshown_elements = elements.filter(|(path, _element)| {
            let mut graphs = graphs.iter();
            graphs.all(|graph| graph.element(path).is_none())
        });
        let problems = unshown_elements.map(|(path, element)| {
            let span = element.source_span();
            let unshown_element = UnshownElement {
                element: path.clone(),
                span,
            };
            Problem::UnshownElement(unshown_element)
        });
        problems.collect()
    }

    /// Report each component description link whose target doesn't resolve, edge targets in every workflow showing the component.
    fn find_component_description_problems(&self, graphs: &[WorkflowGraph]) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (path, element) in self.elements() {
            let Some(description) = element.description() else {
                continue;
            };
            let scope = ComponentScope::new(self, graphs, path);
            let owner = DescriptionOwner::Element(path.clone());
            let span = element.source_span();
            let description_problems = scope.check_description(&owner, description, span);
            problems.extend(description_problems);
        }
        problems
    }

    /// Suggest the declared element whose path is closest to `unknown`.
    fn suggest_closest_element(&self, unknown: &ElementPath) -> Option<Suggestion<ElementPath>> {
        let unknown_text = unknown.to_string();
        let known_paths = self.elements().keys().cloned();
        Suggestion::closest_to(&unknown_text, known_paths)
    }
}

impl ElementSite {
    /// Check each property written at this site, labelled by its own span else `site_span`: every style key v1 with a fitting value, a bounded height, a well-formed code reference, a link resolving among `elements`.
    pub(crate) fn check_properties(
        &self,
        properties: &ComponentProperties,
        site_span: Option<SourceSpan>,
        elements: &BTreeMap<ElementPath, Element>,
    ) -> Vec<Problem> {
        let owner = StyleOwner::Element(self.clone());
        let style_entries = properties.style().iter();
        let style_problems =
            style_entries.filter_map(|(key, value)| owner.check_style_entry(key, value, site_span));
        let mut problems: Vec<Problem> = style_problems.collect();
        let height = SiteValue::new(self, properties.height(), None, site_span);
        let code_span = properties.code_span();
        let code = SiteValue::new(self, properties.code(), code_span, site_span);
        let link_span = properties.link_span();
        let link = SiteValue::new(self, properties.link(), link_span, site_span);
        problems.extend(height.check_height_is_bounded());
        problems.extend(code.check_code_is_well_formed());
        problems.extend(link.check_link_resolves(elements));
        problems
    }
}

impl<'a, T> SiteValue<'a, T> {
    /// Create a new site value: `value` written at `site`, labelled by its own span if recorded, else the site's.
    fn new(
        site: &'a ElementSite,
        value: Option<T>,
        value_span: Option<ByteRange>,
        site_span: Option<SourceSpan>,
    ) -> Self {
        let value_source_span = value_span.map(SourceSpan::from);
        let span = value_source_span.or(site_span);
        Self { site, value, span }
    }
}

impl SiteValue<'_, f64> {
    /// Check that the height, if any, is positive and within `MAX_MAGNITUDE`.
    fn check_height_is_bounded(&self) -> Option<Problem> {
        let height = self.value?;
        let is_positive = height > 0.0;
        let is_within_magnitude = height <= MAX_MAGNITUDE;
        let is_bounded = is_positive && is_within_magnitude;
        if is_bounded {
            return None;
        }
        let impossible_height = ImpossibleHeight {
            site: self.site.clone(),
            height,
            span: self.span,
        };
        Some(Problem::ImpossibleHeight(impossible_height))
    }
}

impl SiteValue<'_, &str> {
    /// Check that the code value, if any, parses as a repo-relative `FILE` or `FILE#SYMBOL`.
    fn check_code_is_well_formed(&self) -> Option<Problem> {
        let raw_code = self.value?;
        let parsed_code = CodeReference::parse(raw_code);
        let Err(reason) = parsed_code else {
            return None;
        };
        let malformed_code_reference = MalformedCodeReference {
            site: self.site.clone(),
            code: raw_code.to_owned(),
            reason,
            span: self.span,
        };
        Some(Problem::MalformedCodeReference(malformed_code_reference))
    }

    /// Check that the link value, if shaped like an element path, names one of `elements`.
    fn check_link_resolves(&self, elements: &BTreeMap<ElementPath, Element>) -> Option<Problem> {
        let raw_link = self.value?;
        let link = Link::resolve(raw_link, elements);
        let target = link.path_shaped_url()?;
        let target_text = target.to_string();
        let known_paths = elements.keys().cloned();
        let suggestion = Suggestion::closest_to(&target_text, known_paths);
        let unresolved_link = UnresolvedLink {
            site: self.site.clone(),
            link: raw_link.to_owned(),
            suggestion,
            span: self.span,
        };
        Some(Problem::UnresolvedLink(unresolved_link))
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
            let unknown_style_key = UnknownStyleKey {
                owner: self.clone(),
                key: key.to_owned(),
                suggestion,
                span,
            };
            return Some(Problem::UnknownStyleKey(unknown_style_key));
        };
        let expected = style_key.value_kind();
        let is_fitting_value = expected.accepts(value);
        if is_fitting_value {
            return None;
        }
        let mismatched_style_value = MismatchedStyleValue {
            owner: self.clone(),
            key: style_key,
            value: value.to_owned(),
            expected,
            span,
        };
        Some(Problem::MismatchedStyleValue(mismatched_style_value))
    }
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

    /// Keep only the problems `keep` accepts, as an importer does with those it already reported more precisely.
    pub fn retain(&mut self, keep: impl FnMut(&Problem) -> bool) {
        self.problems.retain(keep);
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
