---
name: svugy
description: Draw architecture diagrams as interactive isometric SVGs by writing Markdown and rendering it with the `svugy` CLI. Use for any request to diagram, draw or visualize a system, codebase, service map, data flow or pipeline, and to edit a svugy `.md` diagram. Not for charts, sequence or Gantt diagrams.
---

# svugy

## Workflow

1. Write `NAME.md`, starting from the example below; to edit a diagram, change it in place.
2. `svugy validate NAME.md --format json`: every problem in one pass, with line, column and a `did you mean` for typos. Fix all of them; repeat until exit code 0 (warnings don't block).
3. Once per diagram, run `svugy view NAME.md` in the background: the user's browser shows it and reloads on every later save. Don't rerun it after edits.
4. For the final file: `svugy render NAME.md -o NAME.svg`, an interactive SVG holding every workflow; `--workflow ID` picks the one it opens on. `--profile static` draws one workflow and needs `--workflow ID`, or `--all-workflows` to write `NAME.ID.svg` per workflow. Add `--theme sketch` if the user asks for a hand-drawn look.

Starting from a `.d2` or `.json` file: `svugy import IN.d2 -o NAME.md` first. If the input holds something Markdown can't say, it lists every such part and writes nothing. A d2 import puts every edge in workflow `main`, step `main`: split it into scenarios. Imported items spell every property out; factor repeated sets into frontmatter styles if the user will keep editing.

## Example

Uses every property in the table below but `code`.

````markdown
---
styles:
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- web: Web app (caption=React, color=#9945ff, link=https://example.com)
- shop: Shop backend (meta.owner=checkout team)

  ```description
  Owns the cart and the *orders* table:
  - `api` takes the order
  - `orders` keeps it
  ```

  - api (caption=REST, stroke=#14f195)
  - orders (store)
- payments
  - gateway: Card gateway (shape=slab, height=40)
  - fraud (shape=disc, line=dashed)
  - ledger (store)
- vault (link=ledger)

## Workflows
### checkout: A customer pays (subtitle=from cart to charged card)
[The web app](#web) sends the cart; [the ledger](#ledger) records the charge.

- shop (layout=row)
- payments (layout=grid, columns=2)

#### order: The shop takes the order
[The API](#api) saves the cart before [charging it](#checkout/charge).
- web -> api: places order (traveler=envelope)
  ```description
  One request per click on **Pay**.
  ```
- api -> orders: saves it (traveler=document, line=dotted)

#### charge: The gateway charges the card
- api -> gateway: charges (traveler=parcel, color=#f0a020)
- gateway -> fraud -> ledger
- gateway -> vault (line=badge)

#### confirm: The web app learns the result
The answer travels back over [the charge request](#api->gateway).
- api -> gateway[0]
- api -> web: approved (traveler=key, line=solid)
- orders

### refund: A customer gets a refund
#### reverse: The ledger reverses the charge
- web -> api: asks for a refund
- api -> ledger: reverses it
````

- Sections, in order: frontmatter (`styles`, `view`; optional), `## Components`, `## Workflows`; both required.
- Component: `- ID[: LABEL] [(PROPERTIES)]`; indent children two spaces. A component with children is a container.
- Workflow: `### ID: TITLE [(subtitle=TEXT)]`, then optional prose, then optional overrides `- REF (PROPERTIES)`, then ≥ 1 step.
- Step: `#### ID: TITLE`, then optional prose, then ≥ 1 item:
  - `- REF -> REF[: LABEL] [(PROPERTIES)]` declares an edge; a chain `a -> b -> c` one per hop.
  - `- REF -> REF[n]` lists again the n-th edge (from 0) between the pair declared in this workflow.
  - `- REF` puts a component in the step without an edge.
- Prose under `###` and `####` is unfenced Markdown, without `- ` bullets or headings; a fence there renders as a code block. A ```` ```description ```` block goes only under a component or a declared edge.
- Links in prose and descriptions: `[the API](#api)` a component, `#api->gateway` the edge, `#api->gateway[1]` the second one, `#checkout/charge` a step.

## Modeling

- One workflow per scenario: a request, a deploy, a nightly job. One step per group of hops that happen together, in order.
- A workflow draws only the components its steps list, plus their ancestors. A component no workflow uses is never drawn.
- Edges belong to their workflow: repeat one in another workflow by declaring it again; repeat it in a later step of the same workflow with `[n]`.
- The reader walks the steps in a panel that dims everything outside the current step; write each step's prose for that reader.

## Properties

| Key | On | Values |
|---|---|---|
| `caption` | component | text under the label |
| `color` | both | `#rgb[a]`, `#rrggbb[aa]`, CSS name, `transparent`; fill of a component, stroke of an edge |
| `stroke` | component | outline colour |
| `line` | both | `solid`, `dashed`, `dotted`; edges also `badge`: a mini of the target on the source, both ends leaves. Edges default to `dashed` |
| `shape` | leaf | `box` (default), `slab`, `cylinder`, `disc` |
| `height` | leaf | number > 0 |
| `layout` | container, override only | `layered` (default), `row`, `column`, `grid` |
| `columns` | container, override only | integer > 0; requires `layout=grid` |
| `position` | override only | `X,Y`; only when the user asks for a fixed layout |
| `traveler` | edge | `dot` (default), `envelope`, `parcel`, `document`, `key` |
| `link` | component | a URL (holds `:`, `/`, `?` or `#`) or a component REF; opened on click |
| `code` | component | `FILE#SYMBOL`, FILE relative to the repository root; `svugy validate` fails when the file or symbol is gone |
| `meta.NAME` | component | text shown as `NAME: value` in the component card |

## Rules

- No `# Title` and no frontmatter `title`: a workflow's heading is its title. Notes go in `<!-- … -->`.
- A bare word in `( )` is a style name; styles apply first, `key=value` entries override them.
- An override takes any component key and applies to its workflow only. `position`, `layout` and `columns` go only in overrides, directly or through a style used there.
- Quote a label or value only if it holds `,`, `(`, `)` or `"`, or if a label ends in `(…)`: `- cache: "Cache (Redis)"`.
- REF is the shortest unique dotted suffix of a path: `api`, or `shop.api` if two components are named `api`.
- Chains, `[n]` items and bare items take no label, properties or description.
- No edge from a component to itself, its ancestor or its descendant.
- An item's line is read verbatim: no emphasis or backticks there. Its description is Markdown.
- Under a component or edge, the only non-item block is one ```` ```description ```` block, before the component's children; unfenced text there is an error.
- Indent a comment like the item after it; a shallower one breaks nesting.

## References

Read one only when its case applies:

- [references/syntax.md](references/syntax.md): a case the example and rules above don't settle: IDs, paths, REF resolution, overrides, steps, descriptions, links, quoting, migrating a v3 file.
- [references/troubleshoot.md](references/troubleshoot.md): `validate` fails and its message doesn't say how to fix it, or you need its JSON fields or exit codes.
- [references/json.md](references/json.md): the input or the wanted output is a `.json` model.
- [references/install.md](references/install.md): `svugy` is not on `PATH`. Install only once the user agrees.
