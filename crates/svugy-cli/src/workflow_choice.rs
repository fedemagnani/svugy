//! Pick the workflows `render` shows: the one named, else the first, or each in turn.

use clap::Args;
use miette::miette;
use svugy_model::model::{Model, ShownWorkflow};
use svugy_model::workflow::{Workflow, WorkflowId};

use crate::failure::Failure;

/// Name the clap group of `--workflow` and `--all-workflows`, which `--profile static` requires.
pub const WORKFLOW_CHOICE_GROUP: &str = "workflow_choice";

/// Name the workflow `render` shows, or ask for one SVG per workflow.
#[derive(Args)]
#[group(id = WORKFLOW_CHOICE_GROUP, multiple = false)]
pub struct WorkflowChoice {
    /// Workflow to show: the only one --profile static draws, the one the interactive SVG opens on; the first when omitted.
    #[arg(long, value_name = "ID")]
    workflow: Option<WorkflowId>,
    /// Write one file per workflow, each named STEM.ID.svg, or STEM.ID.png with --png, beside OUTPUT, STEM being OUTPUT's file name up to its last dot; needs -o.
    #[arg(long, requires = "output")]
    all_workflows: bool,
}

/// Name what `render` shows: one workflow, or each workflow in turn.
pub enum ShownWorkflows<'m> {
    One(ShownWorkflow<'m>),
    Each(Vec<ShownWorkflow<'m>>),
}

impl WorkflowChoice {
    /// Show each workflow of `model` when `--all-workflows` asks, else the one `--workflow` names, else its first; an id naming no workflow is a usage failure listing the ids it has.
    pub fn shown_workflows<'m>(&self, model: &'m Model) -> Result<ShownWorkflows<'m>, Failure> {
        let is_each_workflow_requested = self.all_workflows;
        if is_each_workflow_requested {
            let each_workflow = model.showing_each_workflow().collect();
            let shown_each_workflow = ShownWorkflows::Each(each_workflow);
            return Ok(shown_each_workflow);
        }
        let Some(workflow_id) = &self.workflow else {
            let first_workflow = model.showing_first_workflow();
            let shown_first_workflow = ShownWorkflows::One(first_workflow);
            return Ok(shown_first_workflow);
        };
        let named_workflow = model.showing(workflow_id).ok_or_else(|| {
            let workflow_ids: Vec<&str> = model
                .workflows()
                .iter()
                .map(Workflow::id)
                .map(WorkflowId::as_str)
                .collect();
            let known_ids = workflow_ids.join(", ");
            let unknown_id = workflow_id.as_str();
            let report = miette!(
                help = format!("the input's workflows are: {known_ids}"),
                "--workflow {unknown_id} names no workflow of the input"
            );
            Failure::Usage(report)
        })?;
        let shown_named_workflow = ShownWorkflows::One(named_workflow);
        Ok(shown_named_workflow)
    }
}
