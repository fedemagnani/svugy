# Runtime

A large system drawn at full depth is unreadable; drawn shallow, it is incomplete. The interactive SVG holds every [layer](model.md), and the runtime embedded in it shows one at a time, with no viewer or server.

The renderer and the runtime share only the attributes of the [runtime contract](runtime-contract.md).

## Components

**Navigation** keeps one shown layer and one focused level.

- Start: layer 0, containers closed, root focused.
- Click a closed container: show the next layer, opening every container at that depth.
- Click an open container's slab: zoom into it.
- Click a stub: jump to the level of the edge's other end.
- Escape or a click on empty canvas: undo one step.
- Zooms and layer crossfades take 450 ms; a layer change keeps the clicked element where it was on screen.

**Elevator** shows where the focused level sits, top right, whenever it is not the root.

- Floors: the levels from the root down; click one to go there.
- Mini-map: the parent level with the focused one framed; click to go up.

**Details**: hover or focus a group for a panel of its details; everything unrelated dims.

**Accessibility**: element, edge and stub groups are focusable; Enter and Space click; Escape backs out. Under reduced motion, zooms and crossfades jump, flows stop, travelers hide.

**Isolation**

- Paths are compared with `getAttribute`, never built into selectors; no `innerHTML`.
- The runtime binds only to the `<svg>` holding its `<script>`, so diagrams sharing a page don't interfere.
- Without script, as in `<img>`, the whole diagram shows.

Code: `crates/svugy/runtime/svugy.js`, `svugy.css`. `fixture.svg` there, rendered by `just fixture` from `crates/svugy-cli/example/webapp.md`, is a two-layer test document.
