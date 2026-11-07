# Parsing

`svugy-parse` turns Markdown, d2 and JSON into a [`Model`](model.md), one module per language, so no other crate parses source text and every language reports problems the same way.

<object class="svugy" type="image/svg+xml" data="../figures/parsing.svg"></object>

## Components

**`diagnostic`** is the reporting harness every language implements.

- `SourceLanguage::import(text)` returns an `Import`: the model's admission, the language's errors (message + byte span, as miette diagnostics) and its warnings (d2 only).
- `Import::admitted_model` returns the model only if neither the language nor admission reported an error.
- Every stage records its problems and continues, so one run reports all of them, sorted by source offset.

**`prose`** writes name lists in messages as `` `a`, `b` and `c` ``.

**`markdown`** reads and prints the [Markdown format](../run/markdown.md).

- comrak splits the file into frontmatter, `##` sections, list-item nesting and verbatim item lines; any other block is an error. The frontmatter is YAML (serde-saphyr).
- logos and chumsky parse each item line into a component, an edge or a legend row, recovering at each broken entry.
- Lowering builds the model through `ModelBuilder`. Named styles expand into each properties list, left to right, then its `key=value` entries apply over them; the model receives only the result.
- A reference (edge endpoint, `link`, legend `target`) resolves to the path it spells, else to the one path it is a dotted suffix of. Unknown references get a suggestion; ambiguous ones list their matches.
- `MarkdownPrinter::print` writes any model back, behind `svugy import my.d2 -o my.md`. Whatever Markdown cannot express, such as a `stroke-dash` other than 5 or 2, is reported, never rounded. Print then parse returns the same model.

| Markdown | Model |
|---|---|
| nesting and IDs | `ElementPath` |
| display name, else ID | label |
| `color`, `stroke` | style `fill`, `stroke` |
| `line=dashed`, `line=dotted` | style `stroke-dash` 5, 2 |
| `layout`, `columns` | `PlacementHint` |
| `meta.NAME` | metadata |
| edge `color` | style `stroke` |
| edge `line=badge` | badge edge |

Other properties map to the field of the same name.

**`d2`** imports [d2](../run/d2.md) v0.9.0.

- d2 has no Rust parser, so svugy has its own: logos for tokens, chumsky with recovery for statements.
- Constructs outside svugy's subset (classes, imports, globs, `sql_table`, …) are dropped with a warning.
- Lowering follows d2's compiler: keys are case-folded, repeated keys merge with the last write winning, an edge to an undeclared key declares it.
- What d2 cannot say (subtitle, legend, caption, shape, height, position, badge) comes from `vars: { svugy: { … } }`, which d2 compiles and ignores, so the files stay valid d2. CI compiles every committed `.d2` fixture with d2 v0.9.0.

**`json`** saves and loads a model without loss.

- `JsonPrinter::print` writes elements keyed by path and edges as `{from, to, ordinal, …}` records.
- Loading keeps the saved ordinals and goes straight to admission, skipping `ModelBuilder`, which would renumber them.
- Duplicate keys are errors; serde_json alone would keep the last one.

`examples/svugy.md`, `examples/svugy.d2` and the builder calls in `crates/svugy/tests/golden/svugy.rs` must yield the same model (`golden_svugy.rs`).
