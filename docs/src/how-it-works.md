# How it works

| Stage | Takes | Returns |
|---|---|---|
| Parse | Markdown, d2, JSON | a `Model`: a tree of elements plus edges, validated once |
| Layers | the `Model` | one model per container depth |
| Layout | each layer | footprints and routes on a flat ground plane |
| Geometry | the layout | isometric solids, floor traces and arcs, in paint order |
| Render | the scene | SVG text, static or interactive |

- **One model:** every language becomes the same `Model`, so adding a language touches only the parser.
- **Layers:** layer K shows elements down to depth K + 1. Containers at that depth are closed and drawn as leaves, and the edges into them merge. The deepest layer is placed once, and shallower layers keep its arrangement, so opening a container never moves its neighbours.
- **Layout is 2D:** a layered layout (dagre) ranks along one axis, so a chain of edges becomes a screen diagonal; `row`, `column` and `grid` pack children in source order; `position` pins. A container with no `layout` and no edges among its children packs them in a row, or in a column when its edges arrive mostly from its side, so every child faces them, each placed near the neighbours it connects to.
- **Traces or arcs:** an edge between two components on the same container's slab runs flat along the floor in right-angle segments, at most two bends, when such a path clears the other components and earlier traces; ports less than 36 units out of line snap straight. Every other edge, and every edge on the open ground, arcs over the scene.
- **Bundles:** two or more edges joining the same two components, at least one a container, share one arc between the containers' boundaries and branch flat inside each one, when every source among them reaches every target, so a bundle never implies an edge that does not exist. An edge that does not fit keeps its own arc.
- **Static or interactive:** the static SVG draws the deepest layer with no CSS or script. The interactive one draws every layer and embeds the runtime that switches between them ([Interactive SVG](interactive.md)).
- **Deterministic:** the same input gives the same bytes on Linux, macOS and wasm: the font is bundled, the maths avoids platform-dependent functions, and nothing depends on hash order. Committed SVGs diff cleanly.

## Crates

| Crate | Contents |
|---|---|
| `svugy-model` | the model, its builder, its validation and its layers |
| `svugy-parse` | Markdown, d2 and JSON in; Markdown and JSON out |
| `svugy` | layout, geometry and render; embeds the runtime |
| `svugy-cli` | the `svugy` command |
| `svugy-wasm` | svugy in the browser; runs this book's editors |
| `svugy-dagre` | a deterministic cut of dagre's layered layout |

## svugy, drawn by svugy

Source: [`examples/svugy/svugy.md`](https://github.com/fedemagnani/svugy/blob/main/examples/svugy/svugy.md).

<object class="svugy" type="image/svg+xml" data="figures/svugy.svg"></object>
