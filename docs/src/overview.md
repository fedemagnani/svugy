# Overview

svugy renders a diagram written in Markdown as an isometric SVG you can click into.

- **Input:** a `.md` file with a nested list of components and a list of edges. Any Markdown preview shows it as an outline; it diffs like code.
- **Output:** one `.svg` file with its CSS and JavaScript inside. Open it in a browser; nothing to install or serve.
- **Errors:** `svugy validate` reports every problem in one run, with line and column; `--format json` for scripts and agents.
- **d2:** `svugy render` also reads `.d2` files, and `svugy import` converts them to Markdown.

The editor below runs svugy compiled to wasm.

<div class="svugy-live">

<textarea data-svugy-language="markdown">{{#include ../diagrams/pipeline.md}}</textarea>

<textarea data-svugy-language="d2" hidden>{{#include playground/sample.d2}}</textarea>

<object class="svugy" type="image/svg+xml" data="figures/pipeline.svg"></object>

</div>

<script src="playground/svugy-wasm.js"></script>
<script src="playground/playground.js"></script>

Next: [Quickstart](run/quickstart.md)
