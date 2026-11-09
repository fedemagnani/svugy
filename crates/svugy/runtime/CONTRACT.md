# Runtime contract

*Version 2. Attribute names are spelled once, in `RuntimeContract` (`crates/svugy/src/render/contract/mod.rs`).*

The renderer and the runtime (`crates/svugy/runtime/svugy.js`, `svugy.css`) communicate only through the `data-svugy-*` attributes below. Preview tools and agents can rely on them too.

- **Both profiles write them**; without script they do nothing.
- **Versioned:** a rename, removal or change of meaning bumps the version. The embedded runtime always matches its file; the version is for outside readers.
- **Paths are never selectors:** no path becomes an id, a class or part of a selector. The runtime finds elements by comparing `getAttribute()` results. The only ids are synthetic `<defs>` targets such as `arrowhead-0`, `traveler-envelope` and `ground-grid`.
- **No markup from data:** the detail and description panels are built with `textContent` and DOM calls, never `innerHTML`.

## Document

The root is `<svg data-svugy-contract="2">`; its viewBox, width and height are the canvas: the union of every layer's root view. Its children, in order:

1. the runtime's `<style>`
2. `<title>TITLE</title>`, if the diagram has a title: the document's tooltip
3. `<desc data-svugy-details="">JSON</desc>`, if the diagram has a description: the diagram details (see [Details](#details))
4. `<defs>`, shared by all layers: arrowhead markers, traveler symbols and the `ground-grid` pattern
5. one canvas `<rect>` covering every layer's root view
6. the ground: one `<rect>` filled with `ground-grid`, one projected grid cell repeated 10⁶ user units past the origin on every side, so the grid never ends when the reader moves the view; the runtime CSS paints the canvas color behind it
7. the layers
8. the runtime's `<script>`

The runtime's CSS and JavaScript use the writer's CDATA form.

## Layers

A path's depth is its segment count: root 0, top-level element 1. Layer K shows every element of depth ≤ K + 1:

- containers of depth ≤ K are open, their interior drawn on their slab;
- containers of depth K + 1 are closed: drawn and painted like a leaf (own shape, height and fill, else the leaf surface, never the slab's), no interior;
- an edge with both endpoints shown is drawn as written;
- an edge with a hidden endpoint is drawn from that endpoint's ancestor of depth K + 1; if both ends land on one element it disappears; lifted edges joining the same ordered pair merge into one.

With D the depth of the deepest container (0 without containers), the interactive document has layers 0 to D; layer D is the whole diagram. Each is a `<g data-svugy-layer="K">`, in increasing K, holding the diagram's title and subtitle inside `<g data-svugy-heading="">`, the legend (document coordinates, no data attributes; a click on it counts as a click on empty canvas), then the root level `<g data-svugy-level="" data-svugy-view="…">`. The root's path is the empty string.

Layers share one coordinate system but are laid out independently: an element's coordinates differ per layer, and a path is unique within a layer only. The renderer writes `data-svugy-shown=""` on layer D, and the CSS hides every layer without it, so a document whose script never runs (inside `<img>`, in an editor preview) shows the whole diagram.

The static profile draws layer D alone and unwrapped: ground grid, heading, legend and root level are direct children of the root, after the canvas; the viewBox is layer D's root view; there is no root title, no diagram details, no heading group, no overlay, no traveler and no endless ground: its ground grid ends one cell past the diagram. Its groups carry the same attributes as the interactive layer D, `data-svugy-from` and `data-svugy-to` included. A document without `data-svugy-layer` is one fully disclosed layer.

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
- A connection traced flat on its level's floor comes before that level's element groups, which stand on it; an arc comes after them.
- A bundled connection is an arc group for its trunk in its home level, holding the traveler that runs the whole route, plus one traced group per branch in the level of the container the branch runs on. Only the group reaching the target ends in an arrowhead.
- A labelled connection has a second group with the same three attributes, title and details, painted after every element, trace and arc of its level so labels sit on top; a bundled connection's label group is in the level of the branch carrying it, the target's if any. The runtime never assumes edge groups are unique.

**Lifted connection:** a connection whose EDGE names its merged edge, which is no edge of the source model: its ordinal follows those of the written edges joining the same pair in its layer, so it never repeats one. A merge of one edge keeps its label, style and traveler; a merge of several has no label, the default style and the `dot` traveler. Title: its label, else its endpoints' labels joined by " → ". Its details list the source edges it stands for.

**Badge:** an edge group inside its source's element group, with the three edge attributes: a mini of the target, the edge's only group, with no flow path or traveler. A click on it acts like a click inside its source's level.

**Stub:** each level group ends with `<g data-svugy-overlay="">`, holding what only the entered view shows; CSS hides it until that level is entered. Each stub marker in it is `<g data-svugy-stub="PATH" data-svugy-goto="LEVEL">`: PATH is the external endpoint as its layer shows it; LEVEL is the deepest container holding it, or the endpoint itself if it is a container, even a closed one. Title: "to " plus the endpoint's label.

**Values:**

- paths as `ElementPath` displays them: dot-separated, segments quoted when needed, `""` for the root;
- edges as `EdgeId` displays them: `(FROM -> TO)[ORDINAL]`;
- `data-svugy-view`: `min-x min-y width height` in document coordinates, every enclosing frame applied, fixed decimals; the viewBox the runtime animates to when the level is focused.

## Details

One JSON object per group, plus one for the diagram, empty keys omitted:

- element: `{"kind":"element","path":…,"label":…,"caption":…,"metadata":{…},"description":…}`
- edge: `{"kind":"edge","edge":…,"from":…,"to":…,"label":…,"description":…}`
- lifted: `{"kind":"lifted","from":…,"to":…,"label":…,"edges":[edge objects]}`
- stub: `{"kind":"stub","target":…,"edges":[edge or lifted objects]}`
- diagram: `{"kind":"diagram","description":…}`, a child of the root

A `description` is the author's Markdown, unrendered. Only the interactive profile writes `description` keys and the diagram object; the static profile writes neither.

## Runtime state

- **`data-svugy-shown`:** rendered on layer D; at start the runtime moves it to layer 0, or to a level link's layer, then to the layer it shows. Two layers carry it during a crossfade.
- **`data-svugy-entered=""`:** on exactly one level group of the shown layer, the focused one; `[data-svugy-shown] [data-svugy-entered] > [data-svugy-overlay]` is visible. Never rendered.
- **`data-svugy-dimmed=""`:** on the element and edge groups the hovered group dims. Never rendered.
- **Heading `transform`:** on every `data-svugy-heading` group, set at start and whenever the viewBox or the SVG's size changes, so the heading sits 20 px inside the top-left corner of the SVG's on-screen box at 1.2 times its rendered size, wherever the view moves. Without script the heading stays where rendered.
- **`data-svugy-panning=""`:** on the root while a drag or a pinch moves the view; the cursor shows grabbing and the detail panel hides. Never rendered.
- **`svugy-highlighted` class:** for 1.2 s, on the shown layer's groups drawing a followed description link's target (see [Navigation](#navigation)). Never rendered.

The CSS toggles `visibility`, never `display:none`, so the runtime can measure a hidden layer. No rule may make anything visible inside a layer that isn't shown. At start the runtime adds:

- `tabindex="0"` on every element, edge and stub group, `tabindex="-1"` on the root;
- `role="button"` on every element, edge and stub group;
- an `aria-label` from each group's title;
- `tabindex="0"`, `role="button"` and `aria-label="diagram description"` on each heading group;
- the `svugy-panel*` classes (detail panel) and `svugy-description*` classes (description panel).

The runtime binds only to the SVG it lives in.

## Navigation

The runtime keeps the shown layer K and the focused level F, whose view fills the screen. F is open in layer K, so depth(F) ≤ K. Start: K = 0, F = root, unless the URL links a level.

**Level link:** a URL fragment `#svugy=PATH&layer=K`, PATH being a level's `data-svugy-level` encoded by `encodeURIComponent`, `&layer=K` optional. At start, F becomes the level and K becomes layer K if that layer holds the level, else the first layer holding it; F's view shows without animation. A PATH that names no level, or any other fragment, leaves the start as it is. `#svugy=&layer=1` starts at the root with layer 1 shown. The runtime reads the fragment of the document it runs in: the SVG file's URL, or the page's for an SVG inlined in HTML.

- **Closed container,** even inside an open container below F: show layer K + 1, keep F. Every container of that depth opens at once, so an edge between children of two of them appears whichever was clicked.
- **Leaf,** at any depth of layer K: F becomes the leaf's enclosing level, then the view animates to a frame centred on the leaf: on each side, 3 times the leaf's on-screen box but at most 70% of that level's view, never less than the leaf plus 12 user units each way, and at least 80 user units wide. That frame counts as a zoom away from F's view, so a step out first returns to F's view. Show the leaf's description; its details only when the view was already on that frame.
- **Inside an open container below F:** focus the outermost open container between F and the target, the slab that was clicked. A click outside F (visible at the edges once zoomed in) applies the same rule from the nearest open level containing it.
- **Badge of F, or F's own slab:** show its details and its description. Same for an edge of F; an edge homed below F acts like its home container.
- **`data-svugy-goto="LEVEL"`:** overrides the above. F becomes LEVEL; K becomes max(K, depth(LEVEL)).
- **Heading:** show the diagram's description; K and F stay.
- **Description link** to target T (see the Markdown subset below): focus T's reading level, in the first layer from K on that draws that level open and an edge T as written. The reading level is a container's own level, a leaf's or a badge's enclosing level, an arc's home level. The description panel keeps the text it showed, so the reader keeps reading; Escape then shows F's description. Put the `svugy-highlighted` class on T's groups in the shown layer.
- **Escape or a click on empty canvas:** undo one step: if a click changed the description panel, show F's description again; then, if the reader zoomed or panned away from F's view, animate back to it, else focus F's parent in layer K, so each step out widens the view; at the root, show layer K − 1; nothing at the root of layer 0. A double click's second click does nothing.

Example: from the root of layer 0, click closed container A (layer 1), A's slab (F = A), then A's closed child a1 (layer 2). Three Escapes give the root in layer 2, the root in layer 1, the root in layer 0.

The reader can also move the view without navigating; K and F stay:

- **Drag** with the main mouse button or a pen, from anywhere: pans. A pointer that moves less than 4 px is a click; a drag never clicks. The diagram's text is not selectable, so a drag never selects labels; the description panel's text is.
- **Pinch, or scroll while holding Ctrl or ⌘:** zooms in around the pointer; zooms out toward F's view, the centre covering the same share of the way home as the width, so zooming back out to F's zoom lands exactly on F's view. From twice the canvas width down to 80 user units wide. Plain scrolling is left to the page.
- **Two fingers** pan and pinch-zoom together; one finger scrolls the page.

Nothing clamps the view: the ground reaches past every side. Any navigation animates to its target view, discarding the move.

Focusing animates the viewBox to F's `data-svugy-view`, fitted to the canvas aspect and centred on it. A layer change crossfades the two layers while the viewBox moves to F's view in the new layer, starting from the view that puts the clicked element's new center where its old center was on screen. Under reduced motion both jump, the flow animation stops and travelers hide.

Hovering or focusing an element dims every element and edge group of the shown layer except:

- the element, its ancestors and its descendants;
- every edge with an endpoint among them;
- those edges' other endpoints and their ancestors.

Hovering an edge keeps only the edge, its endpoints and their ancestors lit. Stub markers and badges are never dimmed.

The description panel sits top right at a constant on-screen width, its height fitting its content up to the viewport's. It shows only the author's `description` from the details JSON, rendered as Markdown:

- **F = root, or the heading clicked:** the diagram's description.
- **F, or a clicked element or badge:** the element's description, also when the click opened or focused it. A click that jumps through `data-svugy-goto` shows F's description instead.
- **A clicked written edge:** its description.
- **A clicked lifted edge:** for each source edge in `edges` that has a description, a heading with its label, else its endpoints' labels joined by " → ", then its description.

Without any description there is no panel, and a click on an undescribed group closes an open one.

The panel floats over the diagram: opening, changing or closing it never moves the view.

Descriptions render a Markdown subset:

- paragraphs, one line per source line;
- `-` and `*` bullet lists and `1.` numbered lists; an indented item nests under the item above, one level deep; an indented line continues the open item;
- `#`, `##` and `###` headings, drawn as `h4`, `h5` and `h6`;
- fenced code between two ```` ``` ```` lines, preformatted, without inline formatting;
- inline `**bold**`, `*em*`, `` `code` ``, and `[text](url)` links whose URL is http(s) or has no scheme and no space or control character;
- `[text](#TARGET)` description links, drawn as a `<button type="button">` holding underlined `text`; on hover or keyboard focus the text takes T's colour (an element's top-face fill, an edge's stroke) when it has at least 3:1 contrast with `--svugy-canvas`, else `--svugy-container-blue-ink`. TARGET names an element as its `data-svugy-element` path, or a written edge as `FROM->TO[ORDINAL]`, FROM and TO its `data-svugy-from` and `data-svugy-to`, ORDINAL the one ending its `data-svugy-edge`; `[ORDINAL]` is required. The runtime resolves TARGET in layer D;
- any other link, and a TARGET naming nothing, renders as its text.

Author text only ever becomes text nodes.
