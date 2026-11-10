# svugy syntax

Rules SKILL.md's example and rules leave out.

## File

- Parts, in order: frontmatter (optional), `## Components` (required, ≥ 1 item), `## Edges`, `## Legend` or `## Legend: TITLE`.
- Sections hold bullet lists only. Blank lines and `<!-- … -->` are ignored anywhere, including at the end of an item.
- Frontmatter keys: `title`, `subtitle`, `view`, `styles`; any other key is an error. `view` is `isometric` (default) or `top` (flat 2D plan). Quote a YAML value holding `: `: `title: "Checkout: the flow"`.
- A style is a properties list, parentheses included: `service: (color=#3478f6, shape=box)`. It may hold component keys, edge keys or both; each key must suit where the style is used.

## Components

- ID: ASCII letters, digits, `_`, single inner `-`; starts with a letter or `_`. Unique among siblings only.
- No label: the ID is drawn.
- Path: IDs from the top, dot-joined: `shop.api`.
- A container ignores `shape` and `height`, with a warning.
- `link=VALUE`: a value holding `:`, `/`, `?` or `#` is a URL; anything else is a REF. Write a relative page as `./guide.html`; bare `guide.html` is read as a REF.
- `meta.NAME=VALUE`: any number of distinct names.
- `position=X,Y` (or `X;Y`): top-left corner relative to the parent's interior. Prefer `layout=row|column|grid` on the parent.

## Edges

- REF: any dotted suffix of a path matching exactly one component; a whole path always wins (`api` is the top-level `api` even if `orders.api` exists).
- Repeated edges between one pair stay separate, in written order.
- Only `->` exists; write a reply as a second edge.
- Edges may cross containers: `web -> shop.api`.
- `line=badge` replaces the arc with a mini of the target on the source's top face. Use it for a dependency many components share (a font, a config store). Both ends must be leaves.

## Descriptions

````markdown
```description
Checkout turns a cart into a charged card.
```

## Components
- web
- shop: Shop backend

  ```description
  Owns the cart and the *orders* table.
  - carts expire after a day
  ```

  - api

## Edges
- web -> api: places order
  ```description
  One request per click on **Pay**.
  ```
````

- A ```` ```description ```` block before `## Components` describes the diagram; indented under a component or an edge, it describes that item.
- Backtick or tilde fence, three or more; when the content holds a line of only ```, open with a longer fence: ````` ````description `````.
- Under an item it goes on the next line or after a blank line, and before a component's children.
- Stored verbatim: any Markdown, bullet lists included.
- Errors: unfenced text under an item or before `## Components`, a code block with another info string, a second description block, a missing closing fence.
- Legend items and chains `a -> b -> c` take no description.

### Links to components and edges

- `[TEXT](#REF)` targets a component; `[TEXT](#REF->REF)` the edge between two, when only one exists; `[TEXT](#REF->REF[n])` the n-th, from 0 in written order. No spaces.
- Targets may name items declared later. The model stores the whole form: `#shop.api`, `#web->shop.api[0]`.
- A target naming nothing or several items is a warning; the link shows as plain text.

## Values

- Bare value: runs to the next `,` or `)`, trimmed: `caption=HTTP endpoints`.
- Quoted value: `"…"`, escapes `\"` and `\\`. Required when the value holds `,`, `(`, `)` or `"`.
- Labels follow the same rule; a label ending in `(…)` must be quoted.
- A key appears once per list. Styles apply left to right, then `key=value` entries override them regardless of position.
