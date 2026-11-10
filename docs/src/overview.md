# svugy

Write a diagram in Markdown; svugy renders it as one isometric SVG you can click into.

- **Input:** a `.md` file: a nested list of components and a list of edges. It diffs like code, and any Markdown preview shows it as an outline.
- **Output:** one self-contained `.svg`. Open it in a browser; nothing to install or serve.
- **Errors:** `svugy validate` reports every problem in one run, with line and column.

Edit the diagram below: svugy, compiled to wasm, re-renders it as you type. The theme toggle switches between the standard and sketch themes.

<div class="svugy-live">

<textarea data-svugy-language="markdown">{{#include ../diagrams/pipeline.md}}</textarea>

<textarea data-svugy-language="d2" hidden>{{#include playground/sample.d2}}</textarea>

<object class="svugy" type="image/svg+xml" data="figures/pipeline.svg"></object>

</div>

<script src="playground/svugy-wasm.js"></script>
<script src="playground/playground.js"></script>

Next: [Quickstart](run/quickstart.md)
