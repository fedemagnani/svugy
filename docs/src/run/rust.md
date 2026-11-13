# Rust

Build the model in code and render it with the `svugy` crate, e.g. to draw a service map from config.

```sh
cargo add --git https://github.com/fedemagnani/svugy svugy svugy-model
```

The Quickstart's web app, built in code ([`examples/webapp/webapp.rs`](https://github.com/fedemagnani/svugy/blob/main/examples/webapp/webapp.rs)):

```rust,ignore
{{#include ../../../examples/webapp/webapp.rs}}
```

<object class="svugy" type="image/svg+xml" data="../figures/webapp-rust.svg"></object>

- `add_top_level` and `add_child` return an `ElementRef`; `add_edge`, `list_element` and `override_element` take only `ElementRef`s, so they can't name a missing component.
- `add_workflow` adds a `Workflow` after the ones already added and returns its handle; `add_step` adds a `Step` to a workflow and returns its handle.
- `add_edge` declares an edge in the step's workflow and lists it in that step. List it again in another step of the same workflow with `list_edge`; `list_element` lists a component without an edge.
- `override_element` sets a component's override in one workflow: `position`, `layout` and any other component property.
- `build()` runs the same checks as a Markdown file: `model` is `None` if any problem is an error, and `report` lists every problem.
- Rendering takes the model with the workflow to show: `model.showing_first_workflow()`, or `model.showing(&id)`, `None` when no workflow has that id. `InteractiveSvg::render` draws every workflow and opens on the shown one; `StaticSvg::render` draws the shown one alone. `SketchInteractiveSvg` and `SketchStaticSvg` do the same in the sketch theme. Each returns the SVG as a `String`.
- `svugy_parse::json::JsonPrinter::print(&model)` returns the model as JSON, which `svugy render` reads.

Each Markdown construct is one call:

| Markdown | Rust |
|---|---|
| `: DISPLAY NAME` | `Element::new(label)` |
| `caption`, `height`, `shape`, `link`, `code` | `with_caption`, `with_height`, `with_shape`, `with_link`, `with_code` |
| `meta.NAME` | `with_metadata(name, value)` |
| `color`, `stroke` | `with_style("fill", …)`, `with_style("stroke", …)` |
| `line=dashed`, `line=dotted` | `with_style("stroke-dash", "5")`, `"2"` |
| a description | `with_description` on `Element`, `Edge`, `Workflow` or `Step` |
| `### ID: TITLE (subtitle=…)` | `Workflow::new(WorkflowId::new(id), title)`, `with_subtitle` |
| `#### ID: TITLE` | `Step::new(StepId::new(id), title)` |
| an override's `position`, `layout`, `columns` | `ElementOverride::new()`, `with_position`, `with_child_placement(PlacementHint::…)` |
| an override's other properties | `ElementOverride::with_properties` |
| edge label, `traveler`, `color` | `Edge::with_label`, `with_traveler`, `with_style("stroke", …)` |
| edge `line=badge` | `Edge::with_badge` |

Next: [Interactive SVG](../interactive.md)
