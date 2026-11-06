# Overview

svugy turns a text description of a system into an isometric SVG you can click into.

The output is one SVG file with its CSS and JavaScript inside, so there's no server or library to install. The input is text, so you can diff and review diagrams like code. `svugy validate` reports every problem at once, in JSON if a script is reading it.

Below, edit the d2 on the left and the diagram on the right re-renders as you type: svugy itself, compiled to wasm, running in this page. Click the `svugy` slab to enter it, press Escape to leave, and hover anything for details.

<textarea id="svugy-source" spellcheck="false" autocapitalize="off" autocomplete="off" aria-label="d2 source">{{#include ../diagrams/pipeline.d2}}</textarea>

<object id="svugy-preview" class="svugy" type="image/svg+xml" data="figures/pipeline.svg"></object>

<ul id="svugy-diagnostics"></ul>

<script src="playground/svugy-wasm.js"></script>
<script src="playground/playground.js"></script>

Next: [Quickstart](run/quickstart.md)
