# Interactive SVG

`svugy render` writes the interactive SVG by default. It holds one layer per container depth, and a small embedded script lets the reader drill in: no viewer, no server, no external URL.

## Navigate

| Do | Get |
|---|---|
| click a closed container | the next layer: every container at that depth opens |
| click an open container | zoom into it |
| click a component | zoom onto it, neighbours still in sight |
| click a stub on a container's edge | jump to the other end of that edge |
| click empty canvas, or Esc | step back out |
| drag | pan |
| pinch, or Ctrl/⌘ + scroll | zoom |
| hover | the element's details; everything unrelated dims |

- Tab moves between elements, Enter or Space clicks, Esc steps out. Reduced-motion settings turn animations into jumps.
- The colours follow the reader's light or dark preference.
- Link to a level by appending `#svugy=PATH` to the SVG's URL, optionally `&layer=K`: `webapp.svg#svugy=backend`.

## Descriptions

The top-right panel shows the [description](run/markdown.md#descriptions) of the focused level, or of the last clicked component or edge.

- It renders paragraphs, lists, headings, code, bold, italics and links.
- A link like `[the API](#api)` zooms to its target, opening layers if needed, and makes it pulse.

## Embedding

The script runs only where the host lets the SVG's own script run:

| Runs | Doesn't run |
|---|---|
| the `.svg` opened directly | `<img>` |
| `<object>`, `<iframe>` | Markdown `![](x.svg)` |
| the `<svg>` pasted inline into HTML | GitHub and GitLab READMEs |

- Without script, the whole diagram shows, still animated: animations need no script. For those hosts, `--profile static` draws text as paths, so it looks the same without fonts.
- Several diagrams on one page don't interfere.
- Tools can read the `data-svugy-*` attributes on every group; [CONTRACT.md](https://github.com/fedemagnani/svugy/blob/main/crates/svugy/runtime/CONTRACT.md) specifies them.

Next: [How it works](how-it-works.md)
