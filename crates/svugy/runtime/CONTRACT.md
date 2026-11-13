# Runtime contract

*Version 4. Attribute names are spelled once, in `RuntimeContract` (`crates/svugy/src/render/contract/mod.rs`).*

The renderer and the runtime (`crates/svugy/runtime/svugy.js`, `svugy.css`) communicate only through the `data-svugy-*` attributes below. Preview tools and agents can rely on them too.

- **Both profiles write them**; without script they do nothing.
- **Versioned:** a rename, removal or change of meaning bumps the version. The embedded runtime always matches its file; the version is for outside readers.
- **Paths are never selectors:** no path, workflow id or step id becomes an id, a class or part of a selector. The runtime finds elements by comparing `getAttribute()` results. The only ids are synthetic `<defs>` targets such as `arrowhead-0`, `traveler-envelope` and `ground-grid-isometric`.
- **No markup from data:** the description panel and the workflow menu are built with `textContent` and DOM calls, never `innerHTML`.

## Document

The root is `<svg data-svugy-contract="4">`; its viewBox, width and height are the canvas: the union of every layer's root view, over every workflow, from the author's camera. Its children, in order:

1. the runtime's `<style>`
2. `<defs>`, shared by all workflows and cameras: arrowhead markers, traveler symbols and one `ground-grid-ANGLE` pattern per camera
3. one canvas `<rect>` covering the canvas
4. the workflows
5. the runtime's `<script>`

The runtime's CSS and JavaScript use the writer's CDATA form.

## Workflows

The interactive document draws every workflow of the model, each a `<g data-svugy-workflow="ID">`, in written order. The renderer writes `data-svugy-shown=""` on the workflow its caller shows, the first unless the caller names another; the CSS hides every workflow without it. Each holds:

1. `<desc data-svugy-details="">JSON</desc>`: the workflow details (see [Details](#details))
2. its cameras

Each workflow is laid out on its own, from its own elements, edges and overrides: an element's coordinates differ per workflow, and a path is unique within one layer of one camera of one workflow only.

## Cameras

A workflow draws its diagram from two angles, each a `<g data-svugy-camera="ANGLE">`, ANGLE being `isometric` or `top`, in that order. The author picks the shown one with the frontmatter's `view` key, `isometric` by default; the renderer writes `data-svugy-shown=""` on its group in every workflow, and the CSS hides every camera without it.

Every camera of a workflow projects the same layout, laid out once from the author's angle: an element keeps its plan position whichever camera shows it. Each holds:

1. the ground: one `<rect>` filled with `ground-grid-ANGLE`, one projected grid cell repeated 10⁶ user units past the origin on every side, so the grid never ends when the reader moves the view; the runtime CSS paints the canvas color behind it
2. its layers

Cameras share one coordinate system but project independently: an element's coordinates differ per camera. `top` looks straight down: it drops every side face, lays container tags flat on the ground below their slab instead of hanging them, and draws connection labels at three quarters of their size.

## Layers

A path's depth is its segment count: root 0, top-level element 1. Layer K shows every element of depth ≤ K + 1:

- containers of depth ≤ K are open, their interior drawn on their slab;
- containers of depth K + 1 are closed: drawn and painted like a leaf (own shape, height and fill, else the leaf surface, never the slab's), no interior;
- an edge with both endpoints shown is drawn as written;
- an edge with a hidden endpoint is drawn from that endpoint's ancestor of depth K + 1; if both ends land on one element it disappears; lifted edges joining the same ordered pair merge into one.

A workflow draws only the elements it shows: the endpoints of its edges, the elements its steps list bare, and their ancestors. A container none of whose children the workflow shows is a leaf there.

With D the depth of the workflow's deepest container (0 without containers), every camera has layers 0 to D; layer D is the whole workflow. Each is a `<g data-svugy-layer="K">`, in increasing K, holding the workflow's title and subtitle inside `<g data-svugy-heading="">`, then the root level `<g data-svugy-level="" data-svugy-view="…">`. The root's path is the empty string.

Layers share one coordinate system but are laid out independently: an element's coordinates differ per layer, and a path is unique within a layer only. The renderer writes `data-svugy-shown=""` on every camera's layer D, and the CSS hides every layer without it, so a document whose script never runs (inside `<img>`, in an editor preview) shows the shown workflow whole, from the author's angle.

The static profile draws one workflow, the one the caller names, at layer D from the author's angle alone and unwrapped: ground grid, heading and root level are direct children of the root, after the canvas; the viewBox is layer D's root view; there is no workflow group, no workflow details, no camera group, no heading group, no overlay, no traveler and no endless ground: its ground grid ends one cell past the diagram. Its groups carry the same attributes as the interactive layer D, `data-svugy-from`, `data-svugy-to` and `data-svugy-steps` included. A document without `data-svugy-workflow` is one workflow; one without `data-svugy-camera` is one camera; one without `data-svugy-layer` is one fully disclosed layer.

## Groups

Every element and edge group starts with `<title>LABEL</title><desc data-svugy-details="">JSON</desc>`, every stub group with its `<title>` alone; browsers show the title as a tooltip without script.

Every element, edge, label, badge and stub group carries `data-svugy-steps="STEP …"`: the ids of the workflow's steps it takes part in, space-separated, in step order, never empty.

- written edge: the steps listing it;
- element: the steps listing it, one of its descendants, or an edge ending at either, so a container takes part in every step of its interior;
- lifted connection: the union of its source edges' steps;
- stub: the union of the steps of the edges it stands for.

**Element:** `<g data-svugy-element="PATH">` holding its faces, its interior level if open, its labels, then its badges. Title: its label.

- Closed container: also `data-svugy-closed=""`; no interior level; its content starts with its sheet, a faint plate under its faces.
- A theme that shades faces with hatching draws each face as its fill paths `<path data-svugy-face-fill="">` (an opaque canvas underlay, then the shade as a translucent wash), its hatch strokes `<path data-svugy-hachure="" fill="none">`, then its outline `<path fill="none">`. Hatch strokes take no pointer events; hover, focus, highlight and selection restyle only the outline.
- A `link` resolving to a path the workflow shows adds `data-svugy-goto="LEVEL"`; a click goes to LEVEL instead. A link to a leaf resolves to the leaf's enclosing level. A link to a path the workflow doesn't show adds nothing.
- A URL link that passes the sanitiser wraps the group in `<a href="…">`, with no data attribute.
- An open container's interior is `<g data-svugy-level="PATH" data-svugy-view="…" transform="…">` inside its group, with the same PATH.

**Connection:** `<g data-svugy-edge="EDGE" data-svugy-from="FROM" data-svugy-to="TO">`, FROM and TO being the endpoints it is drawn between in its layer. Title: its label, else EDGE.

- It holds its stroke `<path data-svugy-flow="">`, whose dash offset the runtime CSS animates.
- Interactive only: it also holds `<use data-svugy-traveler="NAME" href="#traveler-NAME">`, moved by SMIL `<animateMotion>`. NAME is `dot`, `envelope`, `parcel`, `document` or `key`; `<defs>` holds `<symbol id="traveler-NAME">` for each NAME used.
- A connection traced flat on its level's floor comes before that level's element groups, which stand on it; an arc comes after them.
- A bundled connection is an arc group for its trunk in its home level, holding the traveler that runs the whole route, plus one traced group per branch in the level of the container the branch runs on. Only the group reaching the target ends in an arrowhead.
- A labelled connection has a second group with the same three attributes, title and details, painted after every element, trace and arc of its level so labels sit on top; a bundled connection's label group is in the level of the branch carrying it, the target's if any. The runtime never assumes edge groups are unique.

**Lifted connection:** a connection whose EDGE names its merged edge, which is no edge of the workflow: its ordinal follows those of the written edges joining the same pair in its layer, so it never repeats one. A merge of one edge keeps its label, style and traveler; a merge of several has no label, the default style and the `dot` traveler. Title: its label, else its endpoints' labels joined by " → ". Its details list the source edges it stands for.

**Badge:** an edge group inside its source's element group, with the three edge attributes: a mini of the target, the edge's only group, with no flow path or traveler. A click on it acts like a click inside its source's level.

**Stub:** each level group ends with `<g data-svugy-overlay="">`, holding what only the entered view shows; CSS hides it until that level is entered. Each stub marker in it is `<g data-svugy-stub="PATH" data-svugy-goto="LEVEL">`: PATH is the external endpoint as its layer shows it; LEVEL is the deepest container holding it, or the endpoint itself if it is a container, even a closed one. Title: "to " plus the endpoint's label.

**Values:**

- paths as `ElementPath` displays them: dot-separated, segments quoted when needed, `""` for the root;
- edges as `EdgeId` displays them: `(FROM -> TO)[ORDINAL]`, ordinals counted within the workflow;
- workflow and step ids as written: ASCII letters, digits, `_` and inner `-`, never a space;
- `data-svugy-view`: `min-x min-y width height` in document coordinates, every enclosing frame applied, fixed decimals; the viewBox the runtime animates to when the level is focused.

## Details

One JSON object per element and edge group, plus one per workflow, empty keys omitted; a stub carries none, since it only jumps:

- element: `{"kind":"element","path":…,"label":…,"caption":…,"code":…,"metadata":{…},"description":…}`, with the workflow's overrides applied
- edge: `{"kind":"edge","edge":…,"from":…,"to":…,"label":…,"description":…}`
- lifted: `{"kind":"lifted","from":…,"to":…,"label":…,"edges":[edge objects]}`
- workflow: `{"kind":"workflow","id":…,"title":…,"subtitle":…,"description":…,"steps":[step objects]}`, the first child of its workflow group
- step: `{"id":…,"title":…,"description":…,"layer":K,"items":[{"edge":EDGE} or {"element":PATH}]}`, items in written order; K = max(0, d − 1), d being the greatest depth among the step's listed elements and edge endpoints: the shallowest layer drawing every item as written

A `description` is the author's Markdown, unrendered, with every description link target resolved in the group's workflow (see the Markdown subset below). Only the interactive profile writes `description` keys and the workflow object; the static profile writes neither.

## Runtime state

- **`data-svugy-shown`:** rendered on the shown workflow, on the author's camera of every workflow and on every camera's layer D; at start the runtime keeps it on that workflow, or the one a link names, on its author's camera and moves it to its layer 0, or to a level link's layer, then to the workflow, camera and layer it shows. Two layers carry it during a crossfade.
- **`data-svugy-entered=""`:** on exactly one level group of the shown layer, the focused one; `[data-svugy-shown] [data-svugy-entered] > [data-svugy-overlay]` is visible. Never rendered.
- **`data-svugy-dimmed=""`:** on the element and edge groups of the shown layer that the focused step or the hovered group dims. Never rendered.
- **`data-svugy-selected=""`:** on the groups drawing the selected item in the shown layer, for an element the group of its closest ancestor that layer draws when it hides the element; a selected group is never dimmed. Never rendered.
- **Heading `transform`:** on every `data-svugy-heading` group, set at start and whenever the viewBox or the SVG's size changes, so the heading sits 20 px inside the top-left corner of the SVG's on-screen box at 1.2 times its rendered size, wherever the view moves. Without script the heading stays where rendered.
- **`data-svugy-panning=""`:** on the root while a drag or a pinch moves the view; the cursor shows grabbing. Never rendered.
- **`svugy-highlighted` class:** for 1.2 s, on the shown layer's groups drawing a followed description link's target (see [Navigation](#navigation)). Never rendered.

The CSS toggles the visibility of workflows, cameras and layers, never their `display`, so the runtime can measure a hidden layer. No rule may make anything visible inside a workflow or a layer that isn't shown. At start the runtime adds:

- `tabindex="0"` on every element, edge and stub group, `tabindex="-1"` on the root;
- `role="button"` on every element, edge and stub group;
- an `aria-label` from each group's title;
- on each heading group, when the model has more than one workflow: `tabindex="0"`, `role="button"`, `aria-haspopup="listbox"`, `aria-label="choose workflow"` and a `▾` after the title;
- the `svugy-panel*` classes (description panel), `svugy-workflows*` classes (workflow menu) and `svugy-cameras*` classes (camera switch); all three take the root's `font-family` and derive their colours from `--svugy-canvas`, `--svugy-text` and `--svugy-text-muted`.

The runtime binds only to the SVG it lives in.

## Navigation

The runtime keeps the shown workflow W, its shown camera C, its shown layer K, the focused level F, whose view fills the uncovered area, the panel state P, the focused step S and the selected item I. F is open in layer K, so depth(F) ≤ K. Start: W = the rendered shown workflow, C = the author's camera, K = 0, F = root, P = hidden, no S, no I, unless the URL links a level.

The **uncovered area** is the SVG's on-screen box minus the description panel: minus its width when it is a side panel, minus its current height when it is a sheet; a hidden panel's tab or handle covers nothing. Every fit below fits into it. Opening, closing or resizing the panel never moves the view.

**Workflow switch:** with more than one workflow, activating the heading opens a menu below it listing every workflow's title and subtitle, `aria-selected` on W's. Choosing another workflow shows it: W changes, C stays, K = 0, F = root, P = hidden, S and I are cleared, and the view jumps to the root's view. Escape or a click outside closes the menu.

**Camera switch:** with more than one camera, the runtime pins one `<button>` per camera, labelled with its ANGLE and `aria-pressed` on C's, 12 px inside the bottom-left corner of the uncovered area at a constant on-screen size. Pressing another camera's button swaps C for it at once, keeps K and F, and jumps the view to F's view in the new camera, discarding any pan or zoom.

**Level link:** a URL fragment `#svugy=PATH&layer=K&workflow=ID&step=ID`, PATH being a level's `data-svugy-level` encoded by `encodeURIComponent`, every key but `svugy` optional. At start, W becomes workflow ID if one has that id; F becomes the level and K becomes layer K if that layer holds the level, else the first layer holding it; F's view shows without animation. With `step=ID` naming a step of W, the panel opens on it (see [Steps](#steps)), `expanded` on a sheet, with the steps card expanded. A PATH that names no level of W, or any other fragment, leaves the start as it is, its workflow and step ignored. `#svugy=&layer=1` starts at the root with layer 1 shown.

The runtime reads the fragment of the document it runs in: the SVG file's URL, or the page's for an SVG inlined in HTML.

**Clicks in the scene:**

- **Closed container,** even inside an open container below F: show layer K + 1, keep F. Every container of that depth opens at once, so an edge between children of two of them appears whichever was clicked.
- **Leaf,** at any depth of layer K: F becomes the leaf's enclosing level, then the view animates to a frame centred on the leaf: on each side, 3 times the leaf's on-screen box but at most 70% of that level's view, never less than the leaf plus 12 user units each way, and at least 80 user units wide. That frame counts as a zoom away from F's view, so a step out first returns to F's view.
- **Inside an open container below F:** focus the outermost open container between F and the target, the slab that was clicked. A click outside F (visible at the edges once zoomed in) applies the same rule from the nearest open level containing it.
- **Badge of F, F's own slab, or an edge of F:** no navigation; an edge homed below F acts like its home container.
- **`data-svugy-goto="LEVEL"`:** overrides the above. F becomes LEVEL; K becomes max(K, depth(LEVEL)).

Every click on an element, edge or badge also selects it: I becomes the clicked item and its component card opens (see [Description panel](#description-panel)). The panel opens and resizes first, so the click's view fits the area it leaves uncovered. When the click opens the panel on a step, the click's navigation wins: the step's effects are skipped. A `data-svugy-goto` jump and a stub select nothing.

**Description link** to target T (see the Markdown subset below):

- element or edge: I becomes T; then, once the panel has resized, focus T's reading level, in the first layer from K on that draws that level open and an edge T as written. The reading level is a container's own level, a leaf's or a badge's enclosing level, an arc's home level. Put the `svugy-highlighted` class on T's groups in the shown layer.
- step `WORKFLOW/STEP`: if WORKFLOW isn't W, switch to it as the workflow switch does, keeping the panel open; S becomes STEP, with the effects of a step change.

**Escape or a click on empty canvas:** undo one step: if I is set, clear it and return to the layer and focused level the reader stood on before selecting it, at that level's view, consecutive selections counting as one; if the reader moved to another layer or level since, only animate back to F's view; else, if the reader zoomed or panned away from F's view, animate back to it, else focus F's parent in layer K, so each step out widens the view; at the root, show layer K − 1; at the root of layer 0, make S slot 0, hiding the panel when W has no overview, so nothing dims. The second click of a double click on empty canvas does nothing; a canvas click right after a click on a group is never a second click.

Example: from the root of layer 0, click closed container A (layer 1, I = A), A's slab (F = A), then A's closed child a1 (layer 2, I = a1). Four Escapes clear I, then give the root in layer 2, the root in layer 1, the root in layer 0.

The reader can also move the view without navigating; K and F stay:

- **Drag** with the main mouse button or a pen, from anywhere in the uncovered area: pans. A pointer that moves less than 4 px is a click; a drag never clicks. The diagram's text is not selectable, so a drag never selects labels; the panel's text is.
- **Pinch, or scroll while holding Ctrl or ⌘:** zooms in around the pointer; zooms out toward F's view, the centre covering the same share of the way home as the width, so zooming back out to F's zoom lands exactly on F's view. From twice the canvas width down to 80 user units wide. Plain scrolling is left to the page.
- **Two fingers** pan and pinch-zoom together; one finger scrolls the page.

Nothing clamps the view: the ground reaches past every side. Any navigation animates to its target view, discarding the move.

Focusing animates the viewBox to F's `data-svugy-view`, fitted to the uncovered area and centred in it. A layer change crossfades the two layers while the viewBox moves to F's view in the new layer, starting from the view that puts the clicked element's new center where its old center was on screen. Under reduced motion both jump, the flow animation stops and travelers hide.

### Steps

W's steps are numbered from 1 in written order. When W has a description, slot 0 precedes them: the overview.

- **First opening:** the first time the panel opens in W, S is slot 0 if W has an overview, else step 1. Afterwards S keeps the last value, across hiding and showing the panel, until W changes.
- **Step change:** ‹ and › in the steps card, ← and → while the panel is visible and the workflow menu is closed, a step in a component card, or a step description link. ‹ is disabled on the first slot, › on the last; nothing wraps.
- **Effects,** whenever S becomes a step while the panel is visible, or the panel opens on one, except by a click in the scene, whose own navigation wins: K becomes max(K, the step's `layer`); if a group of the shown layer listing S lies outside F's level group and does not enclose it, F becomes the root (the containers enclosing F take part in every step of F's interior, so they never count); the view animates to F's view if K or F changed, else stays.
- **Dimming:** while the panel is visible, the steps card is expanded and S is a step, every element and edge group of the shown layer whose `data-svugy-steps` lacks S is dimmed, except selected groups. Slot 0 and a hidden panel dim nothing.

Hovering or focusing a group dims only while no step dims. Hovering or focusing an element then dims every element and edge group of the shown layer except:

- the element, its ancestors and its descendants;
- every edge with an endpoint among them;
- those edges' other endpoints and their ancestors.

Hovering an edge keeps only the edge, its endpoints and their ancestors lit. While a step dims, hovering or focusing an element, edge or stub only highlights it. Stub markers and badges are never dimmed. Travelers on dimmed connections, by a step or a hover alike, hide and their flow stops.

## Description panel

The panel holds two cards stacked vertically: the **steps card** on top, the **component card** below. It is a side panel when the SVG's on-screen box is at least 640 px wide, a sheet below that; when a resize crosses 640 px, a visible panel becomes `open` or `expanded` and a hidden one stays hidden. It floats over the diagram.

**Side panel:** pinned to the right edge, the box's full height, min(360 px, 45% of the box width) wide until the reader resizes it. P is `hidden` or `open`.

- Hidden: a 14 × 48 px `‹` tab at the middle of the right edge, the size of the open panel's `›` tab; activating it opens the panel.
- Open: along its left edge, a 14 px resizing edge holding a 14 × 48 px `›` tab at its middle; then the steps card, a divider with a grab bar, then the component card, each card expanded or collapsed to its line; a collapsed component card sits at the panel's bottom, its divider just above it.
- Dragging the resizing edge from any point, the tab included, or ← and → while it has focus, sets the panel's width, kept between 240 px and 70% of the box width; the width lasts across hiding and showing until the page reloads. A press on the tab that never drags, a double click anywhere on the edge, or Enter or Space on the tab or the edge, hides the panel.
- Double clicking the divider, or Enter or Space while it has focus, collapses the component card when it is expanded, else expands it. While both cards are expanded, dragging the divider, or ↑ and ↓ while it has focus, sets the steps card's share of the two cards' height, kept between 15% and 85%; until the reader moves it, the steps card takes at most half the panel's height. Each card scrolls on its own.
- Arrow keys a resizer handles never reach the diagram.

**Sheet:** pinned to the bottom edge, the box's full width. P is `hidden`, `peek` or `expanded`.

- Hidden: only a handle bar `═══`.
- Peek: the handle, then the steps card down to its line.
- Expanded: up to 70% of the box's height; the body of at most one expanded card shows, the front one, the card expanded last; the other shows only its line and keeps its state. Activating the line of a card whose body is hidden expands it and brings it to the front.
- A tap on the handle goes hidden → peek → expanded, and expanded → peek. Dragging the handle moves the sheet; on release it snaps to the state whose resting height is nearest: hidden the handle's, peek the handle's plus one line, expanded 70% of the box's height.

**Card lines:** each card's line toggles it: activating the line while the card's body shows collapses the card, else expands it. The steps card's line shows the slot, `OVERVIEW` or `i/N`, and, while the card's body is hidden, the slot's title; then ‹ and ›.

**Steps card:** the slot's title, then its description rendered as Markdown; the overview shows W's title, subtitle and description. Collapsing it stops its step's dimming. ‹, ›, ← and → keep it collapsed or expanded; a step in a component card or a step description link expands it.

**Component card:** for I, its kind (`COMPONENT` or `EDGE`) and a ✕, then:

- element: label, caption, code (as text), each metadata entry as `NAME: value`, description;
- written edge: label, else its endpoints' labels joined by " → ", then its description;
- lifted connection: its title, then for each source edge, a heading with its label, else its endpoints' labels joined by " → ", then its description;
- then **Steps:** one button per step in I's `data-svugy-steps`, labelled `i · TITLE`; activating one makes S that step and expands the steps card.

The component card is expanded exactly while I is set. Collapsing it, or its ✕, clears I as Escape does: the highlight goes and the view returns from the selection. Without I, the card shows its `COMPONENTS` line; on a sheet, with I behind the steps card, its line shows I's kind and name. Expanding it without I selects the first component written in S, its first item's element or edge source; on the overview, step 1's. That selection puts `data-svugy-selected` on the group drawing the component in the shown layer, its closed ancestor if it is hidden, and moves no view.

A card's line starts with a `<button>` holding its label and title, its `aria-expanded` true while the card is expanded; the line's ‹, › and ✕ buttons follow it, never inside it. The sheet's handle carries `aria-expanded`, true while the sheet is expanded.

**Opening rules:**

- Selecting I while P is hidden opens the panel: `open` on a side panel, `expanded` on a sheet.
- Selecting I expands the component card and brings it to the front, out of a peeking sheet too.
- A step change from a component card or a description link expands the steps card and brings it to the front.
- Hiding the panel clears I.

Descriptions render a Markdown subset:

- paragraphs, one line per source line;
- `-` and `*` bullet lists and `1.` numbered lists; an indented item nests under the item above, one level deep; an indented line continues the open item;
- `#`, `##` and `###` headings, drawn as `h4`, `h5` and `h6`;
- fenced code between two ```` ``` ```` lines, preformatted, without inline formatting;
- inline `**bold**`, `*em*`, `` `code` ``, and `[text](url)` links whose URL is http(s) or has no scheme and no space or control character;
- `[text](#TARGET)` description links, drawn as a `<button type="button">` holding underlined `text`. TARGET names an element as its `data-svugy-element` path, a written edge as `FROM->TO[ORDINAL]`, FROM and TO its `data-svugy-from` and `data-svugy-to`, ORDINAL the one ending its `data-svugy-edge`, or a step as `WORKFLOW/STEP`. `[ORDINAL]` is required; the renderer adds it to an edge target stored without one when exactly one edge of the group's workflow joins the pair, and leaves it as written otherwise. The runtime resolves element and edge targets in layer D of W. On hover or keyboard focus an element or edge link's text takes T's colour (an element's top-face fill, an edge's stroke) when it has at least 3:1 contrast with `--svugy-canvas`, else `--svugy-container-blue-ink`;
- any other link, and a TARGET naming nothing, renders as its text.

Author text only ever becomes text nodes.
