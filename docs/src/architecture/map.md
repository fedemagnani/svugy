# The map

Parsers output a `Model`; layout, geometry and render take only a `Model`. Adding an input language touches only `svugy-parse`.

| Layer | Takes | Returns |
|---|---|---|
| [Model](model.md) | builder calls | an admitted `Model`, cut into layers |
| [Parsing](parsing.md) | Markdown, d2, JSON | `Import`: model, errors, warnings |
| [Layout](layout.md) | `Model` | `LevelPlans`: footprints, routes, stubs in plan space |
| [Geometry](geometry.md) | `Model`, `LevelPlans` | `Scene`: projected faces, arcs, labels in paint order |
| [Render](render.md) | `Scene` | SVG text |
| [Runtime](runtime.md) | the interactive SVG | navigation, in the browser |

- `StaticSvg::render`: lay out, project, write the whole model.
- `InteractiveSvg::render`: cut the model into layers, lay out and project each, write all of them into one file.
- Output is [deterministic](determinism.md): same input, same bytes on Linux, macOS and wasm.

## Crates

- **`svugy-model`:** `Model`, `ModelBuilder`, validation, layers. No svugy dependency.
- **`svugy-parse`:** `markdown`, `d2`, `json`.
- **`svugy`:** layout, geometry, render; embeds the runtime.
- **`svugy-cli`:** the `svugy` binary.
- **`svugy-wasm`:** Markdown or d2 to interactive SVG in the browser; runs this book's live editors.
- **`svugy-dagre`:** `LeftToRightLayout::node_centres`, dagre's layered layout cut down from dagre-dgl-rs to what `svugy` uses, deterministic.

## svugy, drawn by svugy

Source: `examples/svugy/svugy.md`, also the golden test.

<object class="svugy" type="image/svg+xml" data="../figures/svugy.svg"></object>
