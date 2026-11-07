# Runtime

A large system drawn at full depth is unreadable; drawn shallow, it is incomplete. The interactive SVG holds every [layer](model.md), and the runtime embedded in it shows one at a time, with no viewer or server.

The renderer and the runtime share only the attributes of the [runtime contract](runtime-contract.md).

## Components

**Navigation** keeps one shown layer and one focused level.

- Start: layer 0, containers closed, root focused; a `#svugy=PATH&layer=K` fragment in the URL starts focused on level PATH with layer K shown instead.
- Click a closed container: show the next layer, opening every container at that depth.
- Click an open container's slab: zoom into it.
- Click a stub: jump to the level of the edge's other end.
- Escape or a click on empty canvas: undo one step.
- Zooms and layer crossfades take 450 ms; a layer change keeps the clicked element where it was on screen.

**Description panel** shows, top right, what the author wrote about the focused level, rendered as Markdown, and nothing else.

- At the root: the diagram's description. A level, leaf or edge without a description shows no panel.
- Click a leaf, an edge or the heading: show its description instead; one without a description closes the panel. Escape goes back to the focused level's description.
- A lifted edge shows the description of each source edge it stands for, under that edge's label or `FROM → TO`; source edges without one are skipped.
- Markdown: paragraphs keeping their line breaks, `-`, `*` and `1.` lists nested one level, `#` to `###` headings, fenced code, bold, emphasis, inline code and http(s) or relative links.
- A `[text](#target)` link naming an element or edge is a button: it zooms to the target's level, opening layers if needed, shows the target's description and highlights the target for 1.2 s (a pulsing glow, a static outline under reduced motion). Escape backs out of it like a click. A target naming nothing is plain text.
- While the panel is open, the focused level is framed in the part of the screen left of it.

**Details**: hover or focus a group for a short panel of its details; everything unrelated dims.

**Accessibility**: element, edge and stub groups and the heading are focusable; Enter and Space click; Escape backs out. Under reduced motion, zooms and crossfades jump, flows stop, travelers hide.

**Isolation**

- Paths are compared with `getAttribute`, never built into selectors; no `innerHTML`.
- The runtime binds only to the `<svg>` holding its `<script>`, so diagrams sharing a page don't interfere.
- Without script, as in `<img>`, the whole diagram shows.

Code: `crates/svugy/runtime/svugy.js`, `svugy.css`. `fixture.svg` there, rendered by `just fixture` from `examples/webapp/webapp.md`, is a two-layer test document.
