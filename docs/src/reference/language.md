# The svugy language

*Draft for review. Nothing parses it yet.*

The svugy language is the input format written for LLM agents first and people second. A diagram is a plain Markdown file (`.md`): YAML frontmatter with the title and reusable property sets, a nested list of components, a list of edges, and a legend. Every value is a named property, so a file explains itself, and a typo is an error that names what was meant. Any Markdown preview shows a diagram file as a readable outline.

```
---
title: Order desk
subtitle: How an order becomes a trade.
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- client (caption="web app", color=#9945ff)
- desk: "Order desk" (caption="matching", position=260,0)
  - api (service, caption="REST")
  - book (store)

## Edges
- client -> api: "places order" (traveler=envelope)
- api -> book
```

## Document

The file is CommonMark, read with comrak. It has these parts, in this order:

1. **Frontmatter:** optional, between `---` lines at the very top.
2. **`## Components`:** required, holding at least one component.
3. **`## Edges`:** optional.
4. **`## Legend`** or **`## Legend: TITLE`:** optional.

Each section holds bullet lists and nothing else. A list item is one line, read verbatim: Markdown inline syntax such as `*`, `_` or backticks has no meaning inside it. HTML comments (`<!-- … -->`) are ignored, and so are blank lines.

A comment between list items must be indented like the item after it. In CommonMark a comment at a shallower indent ends the list, so the indented item after it would silently become a top-level item. svugy reports that case as an error: an item indented but not nested.

## Frontmatter

| Key | Value | Meaning |
|---|---|---|
| `title` | text | Drawn top-left of the top-level view. |
| `subtitle` | text | Drawn under the title. |
| `styles` | map of name to properties | Named property sets, used in any properties list by their name. |

Any other key is an error. A style is written exactly like a properties list, parentheses included: `service: (color=#3478f6, shape=box)`. Written that way a colour needs no YAML quotes, since `#` only starts a YAML comment after a space.

## Components

Each component is one list item:

```
- ID[: "DISPLAY NAME"] [(PROPERTIES)]
```

- **ID** is letters, digits, `_` and `-`, starting with a letter or `_`. It must be unique among its siblings. It is also what edges use to refer to the component.
- **DISPLAY NAME** is the label drawn on the component. Without one, the ID is drawn.
- **Nesting** follows CommonMark lists: an item indented under another is its child. Write two spaces per level.

A component with children is a container. A container is drawn as a slab holding its children, or as a closed box until it is opened in the interactive SVG.

## Edges

Each edge is one list item:

```
- REF -> REF[: "LABEL"] [(PROPERTIES)]
- REF -> REF -> REF ...
```

- **REF** names a component by any dotted suffix of its path that matches exactly one component. In most diagrams that is just its ID: `writer`. When two components share an ID, add parents until the reference is unique: `render.writer` or `pipeline.render.writer`. An unknown reference is an error with a suggestion, and an ambiguous one is an error listing the matches.
- **A chain** `a -> b -> c` writes one edge per hop. A chain takes no label and no properties; write its hops on separate lines to give them some.
- **Repeated edges** between the same pair are separate edges, numbered in the order written.
- An edge between a component and its own ancestor or descendant is an error.

## Properties

A properties list is a parenthesised, comma-separated list of entries. An entry is either `key=value` or the name of a style from the frontmatter.

- **Styles apply first,** left to right, then the `key=value` entries override them. `(service, color=#ff0000)` is a red service, whatever order the entries are in.
- **A key appears at most once** among the `key=value` entries. An unknown key, or an unknown style name, is an error that suggests the nearest known one.
- **A value** is a quoted string (`"…"`, with `\"` and `\\` as its only escapes), or a bare token: anything without spaces, commas, parentheses or quotes. The key decides how the value is read, which is why `position=760,0` reads as one value even though commas also separate entries. A value of the wrong kind is an error that lists what the key accepts.
- **A style may hold component keys, edge keys or both.** Where it is used, each of its keys must suit what it is used on.

### Component properties

| Key | Value | Meaning |
|---|---|---|
| `caption` | string | Second line drawn under the label. |
| `color` | colour | Fill. |
| `stroke` | colour | Outline. |
| `line` | `solid`, `dashed`, `dotted` | Outline dash. |
| `shape` | `box`, `slab`, `cylinder`, `disc` | Leaf primitive. Default `box`. |
| `height` | number > 0 | Leaf height, in plan units. |
| `position` | `X,Y` or `X;Y` | Pinned top-left corner, relative to the parent's interior. Unpinned components are placed automatically. |
| `layout` | `layered`, `grid`, `row`, `column` | How a container places its children. Default `layered`. |
| `columns` | integer > 0 | Column count, with `layout=grid` only. |
| `link` | component reference or URL | A click goes to that component, or opens the URL. |
| `meta.NAME` | string | Shown in the detail panel as `NAME: value`. |

A colour is `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name or `transparent`. A container ignores `shape` and `height` and warns about them, since it is always a slab.

### Edge properties

| Key | Value | Meaning |
|---|---|---|
| `line` | `solid`, `dashed`, `dotted`, `badge` | How the edge is drawn. `badge` draws it as a small copy of the target on the source's top face, with no arc. |
| `color` | colour | Stroke. |
| `traveler` | `dot`, `envelope`, `parcel`, `document`, `key` | What moves along the edge in the interactive SVG. Default `dot`. |

A `badge` edge needs a leaf at both ends; anything else is an error.

## Legend

The items of `## Legend` are drawn in order under its title. An item is either free text or a key row:

```
## Legend: Read it as a story
- ① your text becomes a model
- (line=dashed): data flows this way
- (line=dotted): embedded at build time
- (line=badge, target=inter): reads the bundled font
```

A key row describes what the diagram draws, and is drawn next to a sample of it. `(line=dashed)` and `(line=dotted)` describe edges drawn that way; `(line=badge, target=REF)` describes the badges of one target.

## Diagnostics

The parser reports every problem it finds in one pass, each with its line and column, so an agent fixes a file in one round. `svugy validate --format json` returns them as JSON. All of the above are errors unless marked as warnings.

## Lowering to the model

| Language | Model |
|---|---|
| frontmatter `title`, `subtitle` | `title`, `subtitle` |
| component path | `ElementPath`, segments from the nesting |
| display name, else ID | `Element::label` |
| `caption`, `height`, `shape`, `position`, `link`, `meta.*` | the element field of the same meaning |
| `layout`, `columns` | `PlacementHint` |
| `color`, `stroke` | style `fill`, `stroke` |
| `line=dashed`, `line=dotted` | style `stroke-dash` 5, 2 |
| edge, label | `EdgeId` with its ordinal, `Edge::label` |
| edge `color`, `traveler` | style `stroke`, `Edge::traveler` |
| edge `line=badge` | a badge edge (see below) |
| legend items | `Legend` entries, dash and badge descriptions |

Styles are expanded before lowering, so the model never sees them.

One change to the model comes with the language: a badge becomes a property of the edge, not of its target. Today `vars.svugy.badge` marks a target, and every edge into it from a leaf becomes a badge. The d2 importer keeps its syntax by marking every such edge.
