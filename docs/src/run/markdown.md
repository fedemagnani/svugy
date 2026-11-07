# Markdown

A diagram is one `.md` file. Every value is a named property, so a misspelt key is an error that names the key you meant.

<div class="svugy-live">

<textarea data-svugy-language="markdown">{{#include ../../../examples/webapp/webapp.md}}</textarea>

<object class="svugy" type="image/svg+xml" data="../figures/webapp.svg"></object>

</div>

<script src="../playground/svugy-wasm.js"></script>
<script src="../playground/playground.js"></script>

Every rule below is an error unless it says warning. `svugy validate FILE` reports all of them in one run, each with line and column.

## File

| Part, in this order | Required |
|---|---|
| frontmatter between `---` lines; only a UTF-8 byte-order mark and blank lines may precede it | no |
| the diagram's [description](#descriptions) | no |
| `## Components`, with at least one item | yes |
| `## Edges` | no |
| `## Legend` or `## Legend: TITLE` | no |

- Each section appears once, at heading level 2. A CommonMark closing sequence (`## Edges ##`) is allowed.
- Rejected headings:
  - `# Title`: the title goes in the frontmatter `title`
  - a known section at another level, e.g. `### Edges`
  - an unknown `## Name`, or any heading of level 3 or more
- A section holds bullet lists only. A [description block](#descriptions) goes indented under an item.
- An item's first line is read verbatim: `*`, `_` and backticks are plain characters. An empty item is an error.
- Only `## Components` items nest.
- HTML comments `<!-- … -->` and blank lines are ignored anywhere, including at the end of an item's line. Inside a description block they are part of its text.
- A comment between nested items must be indented like the item after it. A shallower comment ends the list in CommonMark, which would silently make the next item top-level; svugy reports that item as "indented but not nested".

```md
## Components
- billing
  - api
  <!-- the ledger keeps every charge -->
  - ledger
```

## Frontmatter

```yaml
---
title: A web app
subtitle: from the browser to the database and back
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---
```

| Key | Value | Effect |
|---|---|---|
| `title` | text | drawn top-left of the top-level view |
| `subtitle` | text | drawn under the title |
| `styles` | name → properties list | each name becomes usable in any properties list; draws nothing itself |

- Any other key is an error.
- A style's value is a properties list, parentheses included. A colour needs no YAML quotes: YAML starts a comment only at `#` after a space.
- A style holds `key=value` entries only, never another style.

## Components

```md
- ID[: DISPLAY NAME] [(PROPERTIES)]
```

| Part | Rule |
|---|---|
| ID | ASCII letters, digits, `_`, `-`; starts with a letter or `_`; no trailing `-`, no `--`; unique among its siblings |
| DISPLAY NAME | the text after the first `:`, up to a trailing properties list, trimmed; may hold `:` and `->`; without it the ID is drawn |
| children | items indented under the item, two spaces per level |

- Quote a display name that ends in a parenthesised group or holds `"`: `- cache: "Cache (Redis)"`. Escapes are `\"` and `\\` only.

A component with children is a container: a slab holding its children. In the interactive SVG it starts closed and opens on click.

## Edges

```md
- REF -> REF[: LABEL] [(PROPERTIES)]
- REF -> REF -> REF
```

- REF is a component's whole dotted path, or any dotted suffix of it that matches exactly one component:

| Components | REF | Resolves to |
|---|---|---|
| one `writer` | `writer` | `writer` |
| `render.writer` and `store.writer` | `writer` | error listing both matches |
| `render.writer` and `store.writer` | `render.writer` | `render.writer` |
| top-level `api` and `orders.api` | `api` | the top-level `api`: a whole path wins |
| no `wirter` | `wirter` | error suggesting `writer` |

- A chain `a -> b -> c` adds one edge per hop. A chain takes no label and no properties; write its hops on separate lines to give them some.
- The label follows the display name's rules: `- browser -> api: places an order (traveler=envelope)`.
- Two items joining the same pair are two edges.
- Rejected: `a -> a`, and an edge between a component and its own ancestor or descendant.

## Descriptions

````md
```description
Every request starts in the browser and ends in the database.
```

## Components
- browser
- backend: Backend (caption=your servers)

  ```description
  Every browser request lands here.
  - the API answers synchronously
  - anything slower than 100 ms goes to the queue
  ```

  - api: API
  - queue

## Edges
- browser -> api
- api -> queue: schedules a receipt (traveler=parcel)
  ```description
  One job per order, retried 3 times with backoff.
  ```
````

- A description is a fenced code block whose info string is `description`. The fence is backticks or tildes, three or more.
- Before `## Components`, it describes the diagram. Indented to the content column of a component or an edge item, it describes that item, either on the line right under the item's line or after a blank line.
- Its content is stored as written, any Markdown included, bullet lists too, except [link targets](#links-to-components-and-edges), stored in their whole form. The fence's indentation is removed and line breaks are stored as `\n`.
- Under a component, the description goes before the children.
- Rejected:
  - text outside a description block, under an item or before `## Components`: wrap it in a ```` ```description ```` block
  - a code block with another info string, or an indented code block
  - a second description block on one item or on the diagram
  - a description block with no closing fence
  - a description on a legend item, since the legend holds one line of text per item, and on a chain `a -> b -> c`

### Links to components and edges

```md
The API answers [the browser](#browser) and hands [the receipt job](#api->queue) to [the queue](#queue).
```

- A link `[TEXT](#TARGET)` targets a component or an edge of the diagram:

| TARGET | Targets |
|---|---|
| `REF` | the component REF names, resolved like an edge's REF |
| `REF->REF` | the edge between the two, when exactly one exists |
| `REF->REF[n]` | the n-th edge between the two, counting from 0 in written order |

- No spaces inside TARGET. It may name a component or edge declared later.
- The model stores each target in its whole form, an edge always with `[n]`: `#backend.api`, `#backend.api->backend.queue[0]`.
- Warning: a TARGET naming nothing, or more than one component or edge. The link stays as written and shows as plain text.
- Links inside code spans and fenced code are text. Other destinations, `https://…` or `./guide.html`, are kept as written.

## Properties

```md
- api: API (service, caption=HTTP endpoints, meta.owner=payments team)
```

- A properties list is `( … )` with comma-separated entries: `key=value`, or the name of a frontmatter style.
- Styles apply first, left to right, then every `key=value` over them: `(service, color=#ff0000)` and `(color=#ff0000, service)` are both a red service.
- A key appears at most once among the `key=value` entries.
- An unknown key or style is an error suggesting the nearest known one.
- A key valid elsewhere is an error here, e.g. `` `traveler` does not apply to a component ``.
- A bare value runs to the next `,` or `)`, trimmed. Quote a value holding `,`, `(`, `)` or `"`; escapes are `\"` and `\\` only.
- The key decides how its value is read, so `position=760,0` is one value. A value of the wrong kind is an error listing what the key accepts.

### On a component

| Key | Value | Effect |
|---|---|---|
| `caption` | text | second line under the label |
| `color` | colour | fill |
| `stroke` | colour | outline |
| `line` | `solid` (default), `dashed`, `dotted` | outline dash |
| `shape` | `box` (default), `slab`, `cylinder`, `disc` | a leaf's solid |
| `height` | number, 0 < h ≤ 1,000,000 | a leaf's height, in plan units |
| `position` | `X,Y` or `X;Y`, each within ±1,000,000 | top-left corner, relative to the parent's interior; unpinned components are placed automatically |
| `layout` | `layered` (default), `grid`, `row`, `column` | how a container places its children |
| `columns` | integer > 0 | column count; required by `layout=grid`, rejected without it |
| `link` | component REF or URL | a click goes to that component, or opens the URL |
| `meta.NAME` | text | a `NAME: value` line in the detail panel |

- Colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name, or `transparent`.
- `link`: a value containing `:`, `/`, `?` or `#` is a URL; anything else is a REF, resolved like an edge's. Write a relative page with a slash: `link=./guide.html`.
- Warning: `shape` or `height` on a container is ignored; containers are always slabs.
- Warning: `layout` on a leaf is ignored.
- `grid`, `row` and `column` place at most 32,766 columns and 32,766 rows, so a `row` holds at most 32,766 children.

### On an edge

| Key | Value | Effect |
|---|---|---|
| `line` | `dashed` (default), `dotted`, `solid`, `badge` | how the edge is drawn |
| `color` | colour | stroke |
| `traveler` | `dot` (default), `envelope`, `parcel`, `document`, `key` | the icon that moves along the edge in the interactive SVG |

- `line=badge` draws no arc: a small copy of the target sits on the source's top face. Both ends must be leaves.
- `line=solid` draws a continuous line with no flowing dashes.

## Legend

```md
## Legend: How to read it
- arrows point where a request goes
- (line=dashed): leaves your servers
- (line=dotted): embedded at build time
- (line=badge, target=cache): looks up the cache
```

- Items are drawn in order under the title.
- An item starting with `(` then `KEY=` is a key row, `(PROPERTIES): DESCRIPTION`, drawn next to a sample of what it describes. Any other item is free text, even one starting with `(`.
- A key row takes only `line` and `target`, directly or through a style:

| Key row | Describes |
|---|---|
| `(line=dashed): …` | edges with the default dash |
| `(line=dotted): …` | dotted edges |
| `(line=badge, target=REF): …` | the badges of REF |

- Rejected: `target` without `line=badge`, `line=badge` without `target`, and a key row with neither `dashed`, `dotted` nor `badge`.
- Warning: a key row describing something the diagram never draws.

Next: [d2](d2.md)
