# Model

You can describe a diagram three ways: d2, JSON, or the Rust builder. They all end up as the same model, which every later stage reads.

Every element is named by its path, like `pipeline.inter`. A segment can contain a dot if it's quoted. Edges are keyed by both endpoints plus an ordinal, so two arrows between the same pair stay two arrows. Each edge lives at the lowest common ancestor of its endpoints, which is where layout will place it. The model also carries the diagram's title, subtitle and legend.

A model gets admitted only if it has no errors, and its warnings always come back with it. The checks cover dangling references, duplicate ids, unknown styles, impossible pins and links that point nowhere. Every number is bounded to ±1e6, so a typo can't produce a gigabyte SVG. Near-miss names get a "did you mean".

`serde_json` silently keeps the last of two duplicate keys. The model reads its maps by hand so it can catch them.

The code is in `crates/svugy-model/src`: `path.rs`, `model.rs`, `builder.rs`, `json.rs`, `validate.rs`.
