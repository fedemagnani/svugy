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

- `add_top_level` and `add_child` return an `ElementRef`, and edges take only `ElementRef`s, so an edge can't name a missing element.
- `build()` runs the same checks as a Markdown file: `model` is `None` if any problem is an error, and `report` lists every problem.
- `InteractiveSvg::render(&model)` and `StaticSvg::render(&model)` return the SVG as a `String`.
- `svugy_parse::json::JsonPrinter::print(&model)` returns the model as JSON, which `svugy render` reads.

Each Markdown property is one `with_*` call:

| Markdown | Rust |
|---|---|
| `: DISPLAY NAME` | `Element::new(label)` |
| `caption`, `height`, `shape`, `position`, `link` | `with_caption`, `with_height`, `with_shape`, `with_position`, `with_link` |
| `meta.NAME` | `with_metadata(name, value)` |
| `layout`, `columns` | `with_child_placement(PlacementHint::…)` |
| `color`, `stroke` | `with_style("fill", …)`, `with_style("stroke", …)` |
| `line=dashed`, `line=dotted` | `with_style("stroke-dash", "5")`, `"2"` |
| edge label, `traveler`, `color` | `Edge::with_label`, `with_traveler`, `with_style("stroke", …)` |
| edge `line=badge` | `Edge::with_badge` |
| `title`, `subtitle`, legend | `ModelBuilder::with_title`, `with_subtitle`, `with_legend` |

Next: [Interactive SVG](../interactive.md)
