---
name: svugy
description: Draw architecture diagrams as interactive, explorable isometric SVGs by writing a short svugy Markdown file and rendering it with the `svugy` CLI. Use this whenever the user asks for an architecture, system, component, container, C4-style, service-map, data-flow or dependency diagram, or wants a codebase or infrastructure "drawn" or "visualized", even if they never say svugy; use it instead of hand-writing SVG, Mermaid or d2. Also use it to edit, fix or re-render an existing svugy `.md` diagram (a Markdown file with a `## Components` section).
---

# svugy

A diagram is a plain `.md` file: frontmatter, a nested list of components, a list of edges. svugy lays it out, so write structure, not coordinates.

## Workflow

1. Write `NAME.md` (start from the example below). To edit an existing diagram, read it and change it in place.
2. Run `svugy validate NAME.md --format json`. Exit code 0 means valid; warnings do not block.
3. Fix every reported problem. All of them are reported in one pass, each with its line and column and, for typos, a `did you mean`. Go back to step 2 until it is clean.
4. Run `svugy render NAME.md -o NAME.svg --open`.
5. Tell the user, in one line: "Click a box to open it, click its slab to focus on it, press Escape to go back."

If `svugy` is not on `PATH`, ask the user to install it (`cargo install --path crates/svugy-cli` from the svugy repository).

## Example

```markdown
---
title: Checkout
subtitle: From cart to charged card.
styles:
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- web: "Web app" (caption="React", color=#9945ff)
- shop: "Shop backend"
  - api (caption="REST")
  - orders (store)
- payments
  - gateway (caption="card processor")
  - ledger (store)
- vault (shape=cylinder)

## Edges
- web -> api: "places order" (traveler=envelope)
- api -> orders
- api -> gateway: "charges" (traveler=parcel, line=dashed)
- gateway -> ledger
- gateway -> vault (line=badge)
```

Components: `- ID[: "DISPLAY NAME"] [(PROPERTIES)]`, nested two spaces per level; a component with children is a container. Edges: `- REF -> REF[: "LABEL"] [(PROPERTIES)]`. Sections come in this order: `## Components` (required), `## Edges`, `## Legend`.

## Component properties

| Key | Accepted values |
|---|---|
| `caption` | string, drawn under the label |
| `color`, `stroke` | colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, CSS name, `transparent` |
| `line` | `solid`, `dashed`, `dotted` |
| `shape` | `box` (default), `slab`, `cylinder`, `disc`; leaves only |
| `height` | number > 0; leaves only |
| `layout` | `layered` (default), `grid`, `row`, `column`; containers only |
| `columns` | integer > 0, with `layout=grid` only |
| `link` | component reference or URL |
| `meta.NAME` | string, shown in the detail panel |
| `position` | `X,Y`; omit it (see rules) |

## Edge properties

| Key | Accepted values |
|---|---|
| `line` | `solid`, `dashed`, `dotted`, `badge` (a small copy of the target on the source's top; both ends must be leaves) |
| `color` | colour |
| `traveler` | `dot` (default), `envelope`, `parcel`, `document`, `key` |

## Rules agents get wrong

- **Properties are named:** `(caption="REST", color=#3478f6)`, never bare values. Quote a value that has spaces or commas. A bare style name applies that style first; `key=value` entries then override it, whatever their order.
- **Edge references are the shortest unique dotted suffix** of a path: `api` when only one component is called `api`, `shop.api` when two are. Never write the full path out of habit, and never join a container to its own child.
- **A chain `a -> b -> c` takes no label and no properties.** Write each hop on its own line to give it some.
- **Indent a comment like the item after it.** A shallower `<!-- -->` ends the list and breaks nesting.
- **Omit `position`** unless the user asked for a fixed layout; automatic layout is better than guessed coordinates.

Each item is one line, read verbatim: no Markdown emphasis or code inside it.

For legends, styles in depth, every diagnostic and importing d2 or JSON, read [reference.md](reference.md). A first diagram never needs it.
