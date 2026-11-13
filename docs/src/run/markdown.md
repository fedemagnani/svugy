# Markdown

A diagram is one `.md` file: frontmatter, a nested list of components, then workflows. A workflow is a story told in steps; each step lists the edges and components it involves.

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
| `## Components`, at least one item | yes |
| `## Workflows`, at least one workflow | yes |

- `# Title` is an error: the title goes in the first workflow's heading.
- An item's line is read verbatim: `*`, `_` and backticks are plain characters.
- `<!-- comments -->` and blank lines are ignored anywhere. Indent a comment between nested items like the item after it; a shallower one would end the list.

## Frontmatter

```yaml
---
view: top
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---
```

| Key | Value | Effect |
|---|---|---|
| `view` | `isometric` or `top` | the angle the diagram is seen from: `top` draws it flat, as a 2D plan; `isometric` by default |
| `styles` | name → properties list | each name becomes usable in any properties list |

Any other key is an error.

## Components

```md
- ID[: DISPLAY NAME] [(PROPERTIES)]
```

- **ID:** ASCII letters, digits, `_` and `-`, starting with a letter or `_`; unique among its siblings.
- **Display name:** the text after the first `:`; without one, the ID is drawn. Quote it if it ends in a parenthesised group or holds `"`: `- cache: "Cache (Redis)"`.
- **Children:** items indented two spaces under it. A component with children is a container: a slab holding them, which starts closed in the interactive SVG.
- `## Components` holds `- ` bullets only.
- `position`, `layout` and `columns` are an error here: set them in a workflow's [overrides](#overrides).
- Warning: a component no workflow shows. It is never drawn.

## Workflows

````md
## Components
- web: Web app
- shop: Shop backend
  - api: API
  - orders: Orders (shape=cylinder)
- gateway: Payment gateway
- fraud: Fraud check
- ledger: Ledger
- vault: Card vault

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

### refund: A customer gets a refund
#### ask: The shop asks for the money back
- web -> api: requests a refund
- api -> gateway: refunds
````

A workflow is `### ID: TITLE`, then, in order:

| Part | Required | Is |
|---|---|---|
| prose | no | the workflow's overview, unfenced Markdown |
| overrides | no | a bullet list changing how this workflow draws some components |
| steps | yes, at least one | `#### ID: TITLE` blocks |

- `subtitle=TEXT` in parentheses after the title is the heading's only property.
- The first workflow is the one the SVG opens on. The reader picks the others from the heading's menu.
- A workflow shows only the components its steps list, as an edge end or bare, plus their ancestors. Each workflow is laid out on its own.
- A container none of whose children the workflow shows is drawn there as a leaf.
- Text between the overrides and the first step is an error.

### Overrides

```md
- REF (PROPERTIES)
```

- One line per component: no display name, description or children.
- Any [component property](#on-a-component); `position`, `layout` and `columns` only here.
- They apply to this workflow only: the component's own properties first, then the override's styles, then its `key=value` entries.
- Errors: a component the workflow doesn't show, a component listed twice.

### Steps

A step is `#### ID: TITLE`, then its prose (optional, unfenced Markdown: the step's description), then at least one item. Steps are numbered from 1 in written order; the ID is unique within its workflow.

| Item | Does |
|---|---|
| `- REF -> REF[: LABEL] [(PROPERTIES)]` | declares a new edge; a [description](#descriptions) block may follow |
| `- REF -> REF -> REF` | declares one edge per hop; no label or properties |
| `- REF -> REF[n]` | lists again the n-th edge between the pair, counted from 0 in written order across the workflow's steps; no label, properties or description |
| `- REF` | shows a component without an edge |

- Edges belong to their workflow. In the example, `api -> gateway` in `refund` is a different edge from `api -> gateway` in `checkout`, and each workflow numbers its own from 0.
- Two items joining the same pair are two edges.
- No edge from a component to itself, its ancestor or its descendant.
- Errors: a step without items, an item listed twice in one step, `[n]` naming no edge of the workflow, text after the items.
- Prose can't hold `- ` bullets or headings outside a code fence: a `- ` line starts the items, a `#` line is a heading.

**REF** is a component's dotted path, or any dotted suffix of it that matches exactly one component:

| Components | REF | Resolves to |
|---|---|---|
| one `writer` | `writer` | `writer` |
| `render.writer` and `store.writer` | `writer` | error listing both |
| `render.writer` and `store.writer` | `render.writer` | `render.writer` |
| top-level `api` and `orders.api` | `api` | the top-level `api`: a whole path wins |

## Descriptions

Components and edges take a fenced block with the info string `description`; workflows and steps take unfenced prose instead.

````md
## Components
- backend: Backend

  ```description
  Every browser request lands here.
  - the API answers synchronously
  - anything slower goes to the queue
  ```

  - api: API
  - queue: Queue

## Workflows
### main: A web app
#### receipt: The API schedules a receipt
- api -> queue: schedules a receipt
  ```description
  One job per order, retried 3 times.
  ```
````

- The block holds any Markdown. Indent it under the component or the edge; under a component, it goes before the children.
- One description per item. Text under a component outside a description block is an error.
- No description under an override, a bare step item, an `[n]` item or a chain.
- The interactive SVG shows workflow and step prose in the steps card, component and edge descriptions in the component card ([Interactive SVG](../interactive.md#description-panel)).

### Links

A link in any description or prose points into the diagram; clicking one goes to its target.

| Link | Targets |
|---|---|
| `[the API](#api)` | a component, by REF |
| `[the charge](#api->gateway)` | the edge between two components, when exactly one exists |
| `[the retry](#api->gateway[1])` | the n-th edge between them, from 0 |
| `[charging it](#checkout/charge)` | step `charge` of workflow `checkout`, from any workflow |

Edge links resolve:

- in workflow prose, step prose and edge descriptions: in that workflow;
- in a component description: in each workflow showing the component, separately.

Targets may name items declared later. Warnings, and the link shows as plain text there:

- a target naming nothing or several items;
- in workflow prose, step prose or an edge description, a component the workflow doesn't show;
- in a component description, an edge target naming nothing or several edges in a workflow showing the component: one warning per such workflow.

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
| `position` | `X,Y` | overrides only: top-left corner in the parent; unpinned components are placed automatically |
| `layout` | `layered` (default), `grid`, `row`, `column` | overrides only: how a container places its children; without it, children with no edges among them line up facing the container's edges, near the neighbours they connect to |
| `columns` | integer > 0 | overrides only: column count, required by `layout=grid` |
| `link` | component REF or URL | a click goes to that component, or opens the URL |
| `code` | `FILE` or `FILE#SYMBOL` | its code, as text, in the component card; `svugy validate` checks it against the repository |
| `meta.NAME` | text | a `NAME: value` line in the component card |

- A colour is `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name or `transparent`.
- A `link` holding `:`, `/`, `?` or `#` is a URL; anything else is a REF. Write a relative page as `./guide.html`.
- Warning: a `link` REF naming a component a workflow doesn't show; in that workflow the component links nowhere.
- A `code` FILE is relative to the root of the git checkout holding the diagram (else the diagram's directory): no leading `/`, no `..`. SYMBOL is `NAME`, `Type::NAME` or `Type.NAME`. `svugy validate` fails when FILE is missing or does not spell each SYMBOL segment as a whole identifier; `render` never reads the repository.
- Warnings: `shape` or `height` on a component a workflow draws as a container (always a slab); `layout` on a component a workflow draws as a leaf.

### On an edge

| Key | Value | Effect |
|---|---|---|
| `line` | `dashed` (default), `dotted`, `solid`, `badge` | how the edge is drawn |
| `color` | colour | stroke |
| `traveler` | `dot` (default), `envelope`, `parcel`, `document`, `key` | the icon moving along the edge |

`line=badge` draws no arc: a small copy of the target sits on the source's top face. Use it for a dependency many components share. Both ends must be leaves.

## Migrating from v3

v3's frontmatter `title` and `subtitle`, the description before `## Components`, `## Edges`, `## Legend`, and `position`, `layout` or `columns` on a component are now errors; each message names its fix. The [migration table](https://github.com/fedemagnani/svugy/blob/main/crates/svugy-cli/skill/references/syntax.md#migrating-from-v3) maps every v3 construct to v4.

Next: [d2](d2.md)
