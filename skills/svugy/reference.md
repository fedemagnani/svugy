# svugy language reference

The full language. SKILL.md covers a first diagram; read the section you need here.

- [Document](#document)
- [Frontmatter](#frontmatter)
- [Components](#components)
- [Edges](#edges)
- [Properties and styles](#properties-and-styles)
- [Legend](#legend)
- [Diagnostics](#diagnostics)
- [Importing existing diagrams](#importing-existing-diagrams)

## Document

A diagram is a CommonMark `.md` file with these parts, in this order:

1. Frontmatter: optional, between `---` lines at the very top.
2. `## Components`: required, at least one component.
3. `## Edges`: optional.
4. `## Legend` or `## Legend: TITLE`: optional.

- Put only bullet lists (`- `) in a section. Blank lines and HTML comments (`<!-- … -->`) are ignored.
- Write each list item on one line. It is read verbatim: `*`, `_` and backticks are plain characters inside it.
- Indent a comment between items exactly like the item after it. A comment at a shallower indent ends the list in CommonMark, so the next indented item would become top-level; svugy reports this as "indented but not nested".

```markdown
- billing
  - api
  <!-- the ledger keeps every charge -->
  - ledger
```

## Frontmatter

| Key | Value | Effect |
|---|---|---|
| `title` | text | Drawn top-left of the top-level view; also the browser tooltip and the elevator's root label. |
| `subtitle` | text | Drawn under the title. |
| `styles` | map of name to a properties list | Named property sets, used by name in any properties list. |

Any other key is an error. Write a style exactly like a properties list, parentheses included; colours then need no YAML quotes:

```yaml
styles:
  service: (color=#3478f6, shape=box)
  async: (line=dashed, traveler=envelope)
```

## Components

```
- ID[: "DISPLAY NAME"] [(PROPERTIES)]
```

- ID: letters, digits, `_` and `-`, starting with a letter or `_`. Unique among its siblings, not across the diagram.
- DISPLAY NAME: the label drawn on the component. Without one, the ID is drawn.
- Nesting: indent a child two spaces under its parent. The path of a component is its IDs from the top joined by dots: `shop.api`.
- A component with children is a container. It is drawn as a closed box until the viewer opens it, then as a slab holding its children. A container ignores `shape` and `height` with a warning.

## Edges

```
- REF -> REF[: "LABEL"] [(PROPERTIES)]
- REF -> REF -> REF ...
```

- REF: any dotted suffix of a component's path that matches exactly one component. Use the bare ID when it is unique; add parents (`render.writer`, then `pipeline.render.writer`) only until it is.
- Chain: `a -> b -> c` writes one edge per hop and takes no label and no properties. Write hops on separate lines to label or style them.
- Repeated edges between the same pair are separate edges, kept in the order written.
- An edge between a component and its own ancestor or descendant is an error.
- Edges may cross container boundaries freely: `web -> shop.api` is fine.
- Only `->` exists. To show a reply, write a second edge the other way.

## Properties and styles

A properties list is `( … )` holding comma-separated entries. An entry is `key=value` or a style name.

- Styles apply first, left to right; `key=value` entries then override them, whatever their position. `(service, color=#ff0000)` and `(color=#ff0000, service)` are both a red service.
- A key appears at most once among the `key=value` entries.
- A value is a quoted string (`"…"`, escapes `\"` and `\\` only) or a bare token with no spaces, commas, parentheses or quotes. Quote any text with spaces or commas: `caption="edge, rate limits"`.
- The key decides how its value is read, so `position=760,0` is one value.
- A style may hold component keys, edge keys or both; where it is used, each of its keys must suit that use.

### Component properties

| Key | Value | Effect |
|---|---|---|
| `caption` | string | Second line drawn under the label. |
| `color` | colour | Fill. |
| `stroke` | colour | Outline. |
| `line` | `solid`, `dashed`, `dotted` | Outline dash. |
| `shape` | `box`, `slab`, `cylinder`, `disc` | Leaf primitive. Default `box`. |
| `height` | number > 0 | Leaf height, in plan units. |
| `position` | `X,Y` or `X;Y` | Pinned top-left corner, relative to the parent's interior. |
| `layout` | `layered`, `grid`, `row`, `column` | How a container places its children. Default `layered`. |
| `columns` | integer > 0 | Column count; needs `layout=grid`. |
| `link` | component reference or URL | A click goes to that component, or opens the URL. |
| `meta.NAME` | string | Shown in the detail panel as `NAME: value`. Any number of distinct names. |

A colour is `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name or `transparent`.

Layout: leave `position` out and svugy places components automatically; pick `layout=row`, `column` or `grid` on a container to change how its children are arranged. Pin positions only when the user asks for a fixed layout.

### Edge properties

| Key | Value | Effect |
|---|---|---|
| `line` | `solid`, `dashed`, `dotted`, `badge` | How the edge is drawn. |
| `color` | colour | Stroke. |
| `traveler` | `dot`, `envelope`, `parcel`, `document`, `key` | What moves along the edge in the interactive SVG. Default `dot`. |

`line=badge` draws no arc: it puts a small copy of the target on the source's top face. Use it for a shared dependency that would otherwise draw many long edges (a font, a config, a secret store). Both ends must be leaves.

## Legend

```markdown
## Legend: Read it as a story
- ① your text becomes a model
- (line=dashed): data flows this way
- (line=dotted): embedded at build time
- (line=badge, target=inter): reads the bundled font
```

Items are drawn in order under the title. An item is free text or a key row, drawn next to a sample:

- `(line=dashed): TEXT` and `(line=dotted): TEXT` describe edges drawn that way.
- `(line=badge, target=REF): TEXT` describes the badges of the target `REF`.

## Diagnostics

`svugy validate FILE.md --format json` prints one JSON object on stdout, even when the file is clean. Its `related` array has one entry per problem with `message`, `severity` (`error` or `warning`), `help` (the suggestion, when there is one) and the location. Exit code 0: valid, warnings included. 1: the file has errors. 2: usage error. 3: the file cannot be read or written.

Every problem is reported in one pass, so fix them all before validating again.

| Message | Fix |
|---|---|
| unknown frontmatter key `subtitel`; did you mean `subtitle`? | Use `title`, `subtitle` or `styles` only. |
| unknown key `colour`; did you mean `color`? | Use the suggested key; see the property tables. |
| `pyramid` is not a shape: expected `box`, `slab`, `cylinder` or `disc` | Pick one of the listed values. Same form for every key with fixed values. |
| `position` takes two numbers, `X,Y` | Write `position=X,Y`, or drop `position`. |
| `columns` needs `layout=grid` | Add `layout=grid`, or drop `columns`. |
| `caption` appears twice | Keep one `key=value`; to override a style, write the key once after the style name. |
| no style `servise`; did you mean `service`? | Use the suggested name, or define the style in the frontmatter. |
| `shape` does not apply to an edge | Remove the component-only key from the edge, or from the style used on it. |
| `ledger` is indented but not nested: the comment above ends its parent's list; indent the comment like the item | Indent the comment above it to the item's depth. |
| no component `ordres`; did you mean `orders`? | Fix the reference, or add the missing component. |
| `api` matches `orders.api` and `billing.api`; add a parent to pick one | Write `orders.api` or `billing.api`. |
| a chain takes no label; write its hops on separate lines | Split `a -> b -> c: "x"` into `a -> b: "x"` and `b -> c`. Same for properties. |
| an edge cannot join `orders` and its descendant `orders.store` | Point the edge at a sibling, or drop it: nesting already shows ownership. |
| a badge edge needs a leaf at both ends; `billing` has children | Target a leaf inside the container, or use a normal `line`. |
| warning: a container ignores `shape` and `height` | Remove them; a container is always a slab. |

## Importing existing diagrams

- `svugy import diagram.d2 -o diagram.md` turns a d2 file into a svugy diagram.
- `svugy import model.json -o model.md` does the same for a JSON model; `-o model.json` writes JSON instead.
- The output spells every property out on each item, since the model holds no styles. If the user will keep editing the file, factor repeated property sets into frontmatter styles, then validate again.
- d2 and JSON still render directly, but d2 gets no new features: write new diagrams in the language.
