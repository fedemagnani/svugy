# The map

[d2 import](import.md) parses your text into the [model](model.md). [Layout](layout.md) places boxes on a flat plane, [geometry](geometry.md) projects them into isometric shapes, and [render](render.md) writes the SVG. The [runtime](runtime.md) embedded in that SVG handles clicking and zooming. [Determinism](determinism.md) covers how the same input gives the same bytes on every machine.

There are five crates. `svugy-model` holds the model, its builder, JSON and validation. `svugy-import` parses d2. `svugy` is the pipeline: metrics, layout, geometry, render. `svugy-cli` is the `svugy` binary. `svugy-wasm` runs the pipeline in the browser, behind the editor on the overview. The JavaScript and CSS embedded in every interactive SVG live in `crates/svugy/runtime/`.

The figure below is svugy, drawn by svugy. Click a crate to step inside it. Its source, `examples/svugy.d2`, is also the golden test.

<object class="svugy" type="image/svg+xml" data="../figures/svugy.svg"></object>
