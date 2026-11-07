# Runtime contract

*Version 2. Attribute names are spelled once, in `RuntimeContract` (`crates/svugy/src/render/contract/mod.rs`).*

The renderer and the runtime (`crates/svugy/runtime/svugy.js`, `svugy.css`) communicate only through the `data-svugy-*` attributes below. Preview tools and agents can rely on them too.

- **Both profiles write them**; without script they do nothing.
- **Versioned:** a rename, removal or change of meaning bumps the version. The embedded runtime always matches its file; the version is for outside readers.
- **Paths are never selectors:** no path becomes an id, a class or part of a selector. The runtime finds elements by comparing `getAttribute()` results. The only ids are synthetic `<defs>` targets such as `arrowhead-0` and `traveler-envelope`.
- **No markup from data:** detail panels are built with `textContent` and DOM calls, never `innerHTML`.

## Document

The root is `<svg data-svugy-contract="2">`; its viewBox, width and height are the canvas: the union of every layer's root view. Its children, in order:

1. the runtime's `<style>`
2. `<title>TITLE</title>`, if the diagram has a title: the document's tooltip and the label of the elevator's root floor, which otherwise reads "root"
3. `<defs>`, shared by all layers: arrowhead markers and traveler symbols
4. one canvas `<rect>` covering every layer's root view
5. the layers
6. the runtime's `<script>`

The runtime's CSS and JavaScript use the writer's CDATA form.

## Layers

A path's depth is its segment count: root 0, top-level element 1. Layer K shows every element of depth ≤ K + 1:

- containers of depth ≤ K are open, their interior drawn on their slab;
- containers of depth K + 1 are closed: drawn and painted like a leaf (own shape, height and fill, else the leaf surface, never the slab's), no interior;
- an edge with both endpoints shown is drawn as written;
- an edge with a hidden endpoint is drawn from that endpoint's ancestor of depth K + 1; if both ends land on one element it disappears; lifted edges joining the same ordered pair merge into one.

With D the depth of the deepest container (0 without containers), the interactive document has layers 0 to D; layer D is the whole diagram. Each is a `<g data-svugy-layer="K">`, in increasing K, holding the layer's ground grid, the diagram's title, subtitle and legend (document coordinates, no data attributes; a click on them counts as a click on empty canvas), then the root level `<g data-svugy-level="" data-svugy-view="…">`. The root's path is the empty string.

Layers share one coordinate system but are laid out independently: an element's coordinates differ per layer, and a path is unique within a layer only. The renderer writes `data-svugy-shown=""` on layer D, and the CSS hides every layer without it, so a document whose script never runs (inside `<img>`, in an editor preview) shows the whole diagram.

The static profile draws layer D alone and unwrapped: ground grid, heading, legend and root level are direct children of the root, after the canvas; the viewBox is layer D's root view; there is no root title, no overlay and no traveler. Its groups carry the same attributes as the interactive layer D, `data-svugy-from` and `data-svugy-to` included. A document without `data-svugy-layer` is one fully disclosed layer.

## Groups

Every element, edge and stub group starts with `<title>LABEL</title><desc data-svugy-details="">JSON</desc>`; browsers show the title as a tooltip without script.

**Element:** `<g data-svugy-element="PATH">` holding its faces, its interior level if open, its labels, then its badges. Title: its label.

- Closed container: also `data-svugy-closed=""`; no interior level; its content starts with its sheet, a faint plate under its faces.
- A `link` resolving to a model path adds `data-svugy-goto="LEVEL"`; a click goes to LEVEL instead. A link to a leaf resolves to the leaf's enclosing level.
- A URL link that passes the sanitiser wraps the group in `<a href="…">`, with no data attribute.
- An open container's interior is `<g data-svugy-level="PATH" data-svugy-view="…" transform="…">` inside its group, with the same PATH.

**Connection:** `<g data-svugy-edge="EDGE" data-svugy-from="FROM" data-svugy-to="TO">`, FROM and TO being the endpoints it is drawn between in its layer. Title: its label, else EDGE.

- It holds its stroke `<path data-svugy-flow="">`, whose dash offset the runtime CSS animates.
- Interactive only: it also holds `<use data-svugy-traveler="NAME" href="#traveler-NAME">`, moved by SMIL `<animateMotion>`. NAME is `dot`, `envelope`, `parcel`, `document` or `key`; `<defs>` holds `<symbol id="traveler-NAME">` for each NAME used.
- A labelled connection has a second group with the same three attributes, title and details, painted after every arc of its level so labels sit on top. The runtime never assumes edge groups are unique.

**Lifted connection:** a connection whose EDGE names its merged edge, which is no edge of the source model: its ordinal follows those of the written edges joining the same pair in its layer, so it never repeats one. A merge of one edge keeps its label, style and traveler; a merge of several has no label, the default style and the `dot` traveler. Title: its label, else its endpoints' labels joined by " → ". Its details list the source edges it stands for.

**Badge:** an edge group inside its source's element group, with the three edge attributes: a mini of the target, the edge's only group, with no flow path or traveler. A click on it acts like a click inside its source's level.

**Stub:** each level group ends with `<g data-svugy-overlay="">`, holding what only the entered view shows; CSS hides it until that level is entered. Each stub marker in it is `<g data-svugy-stub="PATH" data-svugy-goto="LEVEL">`: PATH is the external endpoint as its layer shows it; LEVEL is the deepest container holding it, or the endpoint itself if it is a container, even a closed one. Title: "to " plus the endpoint's label.

**Values:**

- paths as `ElementPath` displays them: dot-separated, segments quoted when needed, `""` for the root;
- edges as `EdgeId` displays them: `(FROM -> TO)[ORDINAL]`;
- `data-svugy-view`: `min-x min-y width height` in document coordinates, every enclosing frame applied, fixed decimals; the viewBox the runtime animates to when the level is focused.

## Details

One JSON object per group, empty keys omitted:

- element: `{"kind":"element","path":…,"label":…,"caption":…,"metadata":{…}}`
- edge: `{"kind":"edge","edge":…,"from":…,"to":…,"label":…}`
- lifted: `{"kind":"lifted","from":…,"to":…,"label":…,"edges":[edge objects]}`
- stub: `{"kind":"stub","target":…,"edges":[edge or lifted objects]}`

## Runtime state

- **`data-svugy-shown`:** rendered on layer D; at start the runtime moves it to layer 0, or to a level link's layer, then to the layer it shows. Two layers carry it during a crossfade.
- **`data-svugy-entered=""`:** on exactly one level group of the shown layer, the focused one; `[data-svugy-shown] [data-svugy-entered] > [data-svugy-overlay]` is visible. Never rendered.
- **`data-svugy-dimmed=""`:** on the element and edge groups the hovered group dims. Never rendered.

The CSS toggles `visibility`, never `display:none`, so the runtime can measure a hidden layer. No rule may make anything visible inside a layer that isn't shown. At start the runtime adds:

- `tabindex="0"` on every element, edge and stub group, `tabindex="-1"` on the root;
- `role="button"` on the groups a click acts on: closed containers, open containers, `data-svugy-goto` groups, stubs;
- an `aria-label` from each group's title;
- the `svugy-panel*` classes (detail panel) and `svugy-elevator*` classes (elevator).

The runtime binds only to the SVG it lives in.

## Navigation

The runtime keeps the shown layer K and the focused level F, whose view fills the screen. F is open in layer K, so depth(F) ≤ K. Start: K = 0, F = root, unless the URL links a level.

**Level link:** a URL fragment `#svugy=PATH`, PATH being a level's `data-svugy-level` encoded by `encodeURIComponent`. At start, K becomes the first layer holding that level and F the level, its view shown without animation. A PATH that names no level, or any other fragment, leaves the start as it is. The runtime reads the fragment of the document it runs in: the SVG file's URL, or the page's for an SVG inlined in HTML.

- **Closed container,** even inside an open container below F: show layer K + 1, keep F. Every container of that depth opens at once, so an edge between children of two of them appears whichever was clicked.
- **Inside an open container below F:** focus the outermost open container between F and the target, the slab that was clicked. A click outside F (visible at the edges once zoomed in) applies the same rule from the nearest open level containing it.
- **Leaf or badge of F, or F's own slab:** show its details. Same for an edge of F; an edge homed below F acts like its home container.
- **`data-svugy-goto="LEVEL"`:** overrides the above. F becomes LEVEL; K becomes max(K, depth(LEVEL)).
- **Escape or a click on empty canvas:** undo one step: show layer K − 1 if K > depth(F), else focus F's parent; nothing at the root of layer 0.

Example: from the root of layer 0, click closed container A (layer 1), A's slab (F = A), then A's closed child a1 (layer 2). Three Escapes give A in layer 1, the root in layer 1, the root in layer 0.

Focusing animates the viewBox to F's `data-svugy-view`. A layer change crossfades the two layers while the viewBox moves to F's view in the new layer, starting from the view that puts the clicked element's new center where its old center was on screen. Under reduced motion both jump, the flow animation stops and travelers hide.

Hovering or focusing an element dims every element and edge group of the shown layer except:

- the element, its ancestors and its descendants;
- every edge with an endpoint among them;
- those edges' other endpoints and their ancestors.

Hovering an edge keeps only the edge, its endpoints and their ancestors lit. Stub markers and badges are never dimmed.

While F isn't the root, the elevator sits top right at a constant on-screen size. A toggle switches its two modes; it starts in chain mode on every load:

- **Chain:** the levels from the root to F, each labelled with its container's title, the root with the document's; F highlighted; a click on a level focuses it.
- **Mini-map:** the shown layer cropped to the view of F's parent, a rectangle marking F; a click focuses the parent.
