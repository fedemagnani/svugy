# d2 import

d2 is written in Go and has no Rust parser, so svugy parses it itself. logos turns the text into tokens that remember their byte positions. chumsky parses those tokens and keeps going after an error, so you get every mistake at once instead of one per run. Lowering then turns the syntax tree into a model: repeated keys merge, and an edge to an undeclared name creates that element, just like d2 does.

d2 is flat, so anything it can't say goes in a `vars: { svugy: { … } }` map on the shape. d2 compiles vars and ignores them, which makes them the only place svugy can add attributes without breaking the file for d2 itself. The exception is an edge's `traveler`: d2 refuses `vars` on an edge, so svugy reads them there anyway, at the cost of d2 refusing that file. Anything outside the supported subset produces a warning.

The importer is pinned to d2 v0.9.0. CI compiles every `.d2` file in the repo with the real d2 binary, and the golden `examples/svugy.d2` (the map of this book) must import into exactly `examples/svugy.json`.

[d2 limits](../reference/d2-limits.md) lists what d2 can't express and what svugy does instead. The code is in `crates/svugy-import/src/d2`: `lex.rs`, `parse.rs`, `ast.rs`, `subset.rs`, `lower.rs`.
