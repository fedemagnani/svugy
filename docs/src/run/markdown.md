# Markdown

A diagram is one `.md` file: frontmatter, then a nested list of components, then a list of edges.

<div class="svugy-live">

<textarea data-svugy-language="markdown">{{#include ../../../examples/webapp/webapp.md}}</textarea>

<object class="svugy" type="image/svg+xml" data="../figures/webapp.svg"></object>

</div>

<script src="../playground/svugy-wasm.js"></script>
<script src="../playground/playground.js"></script>

Every rule below is an error unless it says warning. `svugy validate FILE` reports all of them in one run.

## File

| Part, in this order | Required |
|---|---|
| frontmatter between `---` lines | no |
| the diagram's [description](#descriptions) | no |
| `## Components`, at least one item | yes |
| `## Edges` | no |
| `## Legend` or `## Legend: TITLE` | no |

- Sections hold bullet lists only. `# Title` is an error: the title goes in the frontmatter.
- An item's line is read verbatim: `*`, `_` and backticks are plain characters.
- `<!-- comments -->` and blank lines are ignored anywhere. Indent a comment between nested items like the item after it; a shallower one would end the list.

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
| `title` | text | drawn top-left |
| `subtitle` | text | drawn under the title |
| `styles` | name → properties list | each name becomes usable in any properties list |

## Components

```md
- ID[: DISPLAY NAME] [(PROPERTIES)]
```

- **ID:** ASCII letters, digits, `_` and `-`, starting with a letter or `_`; unique among its siblings.
- **Display name:** the text after the first `:`; without one, the ID is drawn. Quote it if it ends in a parenthesised group or holds `"`: `- cache: "Cache (Redis)"`.
- **Children:** items indented two spaces under it. A component with children is a container: a slab holding them, which starts closed in the interactive SVG.

## Edges

```md
- REF -> REF[: LABEL] [(PROPERTIES)]
- REF -> REF -> REF
```

- **REF** is a component's dotted path, or any dotted suffix of it that matches exactly one component:

| Components | REF | Resolves to |
|---|---|---|
| one `writer` | `writer` | `writer` |
| `render.writer` and `store.writer` | `writer` | error listing both |
| `render.writer` and `store.writer` | `render.writer` | `render.writer` |
| top-level `api` and `orders.api` | `api` | the top-level `api`: a whole path wins |

- A chain `a -> b -> c` adds one edge per hop and takes no label or properties.
- Two items joining the same pair are two edges.
- No edge from a component to itself, its ancestor or its descendant.

## Descriptions

````md
```description
Every request starts in the browser and ends in the database.
```

## Components
- backend: Backend

  ```description
  Every browser request lands here.
  - the API answers synchronously
  - anything slower goes to the queue
  ```

  - api: API

## Edges
- api -> queue: schedules a receipt
  ```description
  One job per order, retried 3 times.
  ```
````

- A description is a fenced block with the info string `description`, holding any Markdown.
- Before `## Components`, it describes the diagram. Indented under a component or an edge, it describes that item; under a component, it goes before the children.
- One description per item. Text under an item outside a description block is an error.
- The interactive SVG shows it in a panel when the item is focused or clicked.

**Links** in a description point into the diagram, and clicking one zooms to its target:

| Link | Targets |
|---|---|
| `[the API](#api)` | a component, by REF |
| `[the job](#api->queue)` | the edge between two components, when exactly one exists |
| `[the retry](#api->queue[1])` | the n-th edge between them, from 0 |

A link naming nothing, or several things, is a warning and shows as plain text.

## Properties

```md
- api: API (service, caption=HTTP endpoints, meta.owner=payments team)
```

- Entries are comma-separated: `key=value`, or a frontmatter style name.
- Styles apply first, left to right, then every `key=value` over them.
- Quote a value holding `,`, `(`, `)` or `"`. Escapes are `\"` and `\\`.
- An unknown key or style is an error suggesting the nearest one.

### On a component

| Key | Value | Effect |
|---|---|---|
| `caption` | text | second line under the label |
| `color` | colour | fill |
| `stroke` | colour | outline |
| `line` | `solid` (default), `dashed`, `dotted` | outline dash |
| `shape` | `box` (default), `slab`, `cylinder`, `disc` | a leaf's solid |
| `height` | number > 0 | a leaf's height |
| `position` | `X,Y` | top-left corner in the parent; unpinned components are placed automatically |
| `layout` | `layered` (default), `grid`, `row`, `column` | how a container places its children |
| `columns` | integer > 0 | column count, required by `layout=grid` |
| `link` | component REF or URL | a click goes to that component, or opens the URL |
| `meta.NAME` | text | a `NAME: value` line in the hover panel |

- A colour is `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name or `transparent`.
- A `link` holding `:`, `/`, `?` or `#` is a URL; anything else is a REF. Write a relative page as `./guide.html`.
- Warnings: `shape` or `height` on a container (always a slab), `layout` on a leaf.

### On an edge

| Key | Value | Effect |
|---|---|---|
| `line` | `dashed` (default), `dotted`, `solid`, `badge` | how the edge is drawn |
| `color` | colour | stroke |
| `traveler` | `dot` (default), `envelope`, `parcel`, `document`, `key` | the icon moving along the edge |

`line=badge` draws no arc: a small copy of the target sits on the source's top face. Use it for a dependency many components share. Both ends must be leaves.

## Legend

```md
## Legend: How to read it
- arrows point where a request goes
- (line=dashed): leaves your servers
- (line=dotted): embedded at build time
- (line=badge, target=cache): looks up the cache
```

- Items are drawn in order under the title.
- `(PROPERTIES): TEXT` is a key row, drawn next to a sample: `line=dashed`, `line=dotted`, or `line=badge` with `target=REF`. Any other item is free text.
- Warning: a key row describing something the diagram never draws.

Next: [d2](d2.md)
