# Runtime contract

*Version 2. The attribute names are spelled once, in `crates/svugy/src/render/contract.rs` (`RuntimeContract`).*

The renderer (Rust) and the runtime (`crates/svugy/runtime/svugy.js` and `crates/svugy/runtime/svugy.css`) only talk through the `data-svugy-*` attributes in the rendered document. Both profiles emit them, so even a static file stays machine-readable; without script they do nothing. The static profile leaves out the overlays and the travelers, because it has no entered view and no animation.

Any rename, removal or change of meaning bumps the version. The runtime embedded in a file always matches that file, so the version number is really for outside readers, like a preview tool or an agent parsing rendered SVGs.

Element paths never become an id, a class or a CSS selector. They only travel as attribute values, and the runtime finds elements by comparing `getAttribute()` results, never by building a selector out of a path. The only ids in the document are synthetic ones like `arrowhead-0` and `traveler-envelope`, used for `<defs>` references. Detail panels are built from JSON with `textContent` and DOM calls, never `innerHTML`.

## Document

The root is `<svg data-svugy-contract="2">`. Its viewBox, width and height are layer 0's root view, where the runtime starts. Inside come `<title>TITLE</title>` when the diagram has a title, the runtime's `<style>`, the `<defs>` every layer shares (arrowhead markers and traveler symbols), one canvas `<rect>` covering the root view of every layer, then the layers, and the runtime's `<script>` as the root's last child. The root's title is the browser's tooltip for the whole document and the label of the elevator's root floor; without one, that floor reads "root".

## Layers

A layer draws the diagram disclosed down to one depth. A path's depth is its number of segments: the root's is 0, a top-level element's is 1. Layer K shows every element of depth K + 1 or less. Containers of depth K or less are open, their interior drawn on their slab; containers of depth K + 1 are closed, drawn and painted like a leaf: their own shape, height and fill, else the leaf surface, never the slab's, and no interior. An edge with a hidden endpoint is drawn from that endpoint's ancestor of depth K + 1 instead. If both its ends land on one element, it disappears, and the lifted edges joining the same ordered pair merge into one. Edges whose endpoints are both shown are drawn as written.

With D the depth of the deepest container (0 for a diagram without containers), the interactive document has layers 0 to D, and layer D is the whole diagram. Each layer is a `<g data-svugy-layer="K">`, in increasing K. It holds the layer's ground grid, the diagram's title and subtitle, the legend (these three in document coordinates, with no data attributes), then the layer's root level, `<g data-svugy-level="" data-svugy-view="…">`. A click on the title, subtitle or legend counts as a click on empty canvas. The root's path is the empty string.

The layers share one coordinate system but are laid out independently, so an element sits at different coordinates in different layers, and a path is unique within a layer, not across layers. The renderer writes `data-svugy-shown=""` on layer 0, and the CSS hides every layer without it, so a document whose script never runs, such as one inside `<img>`, shows layer 0.

The static profile draws layer D alone and writes it unwrapped: its ground grid, heading, legend and root level sit directly in the root, after the canvas, and the root's viewBox is layer D's root view. It writes no root title. Its groups carry the same attributes as the interactive profile's layer D, `data-svugy-from` and `data-svugy-to` included. A document without `data-svugy-layer` is a single, fully disclosed layer.

## Groups

Every solid is a `<g data-svugy-element="PATH">` holding its faces, then its interior level if it is open, then its labels, then its badges. A closed container's group also carries `data-svugy-closed=""`. It has no interior level and starts with its sheet, a faint plate offset under its faces that hints there is more inside. If the element's `link:` resolves to a model path, the group gets `data-svugy-goto="LEVEL"`, and a click goes to that level instead. A link to a leaf resolves to the leaf's enclosing level. A URL link wraps the group in `<a href="…">` after the link sanitiser, with no data attribute. An open container's interior is a `<g data-svugy-level="PATH" data-svugy-view="…" transform="…">` nested inside its solid's group, with the same PATH.

A connection is a `<g data-svugy-edge="EDGE" data-svugy-from="FROM" data-svugy-to="TO">`, where FROM and TO are the paths of the endpoints it is drawn between in its layer. It holds its stroke, `<path data-svugy-flow="">`, whose dash offset the runtime CSS animates. In the interactive profile it also holds its traveler, `<use data-svugy-traveler="NAME" href="#traveler-NAME">`, moved by SMIL `<animateMotion>` and hidden under reduced motion. NAME is one of `dot`, `envelope`, `parcel`, `document` and `key`, and the `<defs>` hold a `<symbol id="traveler-NAME">` for each NAME the document uses.

A labelled connection gets a second group with the same `data-svugy-edge`, `data-svugy-from` and `data-svugy-to`, painted after every arc of its level so labels sit on top; both carry the same title and details, and the runtime never assumes edge groups are unique. A lifted connection's EDGE names its merged edge, which is no edge of the source model: its ordinal follows those of the written edges joining the same pair in its layer, so it never repeats one. Its details list the source edges it stands for. A merge of one edge keeps that edge's label, style and traveler; a merge of several has no label, the default style and the `dot` traveler. A badge is an edge group inside its source's element group, with `data-svugy-from` and `data-svugy-to` like any edge group: a mini of the target, the edge's only group, with no flow path or traveler. Clicking it acts like clicking inside its source's level.

Each level group ends with a `<g data-svugy-overlay="">` holding what only the entered view shows; CSS hides it until that level is entered. Inside it, each stub marker is a `<g data-svugy-stub="PATH" data-svugy-goto="LEVEL">`. PATH is the external endpoint as its layer shows it. LEVEL is the deepest container holding it, or the endpoint itself if it is a container, even a closed one.

Every element, edge and stub group starts with `<title>LABEL</title><desc data-svugy-details="">JSON</desc>`. Browsers show the title as a native tooltip even without script. A stub's title is "to " plus the external endpoint's label. A lifted connection's title is its label when it kept one, and otherwise its endpoints' labels joined by " → ". The runtime's CSS and JavaScript use the writer's CDATA form, and the runtime only binds to the SVG it lives in.

Paths are written the way `ElementPath` displays them: dot-separated, with segments quoted when needed, and the empty string for the root. Edges are written the way `EdgeId` displays them: `(FROM -> TO)[ORDINAL]`. `data-svugy-view` is `min-x min-y width height` in document coordinates, with every enclosing frame already applied and fixed decimals; it's the viewBox the runtime animates to when the level is focused.

## Details

The details JSON has one object per group, and empty keys are left out:

- an element is `{"kind":"element","path":…,"label":…,"caption":…,"metadata":{…}}`
- an edge is `{"kind":"edge","edge":…,"from":…,"to":…,"label":…}`
- a lifted connection is `{"kind":"lifted","from":…,"to":…,"label":…,"edges":[edge objects]}`
- a stub is `{"kind":"stub","target":…,"edges":[edge or lifted objects]}`

## Runtime state

Some state belongs to the runtime alone and is never rendered:

- **`data-svugy-shown`** is rendered on layer 0, and the runtime then moves it to the layer it shows. Two layers carry it while the runtime crossfades between them.
- **`data-svugy-entered=""`** sits on exactly one level group of the shown layer, the focused one. `[data-svugy-entered] > [data-svugy-overlay]` is visible.
- **`data-svugy-dimmed=""`** sits on the element and edge groups that the hovered group dims.

The CSS always toggles `visibility`, never `display:none`, so the runtime can measure a hidden layer before showing it. No rule may make anything visible inside a layer that isn't shown. At start-up the runtime also adds:

- `tabindex` on every element, edge and stub group, and `tabindex="-1"` on the root SVG
- `role="button"` on the groups a click acts on: closed containers, open containers, `data-svugy-goto` groups and stubs
- an `aria-label` from each group's title
- the `svugy-panel*` classes for the detail panel and the `svugy-elevator*` classes for the elevator

## Navigation

The runtime keeps two things: the shown layer K and the focused level F, whose view fills the screen. F is open in the shown layer, so depth(F) ≤ K. It starts at layer 0, focused on the root.

- **A closed container,** even one inside an open container below F: a click shows layer K + 1 and keeps F. Every container of that depth opens at once, so an edge between the children of two of them appears whichever one you clicked.
- **Inside an open container below F:** a click focuses the outermost open container between F and what you clicked, the slab you clicked on. If the click lands outside F (visible at the edges once zoomed in), the same rule starts from the nearest open level that contains it.
- **A leaf or badge of F, or F's own slab:** a click shows its details. So does a click on an edge of F; a click on an edge homed below F acts like a click on its home container.
- **`data-svugy-goto="LEVEL"`:** it overrides all of the above. F becomes LEVEL and K becomes max(K, depth(LEVEL)).
- **Escape, or a click on empty canvas:** it undoes one step. It shows layer K − 1 when K > depth(F) and otherwise focuses F's parent; it does nothing at the root of layer 0.

For example: from the root of layer 0, click the closed container A (layer 1), then A's slab (F = A), then A's closed child a1 (layer 2). Pressing Escape three times then gives A in layer 1, the root in layer 1, and the root in layer 0.

Focusing animates the viewBox to F's `data-svugy-view`. Changing layer crossfades the two layers while the viewBox goes to F's view in the new one, starting from the view that puts the clicked element's new center where its old one was on screen. With reduced motion both jump.

Hovering, or focusing, an element dims every element and edge group of the shown layer that has nothing to do with it. These stay lit:

- the element itself, its ancestors and its descendants
- every edge with an endpoint among them
- those edges' other endpoints, with their ancestors

Hovering an edge keeps only the edge, its endpoints and their ancestors lit. Stub markers and badges are never dimmed.

While F isn't the root, the elevator sits at the top right at a constant on-screen size. It has two modes, and a toggle on it switches between them; it starts in chain mode on every load:

- **Chain:** the levels from the root down to F, each labelled with its container's title and the root with the document's, F highlighted; clicking a level focuses it.
- **Mini-map:** a copy of the shown layer cropped to the view of F's parent, with a rectangle marking F; clicking it focuses the parent.
