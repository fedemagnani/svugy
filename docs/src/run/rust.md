# Rust

Build the model in code and render it with the `svugy` crate, e.g. to draw a service map from config.

```sh
cargo add --git https://github.com/fedemagnani/svugy svugy svugy-model
```

The Quickstart's web app, built in code: [`examples/webapp/webapp.rs`](https://github.com/fedemagnani/svugy/blob/main/examples/webapp/webapp.rs). `cargo run -p svugy-examples --example webapp > webapp.svg` writes the SVG below.

```rust,ignore
{{#include ../../../examples/webapp/webapp.rs}}
```

<object class="svugy" type="image/svg+xml" data="../figures/webapp-rust.svg"></object>

- `ModelBuilder::add_top_level` and `add_child` return an `ElementRef`. Parents and edge endpoints take only an `ElementRef`, so an edge can't name a missing element.
- `Element` and `Edge` take one `with_*` call per Markdown property:

| Markdown | Rust |
|---|---|
| `: DISPLAY NAME` | `Element::new(label)` |
| `caption`, `height`, `shape`, `position`, `link` | `with_caption`, `with_height`, `with_shape`, `with_position`, `with_link` |
| `meta.NAME` | `with_metadata(name, value)` |
| `layout`, `columns` | `with_child_placement(PlacementHint::…)` |
| `color`, `stroke` | `with_style("fill", …)`, `with_style("stroke", …)` |
| `line=dashed`, `line=dotted` | `with_style("stroke-dash", "5")`, `"2"` |
| edge label, `traveler` | `Edge::with_label`, `Edge::with_traveler` |
| edge `color` | `Edge::with_style("stroke", …)` |
| edge `line=badge` | `Edge::with_badge` |
| frontmatter `title`, `subtitle`, legend | `ModelBuilder::with_title`, `with_subtitle`, `with_legend` |

- `build()` runs the model checks a Markdown file goes through and returns a `ModelAdmission`: `model` is `None` if any problem is an error; `report` lists every problem, warnings included.
- `InteractiveSvg::render(&model)` returns the interactive SVG as a `String`; `StaticSvg::render(&model)` returns the static one.
- `svugy_parse::json::JsonPrinter::print(&model)` returns the model as JSON, which `svugy render` reads and `svugy import model.json -o model.md` converts to Markdown.

Next: [The map](../architecture/map.md)
