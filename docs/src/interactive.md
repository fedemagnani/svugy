# Interactive SVG

`svugy render` writes the interactive SVG by default. It holds every workflow, each with one layer per container depth, and a small embedded script lets the reader pick a workflow, step through it and drill in: no viewer, no server, no external URL.

## Workflows

- The heading, top-left, is the shown workflow's title and subtitle.
- With more than one workflow, the heading ends in `▾`: click it for a menu of every workflow's title and subtitle. Picking one shows that workflow from its root, at layer 0, with the panel hidden. Esc or a click outside closes the menu.
- Each workflow draws only the components its steps list, plus their ancestors, and is laid out on its own: a component can sit elsewhere in another workflow.
- The SVG opens on the first workflow, or on the one `svugy render --workflow ID` names.

## Navigate

| Do | Get |
|---|---|
| click a closed container | the next layer: every container at that depth opens |
| click an open container | zoom into it |
| click a component | zoom onto it, neighbours still in sight; its component card opens |
| click an edge | its component card opens |
| click a stub on a container's edge | jump to the other end of that edge |
| click empty canvas, or Esc | step back out |
| drag | pan |
| pinch, or Ctrl/⌘ + scroll | zoom |
| hover, while no step is shown | everything unrelated dims |
| hover, while a step is shown | the hovered item highlights; nothing else changes |
| the bottom-left buttons | switch between the `isometric` view and the flat `top` view |

- Esc, or a click on empty canvas, first closes the component card and returns to where you were before selecting, then zooms back to the focused level if you panned or zoomed, then steps out one level, then shows the previous layer. At the root of the outermost layer, it returns to the workflow's overview, so nothing is dimmed; a workflow without prose hides the panel instead.
- Tab moves between elements, Enter or Space clicks. Reduced-motion settings turn animations into jumps and hide travelers.
- The colours follow the reader's light or dark preference.
- It opens on the frontmatter's [`view`](run/markdown.md#frontmatter). Both views show the same layout; switching keeps the focused level.

## Description panel

The panel holds two cards: the **steps card** on top, the **component card** below. It starts hidden. It floats over the diagram; opening, closing or resizing it never moves the view, and every zoom fits into the area it leaves uncovered.

Its form depends on the SVG's on-screen width:

| Width | Form | States |
|---|---|---|
| ≥ 640 px | side panel on the right edge, full height, min(360 px, 45% of the width) wide | hidden: a `‹` tab on the right edge opens it; open |
| < 640 px | bottom sheet, full width | hidden: a handle bar; peek: the steps card's line, `i/N TITLE ‹ ›`; expanded: up to 70% of the height |

- **Card lines:** click a card's line to collapse it, and again to expand it. The steps card's line shows `OVERVIEW` or `i/N`, ‹ and ›.
- **Side panel:** drag its left edge, from anywhere including the `›` tab, to resize it between 240 px and 70% of the width; a click on the `›` tab, or a double click anywhere on the edge, hides it. Collapsed, the component card sits at the bottom of the panel. Double-click the divider between the cards to collapse or expand the component card; while both cards are expanded, drag the divider to share the height (until you do, the steps card takes at most half). Each card scrolls on its own. With the edge or the divider focused, the arrow keys resize and Enter acts like a click or double click.
- **Sheet:** a tap on the handle goes hidden → peek → expanded, and expanded → peek. Drag the handle and release: the sheet snaps to the nearest state. Expanded shows one card's body at a time; tapping the other card's line brings it to the front, and the card behind keeps its state.
- Resizing across 640 px turns a visible panel into an open side panel or an expanded sheet; a hidden one stays hidden.

### Steps card

| Slot | Shows |
|---|---|
| overview, when the workflow has prose | the workflow's title, subtitle and prose |
| step i of N | the step's title and prose |

- ‹ and ›, or ← and → while the panel is visible, move one slot. Nothing wraps.
- The first time the panel opens in a workflow, it shows the overview, else step 1. Afterwards it keeps the last slot, across hiding and showing, until the workflow changes.
- While a step shows, everything outside that step is dimmed, and the travelers on dimmed edges stop. The overview, a collapsed steps card and a hidden panel dim nothing. ‹, ›, ← and → keep the card collapsed or expanded; a step button or a step link expands it.
- Showing a step opens the layer that draws every item of the step as written. If the step involves something outside the focused level, the view returns to the root.

### Component card

Clicking a component or an edge selects it, opens the panel if it was hidden, and expands the component card. The card shows:

- `COMPONENT` or `EDGE`, and a ✕;
- a component's label, caption, code, `NAME: value` metadata and description;
- an edge's label, else its endpoints' labels joined by " → ", then its description; a merged connection lists each edge it stands for;
- **Steps:** one button per step the item takes part in, `i · TITLE`. Clicking one shows that step in the steps card.

- Collapsing the card, or its ✕, clears the selection like Esc: the highlight goes and the view returns to where you were before selecting.
- Expanding the card with nothing selected selects the first component of the shown step (step 1 on the overview), without moving the view.
- Clicking a step in the card shows that step with the steps card expanded; clicking a component in a step's text selects it with the component card expanded.
- Hiding the panel clears the selection.

### Descriptions

Descriptions and prose render paragraphs, `-`, `*` and `1.` lists nested one level, headings, code blocks, bold, italics, inline code and http(s) links. [Links into the diagram](run/markdown.md#links) act on click:

| Link to | Does |
|---|---|
| a component or an edge | zooms to it, opening layers if needed, pulses it for 1.2 s and opens its component card |
| a step, `#WORKFLOW/STEP` | switches to that workflow if needed, keeping the panel open, and shows that step |

## Link to a view

Append `#svugy=PATH&layer=K&workflow=ID&step=ID` to the SVG's URL. Every key but `svugy` is optional.

| Key | Opens |
|---|---|
| `svugy=PATH` | the level PATH, encoded with `encodeURIComponent`; empty for the root |
| `layer=K` | layer K, if it holds the level; else the first layer that does |
| `workflow=ID` | workflow ID |
| `step=ID` | the panel on step ID of that workflow: open, or expanded on a sheet |

`webapp.svg#svugy=backend` opens the backend's level. `#svugy=&workflow=checkout&step=charge` opens step `charge` of workflow `checkout`, at the root. A PATH naming no level of the workflow leaves the start as it is.

## Embedding

The script runs only where the host lets the SVG's own script run:

| Runs | Doesn't run |
|---|---|
| the `.svg` opened directly | `<img>` |
| `<object>`, `<iframe>` | Markdown `![](x.svg)` |
| the `<svg>` pasted inline into HTML | GitHub and GitLab READMEs |

- Without script, the shown workflow shows whole, still animated: animations need no script. For those hosts, `svugy render --profile static --workflow ID` draws one workflow with text as paths, so it looks the same without fonts; `--all-workflows -o FILE.svg` writes one such file per workflow.
- Several diagrams on one page don't interfere.
- Tools can read the `data-svugy-*` attributes on every group; [CONTRACT.md](https://github.com/fedemagnani/svugy/blob/main/crates/svugy/runtime/CONTRACT.md) specifies them.

Next: [How it works](how-it-works.md)
