# Rust

Build the model in code and render it with the `svugy` crate, e.g. to draw a service map from config.

```sh
cargo add --git https://github.com/fedemagnani/svugy svugy svugy-model
```

Part of the [Quickstart](quickstart.md)'s web app:

```rust,ignore
use svugy::InteractiveSvg;
use svugy_model::builder::ModelBuilder;
use svugy_model::model::{Edge, Element, Shape, Traveler};

fn main() -> std::io::Result<()> {
    let mut builder = ModelBuilder::new()
        .with_title("A web app")
        .with_subtitle("from the browser to the database and back");

    let browser_element = Element::new("Browser").with_caption("the user's tab");
    let browser = builder.add_top_level("browser", browser_element);
    let backend_element = Element::new("Backend").with_caption("your servers");
    let backend = builder.add_top_level("backend", backend_element);
    let api_element = Element::new("API")
        .with_caption("HTTP endpoints")
        .with_style("fill", "#3478f6");
    let api = builder.add_child(&backend, "api", api_element);
    let data_element = Element::new("Data").with_caption("what the app remembers");
    let data = builder.add_top_level("data", data_element);
    let db_element = Element::new("Database")
        .with_shape(Shape::Cylinder)
        .with_style("fill", "#d5dcea");
    let db = builder.add_child(&data, "db", db_element);

    let sign_in = Edge::new().with_label("signs in").with_traveler(Traveler::Key);
    builder.add_edge(&browser, &api, sign_in);
    let save_order = Edge::new()
        .with_label("saves the order")
        .with_traveler(Traveler::Document);
    builder.add_edge(&api, &db, save_order);

    let admission = builder.build();
    let Some(model) = admission.model else {
        let report_json = admission.report.to_json();
        eprintln!("{report_json}");
        std::process::exit(1);
    };
    let svg = InteractiveSvg::render(&model);
    std::fs::write("webapp.svg", svg)
}
```

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
