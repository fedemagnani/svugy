# Writing diagrams

svugy reads a subset of [d2](https://d2lang.com), pinned to v0.9.0. The source in the [overview's editor](../overview.md) uses every feature below, so it's a good place to try them.

Most of it is plain d2. `name: Label` declares an element, and braces nest: the parent becomes a slab and its children sit on top. `a.b` names a nested element, and `a -> b: text` draws a labelled arc. d2's `shape: cylinder` and `shape: circle` map to svugy's cylinder and disc. Of the style keys, `fill`, `stroke` and `stroke-dash` are drawn. The root's `label` becomes the diagram's title.

d2 is flat, though. It has no notion of height, captions or badges. svugy reads those from a `vars: { svugy: { … } }` map, which d2 itself ignores. On an element you can set a `caption` (a second line under the label), a `shape` (`box`, `slab`, `cylinder` or `disc`), a `height`, a pinned `position: { x; y }` inside its parent, or `badge: true`, which draws every edge into it as a small icon on its source instead of an arc. On an edge, `traveler` picks what moves along it: `dot`, `envelope`, `parcel`, `document` or `key`. d2 itself rejects `vars` on an edge, so a file with a traveler compiles only in svugy; [d2 limits](../reference/d2-limits.md) has the details.

On the root, the same map holds a `subtitle` and a `legend`. A legend has a title and numbered `entries`, plus key rows: `dashes` describes a line style by its `stroke-dash`, and `badges` describes a badge by its target.

Run `svugy validate` often. d2 doesn't look inside `vars`, so a typo in `vars.svugy` only shows up there. Auto layout stretches chains of edges into long diagonals, so pin elements with `position` when you care about the shape. Unsupported d2 features produce warnings; [d2 limits](../reference/d2-limits.md) lists them.

Next: [The map](../architecture/map.md)
