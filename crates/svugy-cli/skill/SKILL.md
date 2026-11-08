---
name: svugy
description: Draw architecture diagrams as interactive isometric SVGs by writing Markdown and rendering it with the `svugy` CLI. Use for any request to diagram, draw or visualize a system, codebase, service map, data flow or pipeline, and to edit a svugy `.md` diagram. Not for charts, sequence or Gantt diagrams.
---

# svugy

## Workflow

1. Write `NAME.md`, starting from the example below; to edit a diagram, change it in place.
2. `svugy validate NAME.md --format json`: every problem in one pass, with line, column and a `did you mean` for typos. Fix all of them; repeat until exit code 0 (warnings don't block).
3. Once per diagram, run `svugy view NAME.md` in the background: the user's browser shows it and reloads on every later save. Don't rerun it after edits.
4. For the final file: `svugy render NAME.md -o NAME.svg`.

No `svugy` on `PATH`: ask the user to run `curl -fsSL https://svugy.dev/install.sh | sh`, on Windows `irm https://svugy.dev/install.ps1 | iex`, or with a Rust toolchain `cargo install svugy-cli`.

## Example

Uses every property in the table below, and descriptions.

````markdown
---
title: Checkout
subtitle: from cart to charged card
styles:
  store: (shape=cylinder, color=#d5dcea)
---

```description
Checkout turns a cart into a charged card.
```

## Components
- web: Web app (caption=React, color=#9945ff, link=https://example.com)
- shop: Shop backend (layout=row, meta.owner=checkout team)

  ```description
  Owns the cart and the *orders* table:
  - `api` takes the order
  - `orders` keeps it
  ```

  - api (caption=REST, stroke=#14f195)
  - orders (store)
- payments (layout=grid, columns=2)
  - gateway: Card gateway (shape=slab, height=40)
  - fraud (shape=disc, line=dashed)
  - ledger (store)
- vault (link=ledger)

## Edges
- web -> api: places order (traveler=envelope)
  ```description
  One request per click on **Pay**.
  ```
- api -> orders: saves it (traveler=document, line=dotted)
- api -> gateway: charges (traveler=parcel, color=#f0a020)
- gateway -> fraud: checks (traveler=key, line=solid)
- gateway -> ledger
- gateway -> vault (line=badge)
````

- Component: `- ID[: LABEL] [(PROPERTIES)]`; indent children two spaces. A component with children is a container.
- Edge: `- REF -> REF[: LABEL] [(PROPERTIES)]`.
- Description: a ```` ```description ```` fenced block of free Markdown, bullets included, indented under a component or an edge; before `## Components` it describes the diagram.
- Description link to the diagram: `[the API](#api)` targets a component REF, `[the job](#api->queue)` the edge, `#api->queue[1]` its second parallel edge.
- Sections, in order: `## Components` (required), `## Edges`, `## Legend`.

## Properties

| Key | On | Values |
|---|---|---|
| `caption` | component | text under the label |
| `color` | both | `#rgb[a]`, `#rrggbb[aa]`, CSS name, `transparent`; fill of a component, stroke of an edge |
| `stroke` | component | outline colour |
| `line` | both | `solid`, `dashed`, `dotted`; edges also `badge`: a mini of the target on the source, both ends leaves. Edges default to `dashed` |
| `shape` | leaf | `box` (default), `slab`, `cylinder`, `disc` |
| `height` | leaf | number > 0 |
| `layout` | container | `layered` (default), `row`, `column`, `grid` |
| `columns` | container | integer > 0; requires `layout=grid` |
| `traveler` | edge | `dot` (default), `envelope`, `parcel`, `document`, `key` |
| `link` | component | a URL (holds `:`, `/`, `?` or `#`) or a component REF; opened on click |
| `meta.NAME` | component | text shown as `NAME: value` in the detail panel |

## Rules

- Title and subtitle go in the frontmatter; `# Title` is an error. Notes go in `<!-- … -->`.
- A bare word in `( )` is a style name; styles apply first, `key=value` entries override them.
- Quote a label or value only if it holds `,`, `(`, `)` or `"`, or if a label ends in `(…)`: `- cache: "Cache (Redis)"`.
- REF is the shortest unique dotted suffix of a path: `api`, or `shop.api` if two components are named `api`.
- A chain `a -> b -> c` takes no label or properties; write one hop per line to add them.
- No edge from a component to itself, its ancestor or its descendant.
- An item's line is read verbatim: no emphasis or backticks there. Its description is Markdown.
- Under an item, the only non-item block is one ```` ```description ```` block, before the component's children; unfenced text there is an error. Legend items and chains take none.
- Indent a comment like the item after it; a shallower one breaks nesting.
- Add `position=X,Y` only when the user asks for a fixed layout.

Legends, every diagnostic and importing d2 or JSON: [reference.md](reference.md).
