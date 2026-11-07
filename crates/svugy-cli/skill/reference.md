# svugy reference

What SKILL.md leaves out. Read only the section you need.

## File

- Parts, in order: frontmatter (optional), `## Components` (required, ≥ 1 item), `## Edges`, `## Legend` or `## Legend: TITLE`.
- Sections hold bullet lists only. Blank lines and `<!-- … -->` are ignored anywhere, including at the end of an item.
- Frontmatter keys: `title`, `subtitle`, `styles`; any other key is an error. Quote a YAML value holding `: `: `title: "Checkout: the flow"`.
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

## Values

- Bare value: runs to the next `,` or `)`, trimmed: `caption=HTTP endpoints`.
- Quoted value: `"…"`, escapes `\"` and `\\`. Required when the value holds `,`, `(`, `)` or `"`.
- Labels follow the same rule; a label ending in `(…)` must be quoted.
- A key appears once per list. Styles apply left to right, then `key=value` entries override them regardless of position.

## Legend

```markdown
## Legend: Read it as a story
- ① your text becomes a model
- (line=dashed): data flows this way
- (line=badge, target=font): reads the bundled font
```

- An item is free text, or a key row `(PROPERTIES): TEXT` drawn next to a sample.
- Key rows take `line` (`solid`, `dashed`, `dotted`, `badge`) and, for `badge`, `target=REF`. A key row describing nothing the diagram draws is a warning.

## Diagnostics

`svugy validate FILE --format json` prints one JSON object. Each entry of `related`:

| Field | Content |
|---|---|
| `message` | the problem, with any `did you mean` |
| `severity` | `error` or `warning` |
| `code` | `svugy::markdown` for format errors; model checks have their own, e.g. `svugy::ignored_container_shape` |
| `labels` | `{label, line, column, span: {offset, length}}`; line and column from 1, span in bytes |

Exit codes: 0 valid (warnings allowed), 1 errors, 2 usage, 3 file unreadable or unwritable.

Messages whose fix isn't in the message:

| Message | Fix |
|---|---|
| `X` is indented but not nested: the comment above ends its parent's list | indent the comment like the item after it |
| a label ending in parentheses needs quotes | `- cache: "Cache (Redis)"` |
| no style `Redis` (after a label) | same: quote the label |
| a chain takes no label | split `a -> b -> c: x` into `a -> b: x` and `b -> c` |
| an edge cannot join `orders` and its descendant | nesting already shows ownership; drop the edge or target a sibling |
| a badge edge needs a leaf at both ends | target a leaf inside the container, or drop `line=badge` |
| malformed frontmatter: mapping values are not allowed | quote the YAML value holding `: ` |

## Importing

- `svugy import in.d2 -o out.md`; `-o out.json` writes the model as JSON. JSON input works the same.
- If the model holds something Markdown can't say (e.g. a d2 key with a space), import lists every such part and writes nothing.
- Imported items spell every property out; factor repeated sets into frontmatter styles if the user will keep editing.
