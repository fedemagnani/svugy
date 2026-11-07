# Model

You can describe a diagram three ways: d2, JSON, or the Rust builder. They all end up as the same model, which every later stage reads.

Every element is named by its path, like `pipeline.inter`. A segment can contain a dot if it's quoted. Edges are keyed by both endpoints plus an ordinal, so two arrows between the same pair stay two arrows. An edge can name its traveler, the symbol the interactive SVG moves along it: `dot` (the default), `envelope`, `parcel`, `document` or `key`. Each edge lives at the lowest common ancestor of its endpoints, which is where layout will place it. The model also carries the diagram's title, subtitle and legend.

A model gets admitted only if it has no errors, and its warnings always come back with it. The checks cover dangling references, duplicate ids, unknown styles, impossible pins and links that point nowhere. Every number is bounded to ±1e6, so a typo can't produce a gigabyte SVG. Near-miss names get a "did you mean".

`serde_json` silently keeps the last of two duplicate keys. The model reads its maps by hand so it can catch them.

Disclosure turns one model into layers 0 to D, where D is the depth of the deepest container and a top-level element has depth 1. Layer K keeps every element down to depth K + 1. The containers at that depth stay as written but lose their children, so in the layer's model they are leaves; the layer remembers which leaves are closed containers. An edge with a hidden endpoint is lifted onto that endpoint's shown ancestor, and dropped if both ends land on one element. The lifted edges between one ordered pair merge into one edge, under the next ordinal free for that pair, and the layer remembers which source edges each merged edge stands for. A merge of one edge keeps that edge's label, style and traveler. Layer D is the model itself. Layers are built directly rather than through the builder, which renumbers ordinals, so every edge they keep has the id it was written with. Since each layer is a plain model, layout never deals with layers, and geometry and render only ask which leaves are closed and what a merged edge stands for.

The code is in `crates/svugy-model/src`: `path.rs`, `model.rs`, `builder.rs`, `json.rs`, `validate.rs`, `disclose.rs`.
