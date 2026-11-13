# svugy syntax

Rules SKILL.md's example and rules leave out.

## File

- Parts, in order: frontmatter (optional), `## Components` (required, ≥ 1 item), `## Workflows` (required, ≥ 1 workflow).
- `## Components` holds a `- ` bullet list only; `*` and `+` bullets are an error. Blank lines and `<!-- … -->` are ignored anywhere, including at the end of an item.
- Frontmatter keys: `view`, `styles`; any other key is an error. `view` is `isometric` (default) or `top` (flat 2D plan).
- A style is a properties list, parentheses included: `service: (color=#3478f6, shape=box)`. It may hold component keys, edge keys or both; each key must suit where the style is used.

## Components

- ID: ASCII letters, digits, `_`, single inner `-`; starts with a letter or `_`. Unique among siblings only.
- No label: the ID is drawn.
- Path: IDs from the top, dot-joined: `shop.api`.
- A component drawn as a container in a workflow ignores `shape` and `height` there, with a warning.
- `link=VALUE`: a value holding `:`, `/`, `?` or `#` is a URL; anything else is a REF. Write a relative page as `./guide.html`; bare `guide.html` is read as a REF.
- `code=FILE` or `code=FILE#SYMBOL`: FILE relative to the root of the git checkout holding the diagram, no leading `/`, no `..`; SYMBOL is `NAME`, `Type::NAME` or `Type.NAME`. `svugy validate` errors when FILE is missing or does not spell each SYMBOL segment as a whole identifier. Use it instead of `meta.code`, which nothing checks.
- `meta.NAME=VALUE`: any number of distinct names.
- `position`, `layout` and `columns` are set per workflow, in its overrides; here, directly or through a style, they are an error.

## Workflows

````markdown
## Workflows
### checkout: A customer pays (subtitle=from cart to charged card)
What happens between [the web app](#web) sending the cart and [the ledger](#ledger) recording the charge.

- shop (layout=row)
- web (position=0,40)

#### order: The shop takes the order
[The API](#api) saves the cart before [charging it](#checkout/charge).
- web -> api: places order (traveler=envelope)
  ```description
  One request per click on **Pay**.
  ```
- api -> orders: saves it

#### charge: The gateway charges the card
- api -> gateway: charges (traveler=parcel)
- gateway -> fraud -> ledger
- vault

#### confirm: The web app learns the result
The answer comes back on [the charge request](#api->gateway).
- api -> gateway[0]
- gateway -> api: approved
````

- `### ID: TITLE [(subtitle=TEXT)]` opens a workflow; `subtitle` is its only property. ID follows the component ID rules and is unique among workflows. The first workflow is the one the SVG opens on.
- Under the heading, in order: prose (optional), overrides (optional), then ≥ 1 step. Text after the overrides is an error.
- A `link=REF` naming a component the workflow doesn't show is a warning there, and the component links nowhere in that workflow.
- A workflow shows the components its steps list, as an edge end or bare, plus their ancestors. A container none of whose children it shows is drawn there as a leaf.
- Edges belong to their workflow: two workflows joining the same pair declare two edges, each workflow numbering its own from 0.

### Overrides

- `- REF (PROPERTIES)`: one line per component, no label, no description, no children; nesting comes from `## Components`.
- Any component key; `position`, `layout` and `columns` are allowed only here.
- They apply to this workflow only: the component's own properties first, then the override's styles, then its `key=value` entries.
- Errors: a component the workflow doesn't show, a component listed twice.

### Steps

- `#### ID: TITLE` opens a step; no properties. ID follows the component ID rules and is unique within its workflow. Steps are numbered from 1 in written order.
- Under the heading: prose (optional), then ≥ 1 item. Text after the items is an error.
- Items:
  - `- REF -> REF[: LABEL] [(PROPERTIES)]` declares a new edge, optionally followed by a description block. A chain `a -> b -> c` declares one edge per hop.
  - `- REF -> REF[n]` lists again the n-th edge between the pair, from 0 in the workflow's written order, declared in any step of the same workflow. n is digits only. No label, properties or description.
  - `- REF` shows a component without an edge. No label, properties or description.
- Spaces around `->` are optional: `- web->api` is `- web -> api`.
- Errors: a step without items, an item listed twice in one step, `[n]` naming no edge of the workflow.

### Prose

- Unfenced Markdown: the lines between a `###` or `####` heading and the first item, override or next heading. The workflow's prose is its overview; a step's prose is its description. A ```` ```description ```` block in prose is an error: drop the fence.
- Stored verbatim, comments removed. Renders with the runtime's Markdown subset (`crates/svugy/runtime/CONTRACT.md`), except `- ` bullet lists and headings: outside a code fence, a `- ` line starts the items and a `#` line is a heading.

## Edges

- REF: any dotted suffix of a path matching exactly one component; a whole path always wins (`api` is the top-level `api` even if `orders.api` exists).
- Repeated edges between one pair stay separate, in written order across the workflow's steps.
- Only `->` exists; write a reply as a second edge.
- Edges may cross containers: `web -> shop.api`.
- Error: an edge from a component to itself, its ancestor or its descendant.
- `line=badge` replaces the arc with a mini of the target on the source's top face. Use it for a dependency many components share (a font, a config store). Both ends must be leaves.

## Descriptions

`````markdown
## Components
- web
- shop: Shop backend

  ````description
  Owns the cart and the *orders* table.
  - carts expire after a day
  ```rs
  struct Shop { carts: Carts, orders: Orders }
  ```
  ````

  - api
`````

- A ```` ```description ```` block indented under a component or an edge declaration describes that item. Workflows and steps take unfenced prose instead.
- Backtick or tilde fence, three or more; when the content holds a line of only ```, open with a longer fence: ````` ````description `````.
- Under an item it goes on the next line or after a blank line, and before a component's children.
- Stored verbatim: any Markdown, bullet lists included.
- Code inside a description or prose: a ```` ```TAG ```` fence. `rs`, `rust`, `ts`, `typescript`, `js`, `javascript`, `py`, `python`, `go`, `json`, `toml`, `yaml`, `yml`, `sql`, `sh`, `bash` and `shell` are highlighted; any other tag renders as plain monospace. Every block gets a copy button. A code block inside a description block needs the longer outer fence, as above.
- Errors: unfenced text in `## Components`, a code block with another info string under an item, a second description block, a missing closing fence, a description block in workflow or step prose, a description under an override, a bare step item, an `[n]` item or a chain.

### Links to components, edges and steps

- `[TEXT](#REF)` targets a component; `[TEXT](#REF->REF)` the edge between two, when only one exists; `[TEXT](#REF->REF[n])` the n-th, from 0 in written order; `[TEXT](#WORKFLOW/STEP)` a step of any workflow. No spaces.
- Edge targets resolve:
  - in workflow prose, step prose and edge descriptions: in that workflow;
  - in component descriptions: in each workflow showing the component, separately.
- Targets may name items declared later. The model stores the whole form: `#shop.api`, `#web->shop.api[0]`, `#checkout/order`. In a component description, an edge target written without `[n]` stays without it: `#web->shop.api`.
- Warnings, and the link shows as plain text there:
  - a target naming nothing or several items;
  - in workflow prose, step prose or an edge description, a component the workflow doesn't show;
  - in a component description, an edge target that names nothing or several edges in a workflow showing the component: one warning per such workflow.

## Values

- Bare value: runs to the next `,` or `)`, trimmed: `caption=HTTP endpoints`.
- Quoted value: `"…"`, escapes `\"` and `\\`. Required when the value holds `,`, `(`, `)` or `"`.
- Labels and titles follow the same rule; a label ending in `(…)` must be quoted.
- A key appears once per list. Styles apply left to right, then `key=value` entries override them regardless of position.

## Checks across the file

- Error: `## Workflows` missing or empty, a workflow without steps.
- Warning: a component no workflow shows; it is never drawn.

## Migrating from v3

Each v3 construct is an error whose message names its fix:

| v3 | v4 |
|---|---|
| frontmatter `title`, `subtitle` | the first workflow's heading: `### main: TITLE (subtitle=SUBTITLE)` |
| ```` ```description ```` before `## Components` | the first workflow's prose, unfenced |
| `## Edges` | each edge in a step: `### main: TITLE`, then `#### main: TITLE` |
| `## Legend` | removed: say what each entry meant in step prose or edge descriptions |
| `position`, `layout`, `columns` on a component | the same keys in each workflow's overrides |

`svugy import FILE.d2` puts every edge in workflow `main`, titled with the root's label, else `main`, holding one step `main: Every edge`. The step lists every edge, then every leaf no edge touches, so the import draws everything d2 drew. d2 positions and layouts become `main`'s overrides; an edge between a component and its own lineage is dropped with a warning; a d2 legend is an error.
